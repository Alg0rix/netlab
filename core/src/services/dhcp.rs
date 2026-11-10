//! DHCP client, server and relay.

use crate::device::{Device, DeviceKind};
use crate::eth::{self, view_ip};
use crate::events::SimEvent;
use crate::frame::{Frame, BROADCAST_MAC};
use crate::{ip_str, mask_bits};

use super::{next_id, note, udp_frame};

const DHCP_DISCOVER: u8 = 1;
const DHCP_OFFER: u8 = 2;
const DHCP_REQUEST: u8 = 3;
const DHCP_ACK: u8 = 5;
const DHCP_MAGIC: [u8; 4] = [99, 130, 83, 99];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DhcpState {
    Discover,
    Request,
    Bound,
}
#[derive(Clone, Debug)]
pub struct DhcpClient {
    pub port: usize,
    pub xid: u32,
    pub state: DhcpState,
    pub offer: [u8; 4],
    pub server: [u8; 4],
    pub tries: u8,
    pub next: u64,
}

pub(super) fn poll_client(dev: &mut Device, tick: u64, events: &mut Vec<SimEvent>) {
    if let Some(c) = dev.dhcp.as_ref() {
        if c.state != DhcpState::Bound && tick >= c.next && dev.powered {
            let port = c.port;
            let tries = c.tries;
            if tries >= 4 {
                dev.dhcp = None;
                note(
                    events,
                    tick,
                    dev,
                    "dhcp",
                    "DHCP failed — no offer (is a server on this link?)",
                );
            } else if c.state == DhcpState::Discover {
                send_discover(dev, port, tick, events);
            } else if c.state == DhcpState::Request {
                send_request(dev, port, tick, events);
            }
        }
    }
}

pub fn start_dhcp(dev: &mut Device, port: usize, tick: u64, events: &mut Vec<SimEvent>) -> bool {
    if !dev.powered || port >= dev.ports.len() || dev.kind == DeviceKind::Switch {
        return false;
    }
    dev.dhcp = Some(DhcpClient {
        port,
        xid: 0,
        state: DhcpState::Discover,
        offer: [0; 4],
        server: [0; 4],
        tries: 0,
        next: tick,
    });
    send_discover(dev, port, tick, events);
    true
}

fn send_discover(dev: &mut Device, port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    let xid = next_id(dev);
    let Some(c) = dev.dhcp.as_mut() else { return };
    c.tries += 1;
    c.state = DhcpState::Discover;
    c.next = tick + 40;
    c.xid = xid;
    let mac = dev.ports[port].mac;
    let pkt = dhcp_packet(
        1,
        xid,
        mac,
        [0; 4],
        [0; 4],
        [0; 4],
        &[(53, vec![DHCP_DISCOVER]), (55, vec![1, 3, 6])],
    );
    let frame = udp_frame(
        dev,
        mac,
        BROADCAST_MAC,
        [0; 4],
        [255, 255, 255, 255],
        68,
        67,
        &pkt,
    );
    note(
        events,
        tick,
        dev,
        "dhcp",
        &format!(
            "DHCP discover on {}",
            &format!("port {} ({})", port, dev.name)
        ),
    );
    dev.outbox.push((port, frame));
}

fn send_request(dev: &mut Device, port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    let Some(c) = dev.dhcp.clone() else { return };
    if let Some(dc) = dev.dhcp.as_mut() {
        dc.tries += 1;
        dc.state = DhcpState::Request;
        dc.next = tick + 40;
    }
    let mac = dev.ports[port].mac;
    let opts = vec![
        (53, vec![DHCP_REQUEST]),
        (50, c.offer.to_vec()),
        (54, c.server.to_vec()),
        (55, vec![1, 3, 6]),
    ];
    let pkt = dhcp_packet(1, c.xid, mac, [0; 4], [0; 4], [0; 4], &opts);
    let frame = udp_frame(
        dev,
        mac,
        BROADCAST_MAC,
        [0; 4],
        [255, 255, 255, 255],
        68,
        67,
        &pkt,
    );
    note(
        events,
        tick,
        dev,
        "dhcp",
        &format!("DHCP request {}", ip_str(c.offer)),
    );
    dev.outbox.push((port, frame));
}

