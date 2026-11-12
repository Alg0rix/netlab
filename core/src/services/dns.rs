//! DNS client and server.

use crate::device::Device;
use crate::eth::{self};
use crate::events::{EvKind, SimEvent};
use crate::frame::Frame;
use crate::ip_str;

use super::flows::{Flow, FlowKind, FlowState};
use super::{alloc_port, note};
pub fn start_dns(dev: &mut Device, name: &str, tick: u64, events: &mut Vec<SimEvent>) -> bool {
    let Some(dst) = dev.resolver else {
        note(
            events,
            tick,
            dev,
            EvKind::Syslog,
            "DNS failed — no server (DHCP, or `ip dns <addr>`)",
        );
        return false;
    };
    let Some(src) = dev.first_ip() else {
        note(events, tick, dev, EvKind::Syslog, "DNS failed — no IPv4 address");
        return false;
    };
    let q = build_query(name);
    let id = u16::from_be_bytes([q[0], q[1]]);
    let sport = alloc_port(dev);
    dev.flows.push(Flow {
        kind: FlowKind::Udp,
        lip: src,
        lport: sport,
        rip: dst,
        rport: 53,
        state: FlowState::Established,
        seq: id as u32,
        ack: 0,
        tx: name.as_bytes().to_vec(),
        deadline: tick + 4000,
    });
    // Source MAC is rewritten by encode() from frame.src_mac, so the placeholder is fine.
    let mac = dev
        .ports
        .iter()
        .find_map(|p| p.ip.map(|_| p.mac))
        .unwrap_or(0);
    let raw = eth::build_udp(
        eth::IpSpec {
            src_mac: mac,
            dst_mac: 0,
            src,
            dst,
            ttl: 64,
            id: dev.ip_id,
        },
        sport,
        53,
        &q,
    );
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.emit_ip(Frame::raw_ip(src, dst, 64, raw), tick);
    true
}

pub(super) fn on_dns(
    dev: &mut Device,
    frame: &Frame,
    raw: &[u8],
    v: eth::IpView,
    in_port: usize,
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let payload = &raw[v.payload.min(raw.len())..];
    if payload.len() < 12 {
        return;
    }
    let name = dns_qname(payload).unwrap_or_default();
    let ip = dev
        .dns_records
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(&name))
        .map(|(_, ip)| *ip)
        .or_else(|| dev.first_ip());
    let answer = match ip {
        Some(ip) => dns_response(payload, ip, false),
        None => dns_response(payload, [0; 4], true),
    };
    let port = in_port.min(dev.ports.len().saturating_sub(1));
    let smac = dev.ports[port].mac;
    let src = dev.ports[port].ip.map(|(a, _)| a).unwrap_or(frame.dst_ip);
    let bytes = eth::build_udp(
        eth::IpSpec {
            src_mac: smac,
            dst_mac: frame.src_mac,
            src,
            dst: frame.src_ip,
            ttl: 64,
            id: dev.ip_id,
        },
        53,
        v.sport,
        &answer,
    );
    dev.ip_id = dev.ip_id.wrapping_add(1);
    let mut f = Frame::raw_ip(src, frame.src_ip, 64, bytes);
    f.src_mac = smac;
    f.dst_mac = frame.src_mac;
    dev.outbox.push((port, f));
    note(
        events,
        tick,
        dev,
        EvKind::Syslog,
        &format!(
            "DNS {name} → {}",
            ip.map(ip_str).unwrap_or_else(|| "NXDOMAIN".into())
        ),
    );
}

fn build_query(name: &str) -> Vec<u8> {
    let mut q = vec![0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    q[0] = (name.len() as u8).wrapping_mul(3).wrapping_add(1);
    q[1] = name.len() as u8;
    for label in name.split('.') {
        if label.is_empty() {
            continue;
        }
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&[0, 1, 0, 1]); // A IN
    q
}

fn dns_qname(msg: &[u8]) -> Option<String> {
    if msg.len() < 12 {
        return None;
    }
    let mut i = 12;
    let mut parts = Vec::new();
    while i < msg.len() {
        let n = msg[i] as usize;
        if n == 0 {
            break;
        }
        if n > 63 || i + 1 + n > msg.len() {
            return None;
        }
        parts.push(String::from_utf8_lossy(&msg[i + 1..i + 1 + n]).to_string());
        i += 1 + n;
    }
    Some(parts.join("."))
}

fn dns_response(query: &[u8], ip: [u8; 4], nx: bool) -> Vec<u8> {
    let mut r = query.to_vec();
    if r.len() < 12 {
        return r;
    }
    r[2] = 0x84; // response, AA
    r[3] = if nx { 0x83 } else { 0x80 }; // RD + NXDOMAIN or RA
    if nx {
        r[6] = 0;
        r[7] = 0;
        return r;
    }
    r[6] = 0;
    r[7] = 1; // ancount
    r.extend_from_slice(&[0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4]);
    r.extend_from_slice(&ip);
    r
}

pub(super) fn dns_answer(msg: &[u8]) -> Option<(String, [u8; 4], bool)> {
    let name = dns_qname(msg).unwrap_or_default();
    if msg.len() < 4 {
        return None;
    }
    let nx = msg[3] & 0x0f == 3;
    if nx {
        return Some((name, [0; 4], true));
    }
    // scan for a type A answer (rough: last 4 bytes after a 4-len)
    let mut i = 12;
    while i < msg.len() && msg[i] != 0 {
        let n = msg[i] as usize;
        if n >= 192 {
            i += 2;
            break;
        }
        if n == 0 || i + 1 + n >= msg.len() {
            break;
        }
        i += 1 + n;
    }
    if i < msg.len() && msg[i] == 0 {
        i += 5;
    }
    if msg.len() >= i + 16 && u16::from_be_bytes([msg[i + 2], msg[i + 3]]) == 1 {
        let ip = [msg[i + 12], msg[i + 13], msg[i + 14], msg[i + 15]];
        return Some((name, ip, false));
    }
    // fallback: if ancount > 0 take the last 4 bytes
    if msg.len() >= 16 && u16::from_be_bytes([msg[6], msg[7]]) > 0 {
        let n = msg.len();
        return Some((
            name,
            [msg[n - 4], msg[n - 3], msg[n - 2], msg[n - 1]],
            false,
        ));
    }
    None
}
