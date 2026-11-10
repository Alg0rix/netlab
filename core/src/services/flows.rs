//! TCP/UDP probes, the HTTP service and ICMP unreachables.

use crate::device::{Device, DeviceKind};
use crate::eth::{self, view_ip};
use crate::events::SimEvent;
use crate::frame::Frame;
use crate::ip_str;

use super::dns::dns_answer;
use super::{alloc_port, next_id, note};

const TCP_FIN: u8 = 0x01;
const TCP_SYN: u8 = 0x02;
const TCP_RST: u8 = 0x04;
const TCP_ACK: u8 = 0x10;
const TCP_PSH: u8 = 0x08;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowKind {
    TcpProbe,
    Http,
    Udp,
    TcpServer,
}

impl FlowKind {
    fn event_kind(self) -> &'static str {
        match self {
            FlowKind::Http => "http",
            FlowKind::Udp => "udp",
            _ => "tcp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowState {
    SynSent,
    Established,
    Done,
}
#[derive(Clone, Debug)]
pub struct Flow {
    pub kind: FlowKind,
    pub lip: [u8; 4],
    pub lport: u16,
    pub rip: [u8; 4],
    pub rport: u16,
    pub state: FlowState,
    pub seq: u32,
    pub ack: u32,
    pub tx: Vec<u8>,
    pub deadline: u64,
}

pub(super) fn poll_flows(dev: &mut Device, tick: u64, events: &mut Vec<SimEvent>) {
    let mut i = 0;
    while i < dev.flows.len() {
        if tick >= dev.flows[i].deadline && dev.flows[i].state != FlowState::Done {
            let f = dev.flows.remove(i);
            let kind = f.kind.event_kind();
            let msg = format!(
                "{} {}:{} timeout",
                kind.to_ascii_uppercase(),
                ip_str(f.rip),
                f.rport
            );
            note(events, tick, dev, kind, &msg);
        } else {
            i += 1;
        }
    }
}

pub fn start_tcp(
    dev: &mut Device,
    dst: [u8; 4],
    dport: u16,
    http: bool,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(src) = dev.first_ip() else {
        note(
            events,
            tick,
            dev,
            if http { "http" } else { "tcp" },
            "no IPv4 address",
        );
        return false;
    };
    let sport = alloc_port(dev);
    let seq = next_id(dev);
    let tx = if http {
        format!("GET / HTTP/1.0\r\nHost: {}\r\n\r\n", ip_str(dst)).into_bytes()
    } else {
        Vec::new()
    };
    dev.flows.push(Flow {
        kind: if http {
            FlowKind::Http
        } else {
            FlowKind::TcpProbe
        },
        lip: src,
        lport: sport,
        rip: dst,
        rport: dport,
        state: FlowState::SynSent,
        seq,
        ack: 0,
        tx,
        deadline: tick + 4000,
    });
    send_tcp(
        dev,
        src,
        sport,
        dst,
        dport,
        seq,
        0,
        TCP_SYN,
        &[],
        tick,
        events,
    );
    true
}

pub fn start_udp(
    dev: &mut Device,
    dst: [u8; 4],
    dport: u16,
    payload: &[u8],
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(src) = dev.first_ip() else {
        note(events, tick, dev, "udp", "no IPv4 address");
        return false;
    };
    let sport = alloc_port(dev);
    dev.flows.push(Flow {
        kind: FlowKind::Udp,
        lip: src,
        lport: sport,
        rip: dst,
        rport: dport,
        state: FlowState::Established,
        seq: 0,
        ack: 0,
        tx: payload.to_vec(),
        deadline: tick + 4000,
    });
    let mac = dev
        .ports
        .iter()
        .find_map(|p| p.ip.map(|_| p.mac))
        .unwrap_or(0);
    let raw = eth::build_udp(mac, 0, src, dst, 64, dev.ip_id, sport, dport, payload);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.emit_ip(Frame::raw_ip(src, dst, 64, raw), tick, events);
    true
}

pub(super) fn on_udp(
    dev: &mut Device,
    frame: &Frame,
    raw: &[u8],
    v: eth::IpView,
    in_port: usize,
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    if let Some(i) = dev.flows.iter().position(|f| {
        f.kind == FlowKind::Udp && f.lport == v.dport && f.rport == v.sport && f.rip == frame.src_ip
    }) {
        let f = dev.flows.remove(i);
        let payload = &raw[v.payload.min(raw.len())..];
        if f.rport == 53 {
            let name = String::from_utf8_lossy(&f.tx);
            if let Some((nm, ip, nx)) = dns_answer(payload) {
                let msg = if nx {
                    format!("DNS {nm} NXDOMAIN")
                } else {
                    format!("DNS {nm} → {}", ip_str(ip))
                };
                note(events, tick, dev, "dns", &msg);
            } else {
                note(events, tick, dev, "dns", &format!("DNS {name} — no answer"));
            }
        } else {
            let text = String::from_utf8_lossy(payload);
            let preview: String = text.chars().take(80).collect();
            note(
                events,
                tick,
                dev,
                "udp",
                &format!("UDP from {}:{} {}", ip_str(frame.src_ip), v.sport, preview),
            );
        }
        return;
    }
    // closed UDP port → ICMP unreachable so `nc` can tell drop from closed
    if dev.kind == DeviceKind::Host {
        send_unreach(dev, frame, raw, in_port);
        note(
            events,
            tick,
            dev,
            "udp",
            &format!("UDP {}:{} closed", ip_str(frame.dst_ip), v.dport),
        );
    }
}

pub(super) fn on_unreach(dev: &mut Device, frame: &Frame, tick: u64, events: &mut Vec<SimEvent>) {
    if let Some(i) = dev
        .flows
        .iter()
        .position(|f| f.rip == frame.src_ip && f.state != FlowState::Done)
    {
        let f = dev.flows.remove(i);
        let kind = f.kind.event_kind();
        note(
            events,
            tick,
            dev,
            kind,
            &format!(
                "{} {}:{} unreachable",
                kind.to_ascii_uppercase(),
                ip_str(f.rip),
                f.rport
            ),
        );
    }
}

pub(super) fn on_tcp(
    dev: &mut Device,
    frame: &Frame,
    raw: &[u8],
    v: eth::IpView,
    _in_port: usize,
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let l4 = v.off + v.ihl;
    if raw.len() < l4 + 20 {
        return;
    }
    let seq = u32::from_be_bytes(raw[l4 + 4..l4 + 8].try_into().unwrap());
    let flags = raw[l4 + 13];
    let doff = (raw[l4 + 12] >> 4) as usize * 4;
    let ip_pay_end =
        (v.off + u16::from_be_bytes([raw[v.off + 2], raw[v.off + 3]]) as usize).min(raw.len());
    let data = if l4 + doff < ip_pay_end {
        &raw[l4 + doff..ip_pay_end]
    } else {
        &[]
    };

    if let Some(i) = dev.flows.iter().position(|f| {
        f.kind != FlowKind::TcpServer
            && f.lport == v.dport
            && f.rport == v.sport
            && f.rip == frame.src_ip
            && f.lip == frame.dst_ip
    }) {
        client_tcp(dev, i, flags, seq, data, tick, events);
        return;
    }
    if flags & TCP_SYN != 0 && flags & TCP_ACK == 0 {
        let listen = (dev.svc_http && v.dport == 80) || dev.listen.contains(&v.dport);
        if listen {
            let isn = next_id(dev);
            dev.flows.push(Flow {
                kind: FlowKind::TcpServer,
                lip: frame.dst_ip,
                lport: v.dport,
                rip: frame.src_ip,
                rport: v.sport,
                state: FlowState::Established,
                seq: isn.wrapping_add(1),
                ack: seq.wrapping_add(1),
                tx: Vec::new(),
                deadline: tick + 4000,
            });
            send_tcp(
                dev,
                frame.dst_ip,
                v.dport,
                frame.src_ip,
                v.sport,
                isn,
                seq.wrapping_add(1),
                TCP_SYN | TCP_ACK,
                &[],
                tick,
                events,
            );
        } else if dev.kind == DeviceKind::Host {
            send_tcp(
                dev,
                frame.dst_ip,
                v.dport,
                frame.src_ip,
                v.sport,
                0,
                seq.wrapping_add(1),
                TCP_RST | TCP_ACK,
                &[],
                tick,
                events,
            );
            note(
                events,
                tick,
                dev,
                "tcp",
                &format!("TCP {}:{} closed", ip_str(frame.dst_ip), v.dport),
            );
        }
    } else if let Some(i) = dev.flows.iter().position(|f| {
        f.kind == FlowKind::TcpServer
            && f.lport == v.dport
            && f.rport == v.sport
            && f.rip == frame.src_ip
    }) {
        server_tcp(dev, i, flags, seq, data, tick, events);
    }
}

fn client_tcp(
    dev: &mut Device,
    i: usize,
    flags: u8,
    seq: u32,
    data: &[u8],
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let f = dev.flows[i].clone();
    if flags & TCP_RST != 0 {
        dev.flows.remove(i);
        let kind = f.kind.event_kind();
        note(
            events,
            tick,
            dev,
            kind,
            &format!("TCP {}:{} closed", ip_str(f.rip), f.rport),
        );
        return;
    }
    if f.state == FlowState::SynSent && flags & TCP_SYN != 0 && flags & TCP_ACK != 0 {
        let next = f.seq.wrapping_add(1);
        if f.kind == FlowKind::TcpProbe && f.tx.is_empty() {
            send_tcp(
                dev,
                f.lip,
                f.lport,
                f.rip,
                f.rport,
                next,
                seq.wrapping_add(1),
                TCP_ACK,
                &[],
                tick,
                events,
            );
            dev.flows.remove(i);
            note(
                events,
                tick,
                dev,
                "tcp",
                &format!("TCP {}:{} open", ip_str(f.rip), f.rport),
            );
            return;
        }
        dev.flows[i].state = FlowState::Established;
        dev.flows[i].seq = next.wrapping_add(f.tx.len() as u32);
        dev.flows[i].ack = seq.wrapping_add(1);
        dev.flows[i].deadline = tick + 4000;
        let tx = f.tx.clone();
        send_tcp(
            dev,
            f.lip,
            f.lport,
            f.rip,
            f.rport,
            next,
            seq.wrapping_add(1),
            TCP_ACK | TCP_PSH,
            &tx,
            tick,
            events,
        );
        return;
    }
    if !data.is_empty() {
        let text = String::from_utf8_lossy(data);
        dev.flows.remove(i);
        if f.kind == FlowKind::Http {
            let status = text.split_whitespace().nth(1).unwrap_or("?");
            note(
                events,
                tick,
                dev,
                "http",
                &format!(
                    "HTTP {status} from {}:{} ({} bytes)",
                    ip_str(f.rip),
                    f.rport,
                    data.len()
                ),
            );
        } else {
            note(
                events,
                tick,
                dev,
                "tcp",
                &format!(
                    "TCP {}:{} data ({} bytes)",
                    ip_str(f.rip),
                    f.rport,
                    data.len()
                ),
            );
        }
    }
}

fn server_tcp(
    dev: &mut Device,
    i: usize,
    flags: u8,
    seq: u32,
    data: &[u8],
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let f = dev.flows[i].clone();
    if flags & TCP_RST != 0 {
        dev.flows.remove(i);
        return;
    }
    if data.is_empty() && flags & TCP_FIN == 0 {
        return;
    }
    let body = b"netlab\n";
    let resp = format!(
        "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: text/plain\r\n\r\n",
        body.len()
    );
    let mut bytes = resp.into_bytes();
    bytes.extend_from_slice(body);
    let ack = seq
        .wrapping_add(data.len() as u32)
        .wrapping_add(if flags & TCP_FIN != 0 { 1 } else { 0 });
    send_tcp(
        dev,
        f.lip,
        f.lport,
        f.rip,
        f.rport,
        f.seq,
        ack,
        TCP_ACK | TCP_PSH | TCP_FIN,
        &bytes,
        tick,
        events,
    );
    dev.flows.remove(i);
    note(
        events,
        tick,
        dev,
        "http",
        &format!("HTTP 200 to {}:{}", ip_str(f.rip), f.rport),
    );
}

fn send_tcp(
    dev: &mut Device,
    src: [u8; 4],
    sport: u16,
    dst: [u8; 4],
    dport: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    payload: &[u8],
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let mac = dev.mac_of_ip(src).unwrap_or(0);
    let raw = eth::build_tcp(
        mac, 0, src, dst, 64, dev.ip_id, sport, dport, seq, ack, flags, payload,
    );
    dev.ip_id = dev.ip_id.wrapping_add(1);
    let mut f = Frame::raw_ip(src, dst, 64, raw);
    f.src_mac = mac;
    dev.emit_ip(f, tick, events);
}

fn send_unreach(dev: &mut Device, frame: &Frame, raw: &[u8], in_port: usize) {
    let Some(v) = view_ip(raw) else { return };
    let src = dev
        .ports
        .get(in_port)
        .and_then(|p| p.ip.map(|(a, _)| a))
        .unwrap_or(frame.dst_ip);
    let smac = dev.ports.get(in_port).map(|p| p.mac).unwrap_or(0);
    // ICMP dest unreachable, code 3, quoting the original IP header + 8 bytes
    let quote_end = (v.off + v.ihl + 8).min(raw.len());
    let mut body = vec![0; 4];
    body.extend_from_slice(&raw[v.off..quote_end]);
    let out = eth::build_icmp(
        smac,
        frame.src_mac,
        src,
        frame.src_ip,
        64,
        dev.ip_id,
        3,
        3,
        &body,
    );
    dev.ip_id = dev.ip_id.wrapping_add(1);
    let mut f = Frame::raw_ip(src, frame.src_ip, 64, out);
    f.src_mac = smac;
    f.dst_mac = frame.src_mac;
    dev.outbox.push((in_port, f));
}