/// DHCP stays on the link unless a helper is set. Relayed unicasts are routed.
pub(super) fn dispatch(
    dev: &mut Device,
    frame: &Frame,
    raw: &[u8],
    in_port: usize,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(v) = view_ip(raw) else { return true };
    let payload = &raw[v.payload.min(raw.len())..];
    let bcast = frame.is_broadcast();
    let mine = dev
        .ports
        .iter()
        .any(|p| p.ip.map(|(a, _)| a) == Some(frame.dst_ip));
    if v.dport == 68 {
        if let Some(msg) = parse_dhcp(payload) {
            if let Some(c) = dev.dhcp.clone() {
                if msg.xid == c.xid && msg.chaddr == eth::mac_bytes(dev.ports[c.port].mac) {
                    client_dhcp(dev, &msg, tick, events);
                }
            }
        }
        return true;
    }
    let Some(msg) = parse_dhcp(payload) else {
        return bcast || mine;
    };
    if dev.svc_dhcp && (bcast || mine || msg.giaddr != [0; 4]) {
        serve_dhcp(dev, &msg, in_port, tick, events);
        return true;
    }
    if mine {
        if msg.bootp == 2 {
            if let Some(port) = dev
                .ports
                .iter()
                .position(|p| p.ip.map(|(a, _)| a) == Some(msg.giaddr))
            {
                relay_to_client(dev, port, payload, tick, events);
            }
        }
        return true;
    }
    if bcast {
        if msg.giaddr == [0; 4] && payload.get(3).copied().unwrap_or(0) < 8 {
            if let Some(helper) = dev.ports.get(in_port).and_then(|p| p.helper) {
                relay_to_helper(dev, in_port, helper, payload, tick, events);
            }
        }
        return true;
    }
    false
}

fn relay_to_helper(
    dev: &mut Device,
    in_port: usize,
    helper: [u8; 4],
    payload: &[u8],
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let Some((src, _)) = dev.ports.get(in_port).and_then(|p| p.ip) else {
        return;
    };
    let mut pkt = payload.to_vec();
    if pkt.len() < 34 {
        return;
    }
    pkt[3] = pkt[3].saturating_add(1);
    pkt[24..28].copy_from_slice(&src);
    let raw = eth::build_udp(0, 0, src, helper, 64, dev.ip_id, 67, 67, &pkt);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    note(
        events,
        tick,
        dev,
        "dhcp",
        &format!("DHCP relay {} -> {}", ip_str(src), ip_str(helper)),
    );
    dev.emit_ip(Frame::raw_ip(src, helper, 64, raw), tick, events);
}

fn relay_to_client(
    dev: &mut Device,
    port: usize,
    payload: &[u8],
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let Some((src, _)) = dev.ports.get(port).and_then(|p| p.ip) else {
        return;
    };
    let smac = dev.ports[port].mac;
    let frame = udp_frame(
        dev,
        smac,
        BROADCAST_MAC,
        src,
        [255, 255, 255, 255],
        67,
        68,
        payload,
    );
    note(
        events,
        tick,
        dev,
        "dhcp",
        &format!(
            "DHCP relay reply on {}",
            &format!("port {} ({})", port, dev.name)
        ),
    );
    dev.outbox.push((port, frame));
}

fn serve_dhcp(
    dev: &mut Device,
    msg: &DhcpMsg,
    in_port: usize,
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    if msg.op != DHCP_DISCOVER && msg.op != DHCP_REQUEST {
        return;
    }
    let relayed = msg.giaddr != [0; 4];
    let port = if relayed {
        dev.ports
            .iter()
            .position(|p| p.ip.is_some())
            .unwrap_or(in_port)
    } else {
        dev.ports
            .iter()
            .position(|p| p.ip.is_some() && p.link.is_some())
            .unwrap_or(in_port)
    };
    let Some((my_ip, mask)) = (if relayed {
        let mask = dev
            .ports
            .iter()
            .find_map(|p| {
                p.ip.filter(|(a, m)| crate::net_of(*a, *m) == crate::net_of(msg.giaddr, *m))
                    .map(|(_, m)| m)
            })
            .unwrap_or([255, 255, 255, 0]);
        Some((msg.giaddr, mask))
    } else {
        dev.ports.get(port).and_then(|p| p.ip)
    }) else {
        return;
    };
    let mac = eth::mac_of(&msg.chaddr);
    let ip = if let Some(ip) = dev.leases.get(&mac).copied() {
        ip
    } else if let Some(ip) = alloc_lease(dev, my_ip, mask) {
        dev.leases.insert(mac, ip);
        ip
    } else {
        return;
    };
    let typ = if msg.op == DHCP_DISCOVER {
        DHCP_OFFER
    } else {
        DHCP_ACK
    };
    let mut opts = vec![
        (53, vec![typ]),
        (54, my_ip.to_vec()),
        (1, mask.to_vec()),
        (51, 3600u32.to_be_bytes().to_vec()),
    ];
    let gw = if relayed {
        msg.giaddr
    } else {
        dev.ports
            .get(port)
            .and_then(|p| p.ip.map(|(a, _)| a))
            .unwrap_or(my_ip)
    };
    opts.push((3, gw.to_vec()));
    let src = dev
        .ports
        .iter()
        .find_map(|p| p.ip.map(|(a, _)| a))
        .unwrap_or(my_ip);
    let dns = dev.resolver.unwrap_or(src);
    opts.push((6, dns.to_vec()));
    let pkt = dhcp_packet(
        2,
        msg.xid,
        mac,
        ip,
        src,
        if relayed { msg.giaddr } else { [0; 4] },
        &opts,
    );
    let word = if typ == DHCP_OFFER { "offer" } else { "ack" };
    note(
        events,
        tick,
        dev,
        "dhcp",
        &format!("DHCP {word} {} to {}", ip_str(ip), crate::mac_str(mac)),
    );
    if relayed {
        let raw = eth::build_udp(0, 0, src, msg.giaddr, 64, dev.ip_id, 67, 67, &pkt);
        dev.ip_id = dev.ip_id.wrapping_add(1);
        dev.emit_ip(Frame::raw_ip(src, msg.giaddr, 64, raw), tick, events);
    } else {
        let smac = dev.ports[port].mac;
        let frame = udp_frame(
            dev,
            smac,
            BROADCAST_MAC,
            my_ip,
            [255, 255, 255, 255],
            67,
            68,
            &pkt,
        );
        dev.outbox.push((port, frame));
    }
}

fn client_dhcp(dev: &mut Device, msg: &DhcpMsg, tick: u64, events: &mut Vec<SimEvent>) {
    let Some(c) = dev.dhcp.as_mut() else { return };
    if msg.op == DHCP_OFFER && c.state == DhcpState::Discover {
        c.offer = msg.yiaddr;
        c.server = opt4(msg, 54).unwrap_or(msg.siaddr);
        c.state = DhcpState::Request;
        c.tries = 0;
        c.next = tick;
        return;
    }
    if msg.op == DHCP_ACK && (c.state == DhcpState::Request || c.state == DhcpState::Discover) {
        let port = c.port;
        let ip = msg.yiaddr;
        let mask = opt4(msg, 1).unwrap_or([255, 255, 255, 0]);
        let gw = opt4(msg, 3).unwrap_or([0; 4]);
        let dns = opt4(msg, 6).unwrap_or([0; 4]);
        c.state = DhcpState::Bound;
        c.next = u64::MAX;
        if port < dev.ports.len() {
            dev.ports[port].ip = Some((ip, mask));
        }
        if gw != [0; 4] {
            dev.set_default_route(gw, port);
        }
        if dns != [0; 4] {
            dev.resolver = Some(dns);
        }
        note(
            events,
            tick,
            dev,
            "dhcp",
            &format!(
                "DHCP {}/{} gw {} dns {}",
                ip_str(ip),
                mask_bits(mask),
                ip_str(gw),
                ip_str(dns)
            ),
        );
    }
}

fn alloc_lease(dev: &Device, my: [u8; 4], mask: [u8; 4]) -> Option<[u8; 4]> {
    let (start, end) = if let Some((a, b)) = dev.pool {
        (a, b)
    } else {
        let net = crate::net_of(my, mask);
        let mut s = net;
        s[3] = s[3].saturating_add(10);
        let mut e = net;
        // last usable-ish octet
        let wild = !mask[3];
        e[3] = if wild == 255 {
            250
        } else {
            net[3].saturating_add(wild).saturating_sub(1)
        };
        if e[3] < s[3] {
            e[3] = s[3];
        }
        (s, e)
    };
    let mut ip = start;
    loop {
        let used = dev.leases.values().any(|v| *v == ip)
            || dev.ports.iter().any(|p| p.ip.map(|(a, _)| a) == Some(ip));
        if !used {
            return Some(ip);
        }
        if ip == end {
            return None;
        }
        ip = inc_ip(ip);
        if ip == my {
            continue;
        }
    }
}

fn inc_ip(mut ip: [u8; 4]) -> [u8; 4] {
    for i in (0..4).rev() {
        if ip[i] != 255 {
            ip[i] += 1;
            return ip;
        }
        ip[i] = 0;
    }
    ip
}

struct DhcpMsg {
    op: u8,
    bootp: u8,
    xid: u32,
    yiaddr: [u8; 4],
    siaddr: [u8; 4],
    giaddr: [u8; 4],
    chaddr: [u8; 6],
    opts: Vec<(u8, Vec<u8>)>,
}

fn parse_dhcp(p: &[u8]) -> Option<DhcpMsg> {
    if p.len() < 240 {
        return None;
    }
    if p[236..240] != DHCP_MAGIC {
        return None;
    }
    let mut opts = Vec::new();
    let mut i = 240;
    let mut typ = 0u8;
    while i < p.len() {
        let code = p[i];
        if code == 255 {
            break;
        }
        if code == 0 {
            i += 1;
            continue;
        }
        if i + 1 >= p.len() {
            break;
        }
        let len = p[i + 1] as usize;
        if i + 2 + len > p.len() {
            break;
        }
        let val = p[i + 2..i + 2 + len].to_vec();
        if code == 53 && !val.is_empty() {
            typ = val[0];
        }
        opts.push((code, val));
        i += 2 + len;
    }
    if typ == 0 {
        return None;
    }
    Some(DhcpMsg {
        op: typ,
        bootp: p[0],
        xid: u32::from_be_bytes(p[4..8].try_into().ok()?),
        yiaddr: p[16..20].try_into().ok()?,
        siaddr: p[20..24].try_into().ok()?,
        giaddr: p[24..28].try_into().ok()?,
        chaddr: p[28..34].try_into().ok()?,
        opts,
    })
}

fn opt4(msg: &DhcpMsg, code: u8) -> Option<[u8; 4]> {
    msg.opts
        .iter()
        .find(|(c, _)| *c == code)
        .and_then(|(_, v)| v.as_slice().try_into().ok())
}

fn dhcp_packet(
    op: u8,
    xid: u32,
    mac: u64,
    yiaddr: [u8; 4],
    siaddr: [u8; 4],
    giaddr: [u8; 4],
    opts: &[(u8, Vec<u8>)],
) -> Vec<u8> {
    let mut p = vec![0u8; 240];
    p[0] = op; // 1 request, 2 reply
    p[1] = 1;
    p[2] = 6;
    p[4..8].copy_from_slice(&xid.to_be_bytes());
    p[10] = 0x80; // broadcast flag
    p[16..20].copy_from_slice(&yiaddr);
    p[20..24].copy_from_slice(&siaddr);
    p[24..28].copy_from_slice(&giaddr);
    p[28..34].copy_from_slice(&eth::mac_bytes(mac));
    p[236..240].copy_from_slice(&DHCP_MAGIC);
    for (c, v) in opts {
        p.push(*c);
        p.push(v.len() as u8);
        p.extend_from_slice(v);
    }
    p.push(255);
    if p.len() < 300 {
        p.resize(300, 0);
    }
    p
}

pub fn dhcp_state(dev: &Device) -> &'static str {
    match dev.dhcp.as_ref().map(|c| c.state) {
        Some(DhcpState::Discover) => "discover",
        Some(DhcpState::Request) => "request",
        Some(DhcpState::Bound) => "bound",
        _ => "",
    }
}
