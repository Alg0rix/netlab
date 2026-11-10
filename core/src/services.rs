//! DHCP, DNS, TCP/UDP/HTTP and source NAT for simulated hosts and routers.
//! Packets are real Ethernet/IPv4 frames so a Cisco IOS or RouterOS node on
//! the same link sees ordinary DHCP, DNS and TCP.

use crate::device::{Device, DeviceKind, NatRole, Route};
use crate::eth::{self, view_ip};
use crate::events::SimEvent;
use crate::frame::{Frame, Payload, BROADCAST_MAC};
use crate::ip_str;

const DHCP_DISCOVER: u8 = 1;
const DHCP_OFFER: u8 = 2;
const DHCP_REQUEST: u8 = 3;
const DHCP_ACK: u8 = 5;
const ST_DISCOVER: u8 = 1;
const ST_REQUEST: u8 = 2;
const ST_BOUND: u8 = 3;

const TCP_FIN: u8 = 0x01;
const TCP_SYN: u8 = 0x02;
const TCP_RST: u8 = 0x04;
const TCP_ACK: u8 = 0x10;
const TCP_PSH: u8 = 0x08;

#[derive(Clone, Debug)]
pub struct DhcpClient {
    pub port: usize,
    pub xid: u32,
    pub state: u8,
    pub offer: [u8; 4],
    pub server: [u8; 4],
    pub tries: u8,
    pub next: u64,
}

#[derive(Clone, Debug)]
pub struct Flow {
    pub kind: u8, // 1 tcp probe, 2 http, 3 udp, 4 tcp server
    pub lip: [u8; 4],
    pub lport: u16,
    pub rip: [u8; 4],
    pub rport: u16,
    pub state: u8, // 1 syn-sent, 2 established, 3 done
    pub seq: u32,
    pub ack: u32,
    pub tx: Vec<u8>,
    pub deadline: u64,
}

#[derive(Clone, Debug)]
pub struct NatEnt {
    pub proto: u8,
    pub inside_ip: [u8; 4],
    pub inside_port: u16,
    pub outside_port: u16,
    pub peer_ip: [u8; 4],
    pub peer_port: u16,
}

pub fn poll(dev: &mut Device, tick: u64, events: &mut Vec<SimEvent>) {
    if let Some(c) = dev.dhcp.as_ref() {
        if c.state < ST_BOUND && tick >= c.next && dev.powered {
            let port = c.port;
            let tries = c.tries;
            if tries >= 4 {
                dev.dhcp = None;
                note(events, tick, dev, "dhcp", "DHCP failed — no offer (is a server on this link?)");
            } else if c.state == ST_DISCOVER {
                send_discover(dev, port, tick, events);
            } else if c.state == ST_REQUEST {
                send_request(dev, port, tick, events);
            }
        }
    }
    let mut i = 0;
    while i < dev.flows.len() {
        if tick >= dev.flows[i].deadline && dev.flows[i].state != 3 {
            let f = dev.flows.remove(i);
            let msg = match f.kind {
                2 => format!("HTTP {}:{} timeout", ip_str(f.rip), f.rport),
                3 => format!("UDP {}:{} timeout", ip_str(f.rip), f.rport),
                _ => format!("TCP {}:{} timeout", ip_str(f.rip), f.rport),
            };
            let kind = match f.kind {
                2 => "http",
                3 => "udp",
                _ => "tcp",
            };
            note(events, tick, dev, kind, &msg);
        } else {
            i += 1;
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
        state: ST_DISCOVER,
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
    c.state = ST_DISCOVER;
    c.next = tick + 40;
    c.xid = xid;
    let mac = dev.ports[port].mac;
    let pkt = dhcp_packet(1, xid, mac, [0; 4], [0; 4], &[(53, vec![DHCP_DISCOVER]), (55, vec![1, 3, 6])]);
    let frame = udp_frame(dev, port, mac, BROADCAST_MAC, [0; 4], [255, 255, 255, 255], 68, 67, &pkt);
    note(events, tick, dev, "dhcp", &format!("DHCP discover on {}", port_label(dev, port)));
    dev.outbox.push((port, frame));
}

fn send_request(dev: &mut Device, port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    let Some(c) = dev.dhcp.clone() else { return };
    if let Some(dc) = dev.dhcp.as_mut() {
        dc.tries += 1;
        dc.state = ST_REQUEST;
        dc.next = tick + 40;
    }
    let mac = dev.ports[port].mac;
    let opts = vec![
        (53, vec![DHCP_REQUEST]),
        (50, c.offer.to_vec()),
        (54, c.server.to_vec()),
        (55, vec![1, 3, 6]),
    ];
    let pkt = dhcp_packet(1, c.xid, mac, [0; 4], [0; 4], &opts);
    let frame = udp_frame(dev, port, mac, BROADCAST_MAC, [0; 4], [255, 255, 255, 255], 68, 67, &pkt);
    note(events, tick, dev, "dhcp", &format!("DHCP request {}", ip_str(c.offer)));
    dev.outbox.push((port, frame));
}

pub fn start_dns(dev: &mut Device, name: &str, tick: u64, events: &mut Vec<SimEvent>) -> bool {
    let Some(dst) = dev.resolver else {
        note(events, tick, dev, "dns", "DNS failed — no server (DHCP, or `ip dns <addr>`)");
        return false;
    };
    let Some(src) = first_ip(dev) else {
        note(events, tick, dev, "dns", "DNS failed — no IPv4 address");
        return false;
    };
    let q = dns_query(name);
    let id = u16::from_be_bytes([q[0], q[1]]);
    let sport = alloc_port(dev);
    dev.flows.push(Flow {
        kind: 3,
        lip: src,
        lport: sport,
        rip: dst,
        rport: 53,
        state: 2,
        seq: id as u32,
        ack: 0,
        tx: name.as_bytes().to_vec(),
        deadline: tick + 4000,
    });
    let frame = Frame {
        src_mac: 0,
        dst_mac: 0,
        src_ip: src,
        dst_ip: dst,
        ttl: 64,
        payload: Payload::Raw { ethertype: 0x0800 },
        raw: None,
        icmp_echo: None,
    };
    // emit_ip fills MACs; attach UDP once we know the source mac inside a raw build after routing.
    // Build with a placeholder mac and let emit_ip / encode keep raw bytes — so the UDP must
    // already be addressed. Source MAC is rewritten by encode() from frame.src_mac.
    let mac = dev.ports.iter().find_map(|p| p.ip.map(|_| p.mac)).unwrap_or(0);
    let raw = eth::build_udp(mac, 0, src, dst, 64, dev.ip_id, sport, 53, &q);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    let mut frame = frame;
    frame.raw = Some(raw);
    dev.emit_ip(frame, tick, events);
    true
}

pub fn start_tcp(dev: &mut Device, dst: [u8; 4], dport: u16, http: bool, tick: u64, events: &mut Vec<SimEvent>) -> bool {
    let Some(src) = first_ip(dev) else {
        note(events, tick, dev, if http { "http" } else { "tcp" }, "no IPv4 address");
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
        kind: if http { 2 } else { 1 },
        lip: src,
        lport: sport,
        rip: dst,
        rport: dport,
        state: 1,
        seq,
        ack: 0,
        tx,
        deadline: tick + 4000,
    });
    send_tcp(dev, src, sport, dst, dport, seq, 0, TCP_SYN, &[], tick, events);
    true
}

pub fn start_udp(dev: &mut Device, dst: [u8; 4], dport: u16, payload: &[u8], tick: u64, events: &mut Vec<SimEvent>) -> bool {
    let Some(src) = first_ip(dev) else {
        note(events, tick, dev, "udp", "no IPv4 address");
        return false;
    };
    let sport = alloc_port(dev);
    dev.flows.push(Flow {
        kind: 3,
        lip: src,
        lport: sport,
        rip: dst,
        rport: dport,
        state: 2,
        seq: 0,
        ack: 0,
        tx: payload.to_vec(),
        deadline: tick + 4000,
    });
    let mac = dev.ports.iter().find_map(|p| p.ip.map(|_| p.mac)).unwrap_or(0);
    let raw = eth::build_udp(mac, 0, src, dst, 64, dev.ip_id, sport, dport, payload);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.emit_ip(
        Frame {
            src_mac: 0,
            dst_mac: 0,
            src_ip: src,
            dst_ip: dst,
            ttl: 64,
            payload: Payload::Raw { ethertype: 0x0800 },
            raw: Some(raw),
            icmp_echo: None,
        },
        tick,
        events,
    );
    true
}

/// True when the frame was translated toward an inside host and should be forwarded.
pub fn nat_ingress(dev: &mut Device, frame: &mut Frame, in_port: usize) -> bool {
    if dev.kind != DeviceKind::Router || in_port >= dev.ports.len() {
        return false;
    }
    if dev.ports[in_port].nat != NatRole::Outside {
        return false;
    }
    let Some((outside_ip, _)) = dev.ports[in_port].ip else { return false };
    if frame.dst_ip != outside_ip {
        return false;
    }
    let (proto, sport, dport, icmp_id) = l4_tuple(frame);
    if proto == 0 {
        return false;
    }
    let key_port = if proto == 1 { icmp_id } else { dport };
    let peer_port = if proto == 1 { 0 } else { sport };
    let ent = dev.nat.iter().find(|e| {
        e.proto == proto && e.outside_port == key_port && e.peer_ip == frame.src_ip && e.peer_port == peer_port
    });
    let Some(ent) = ent.cloned() else { return false };
    frame.dst_ip = ent.inside_ip;
    if let Some(raw) = frame.raw.as_mut() {
        if proto == 1 {
            eth::sync_ipv4(raw, frame.src_ip, frame.dst_ip, frame.ttl, None, None, Some(ent.inside_port));
        } else {
            eth::sync_ipv4(raw, frame.src_ip, frame.dst_ip, frame.ttl, None, Some(ent.inside_port), None);
        }
    } else if proto == 1 {
        let id = ent.inside_port.to_be_bytes();
        let mut echo = frame.icmp_echo.clone().unwrap_or_else(|| vec![0, 0, 0, 1]);
        if echo.len() >= 2 {
            echo[0] = id[0];
            echo[1] = id[1];
        }
        frame.icmp_echo = Some(echo);
    }
    true
}

/// Masquerade packets that are about to leave via an outside interface.
pub fn nat_egress(dev: &mut Device, frame: &mut Frame, in_port: Option<usize>) {
    if dev.kind != DeviceKind::Router {
        return;
    }
    let Some(route) = lookup_out(dev, frame.dst_ip) else { return };
    let out = route;
    if dev.ports.get(out).map(|p| p.nat) != Some(NatRole::Outside) {
        return;
    }
    let explicit_inside = dev.ports.iter().any(|p| p.nat == NatRole::Inside);
    let from_inside = match in_port {
        Some(p) => {
            let role = dev.ports.get(p).map(|p| p.nat).unwrap_or(NatRole::Off);
            role == NatRole::Inside || (!explicit_inside && role != NatRole::Outside)
        }
        None => false,
    };
    if !from_inside {
        return;
    }
    let Some((outside_ip, _)) = dev.ports[out].ip else { return };
    let (proto, sport, dport, icmp_id) = l4_tuple(frame);
    if proto != 1 && proto != 6 && proto != 17 {
        return;
    }
    let inside_port = if proto == 1 { icmp_id } else { sport };
    let peer_port = if proto == 1 { 0 } else { dport };
    let mut outside_port = inside_port;
    let clash = dev.nat.iter().any(|e| {
        e.proto == proto
            && e.outside_port == outside_port
            && e.peer_ip == frame.dst_ip
            && e.peer_port == peer_port
            && e.inside_ip != frame.src_ip
    });
    if clash {
        outside_port = 40_000 + (dev.ip_id % 20_000) as u16;
        dev.ip_id = dev.ip_id.wrapping_add(1);
    }
    dev.nat.retain(|e| {
        !(e.proto == proto && e.inside_ip == frame.src_ip && e.inside_port == inside_port && e.peer_ip == frame.dst_ip && e.peer_port == peer_port)
    });
    dev.nat.push(NatEnt {
        proto,
        inside_ip: frame.src_ip,
        inside_port,
        outside_port,
        peer_ip: frame.dst_ip,
        peer_port,
    });
    if dev.nat.len() > 256 {
        dev.nat.drain(0..64);
    }
    frame.src_ip = outside_ip;
    if let Some(raw) = frame.raw.as_mut() {
        if proto == 1 {
            eth::sync_ipv4(raw, frame.src_ip, frame.dst_ip, frame.ttl, None, None, Some(outside_port));
        } else {
            eth::sync_ipv4(raw, frame.src_ip, frame.dst_ip, frame.ttl, Some(outside_port), None, None);
        }
    } else if proto == 1 && outside_port != inside_port {
        let id = outside_port.to_be_bytes();
        let mut echo = frame.icmp_echo.clone().unwrap_or_else(|| vec![0, 0, 0, 1]);
        if echo.len() >= 2 {
            echo[0] = id[0];
            echo[1] = id[1];
        }
        frame.icmp_echo = Some(echo);
    }
}

/// Local DHCP / DNS / TCP / UDP handling. True = do not also route the frame.
pub fn local_l4(dev: &mut Device, frame: &Frame, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) -> bool {
    let Some(raw) = frame.raw.as_deref() else { return false };
    let Some(v) = view_ip(raw) else { return false };
    let bcast = frame.dst_ip == [255, 255, 255, 255] || frame.dst_mac == BROADCAST_MAC;
    let mine = dev.ports.iter().any(|p| p.ip.map(|(a, _)| a) == Some(frame.dst_ip));
    if v.proto == 17 && (v.dport == 67 || v.dport == 68) {
        on_dhcp(dev, frame, raw, in_port, tick, events);
        return true;
    }
    if bcast && !mine {
        return dev.kind != DeviceKind::Router;
    }
    if !mine {
        return false;
    }
    if v.proto == 17 && v.dport == 53 && dev.svc_dns {
        on_dns(dev, frame, raw, v, in_port, tick, events);
        return true;
    }
    if v.proto == 17 {
        on_udp(dev, frame, raw, v, in_port, tick, events);
        return true;
    }
    if v.proto == 6 {
        on_tcp(dev, frame, raw, v, in_port, tick, events);
        return true;
    }
    if v.proto == 1 && v.icmp_type == 3 {
        on_unreach(dev, frame, tick, events);
        return true;
    }
    false
}

fn on_dhcp(dev: &mut Device, frame: &Frame, raw: &[u8], in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    let Some(v) = view_ip(raw) else { return };
    let payload = &raw[v.payload.min(raw.len())..];
    let Some(msg) = parse_dhcp(payload) else { return };
    if v.dport == 67 && dev.svc_dhcp {
        serve_dhcp(dev, &msg, in_port, tick, events);
    }
    if v.dport == 68 {
        if let Some(c) = dev.dhcp.clone() {
            if msg.xid == c.xid && msg.chaddr == mac6(dev.ports[c.port].mac) {
                client_dhcp(dev, &msg, tick, events);
            }
        }
    }
    let _ = frame;
}

fn serve_dhcp(dev: &mut Device, msg: &DhcpMsg, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    if msg.op != DHCP_DISCOVER && msg.op != DHCP_REQUEST {
        return;
    }
    let port = dev
        .ports
        .iter()
        .position(|p| p.ip.is_some() && p.link.is_some())
        .unwrap_or(in_port);
    let Some((my_ip, mask)) = dev.ports.get(port).and_then(|p| p.ip) else { return };
    let mac = u64_mac(&msg.chaddr);
    let ip = if let Some(ip) = dev.leases.get(&mac).copied() {
        ip
    } else if let Some(ip) = alloc_lease(dev, port, my_ip, mask) {
        dev.leases.insert(mac, ip);
        ip
    } else {
        return;
    };
    let typ = if msg.op == DHCP_DISCOVER { DHCP_OFFER } else { DHCP_ACK };
    let mut opts = vec![
        (53, vec![typ]),
        (54, my_ip.to_vec()),
        (1, mask.to_vec()),
        (51, 3600u32.to_be_bytes().to_vec()),
    ];
    let gw = dev.ports[port].ip.map(|(a, _)| a).unwrap_or(my_ip);
    opts.push((3, gw.to_vec()));
    let dns = dev.resolver.unwrap_or(my_ip);
    opts.push((6, dns.to_vec()));
    let pkt = dhcp_packet(2, msg.xid, mac, ip, my_ip, &opts);
    let smac = dev.ports[port].mac;
    let frame = udp_frame(dev, port, smac, BROADCAST_MAC, my_ip, [255, 255, 255, 255], 67, 68, &pkt);
    let word = if typ == DHCP_OFFER { "offer" } else { "ack" };
    note(events, tick, dev, "dhcp", &format!("DHCP {word} {} to {}", ip_str(ip), crate::mac_str(mac)));
    dev.outbox.push((port, frame));
}

fn client_dhcp(dev: &mut Device, msg: &DhcpMsg, tick: u64, events: &mut Vec<SimEvent>) {
    let Some(c) = dev.dhcp.as_mut() else { return };
    if msg.op == DHCP_OFFER && c.state == ST_DISCOVER {
        c.offer = msg.yiaddr;
        c.server = opt4(msg, 54).unwrap_or(msg.siaddr);
        c.state = ST_REQUEST;
        c.tries = 0;
        c.next = tick;
        return;
    }
    if msg.op == DHCP_ACK && (c.state == ST_REQUEST || c.state == ST_DISCOVER) {
        let port = c.port;
        let ip = msg.yiaddr;
        let mask = opt4(msg, 1).unwrap_or([255, 255, 255, 0]);
        let gw = opt4(msg, 3).unwrap_or([0; 4]);
        let dns = opt4(msg, 6).unwrap_or([0; 4]);
        c.state = ST_BOUND;
        c.next = u64::MAX;
        if port < dev.ports.len() {
            dev.ports[port].ip = Some((ip, mask));
        }
        if gw != [0; 4] {
            dev.routes.retain(|r| r.prefix != [0, 0, 0, 0]);
            dev.routes.push(Route {
                prefix: [0; 4],
                mask: [0; 4],
                next_hop: gw,
                out_port: port,
                connected: false,
            });
        }
        if dns != [0; 4] {
            dev.resolver = Some(dns);
        }
        note(
            events,
            tick,
            dev,
            "dhcp",
            &format!("DHCP {}/{} gw {} dns {}", ip_str(ip), mask_len(mask), ip_str(gw), ip_str(dns)),
        );
    }
}

fn on_dns(dev: &mut Device, frame: &Frame, raw: &[u8], v: eth::IpView, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
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
        .or_else(|| first_ip(dev));
    let answer = match ip {
        Some(ip) => dns_response(payload, &name, ip, false),
        None => dns_response(payload, &name, [0; 4], true),
    };
    let port = in_port.min(dev.ports.len().saturating_sub(1));
    let smac = dev.ports[port].mac;
    let src = dev.ports[port].ip.map(|(a, _)| a).unwrap_or(frame.dst_ip);
    let bytes = eth::build_udp(smac, frame.src_mac, src, frame.src_ip, 64, dev.ip_id, 53, v.sport, &answer);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.outbox.push((
        port,
        Frame {
            src_mac: smac,
            dst_mac: frame.src_mac,
            src_ip: src,
            dst_ip: frame.src_ip,
            ttl: 64,
            payload: Payload::Raw { ethertype: 0x0800 },
            raw: Some(bytes),
            icmp_echo: None,
        },
    ));
    note(events, tick, dev, "dns", &format!("DNS {name} → {}", ip.map(ip_str).unwrap_or_else(|| "NXDOMAIN".into())));
}

fn on_udp(dev: &mut Device, frame: &Frame, raw: &[u8], v: eth::IpView, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    if let Some(i) = dev.flows.iter().position(|f| {
        f.kind == 3 && f.lport == v.dport && f.rport == v.sport && f.rip == frame.src_ip
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
            note(events, tick, dev, "udp", &format!("UDP from {}:{} {}", ip_str(frame.src_ip), v.sport, preview));
        }
        return;
    }
    // closed UDP port → ICMP unreachable so `nc` can tell drop from closed
    if dev.kind == DeviceKind::Host {
        send_unreach(dev, frame, raw, in_port, tick);
        note(events, tick, dev, "udp", &format!("UDP {}:{} closed", ip_str(frame.dst_ip), v.dport));
    }
}

fn on_unreach(dev: &mut Device, frame: &Frame, tick: u64, events: &mut Vec<SimEvent>) {
    if let Some(i) = dev.flows.iter().position(|f| f.rip == frame.src_ip && f.state != 3) {
        let f = dev.flows.remove(i);
        let kind = match f.kind {
            2 => "http",
            3 => "udp",
            _ => "tcp",
        };
        note(events, tick, dev, kind, &format!("{} {}:{} unreachable", kind.to_ascii_uppercase(), ip_str(f.rip), f.rport));
    }
}

fn on_tcp(dev: &mut Device, frame: &Frame, raw: &[u8], v: eth::IpView, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    let l4 = v.off + v.ihl;
    if raw.len() < l4 + 20 {
        return;
    }
    let seq = u32::from_be_bytes(raw[l4 + 4..l4 + 8].try_into().unwrap());
    let ack_n = u32::from_be_bytes(raw[l4 + 8..l4 + 12].try_into().unwrap());
    let flags = raw[l4 + 13];
    let doff = (raw[l4 + 12] >> 4) as usize * 4;
    let ip_pay_end = (v.off + u16::from_be_bytes([raw[v.off + 2], raw[v.off + 3]]) as usize).min(raw.len());
    let data = if l4 + doff < ip_pay_end { &raw[l4 + doff..ip_pay_end] } else { &[] };

    if let Some(i) = dev.flows.iter().position(|f| {
        f.kind != 4 && f.lport == v.dport && f.rport == v.sport && f.rip == frame.src_ip && f.lip == frame.dst_ip
    }) {
        client_tcp(dev, i, flags, seq, ack_n, data, in_port, tick, events);
        return;
    }
    if flags & TCP_SYN != 0 && flags & TCP_ACK == 0 {
        let listen = (dev.svc_http && v.dport == 80) || dev.listen.contains(&v.dport);
        if listen {
            let isn = next_id(dev);
            dev.flows.push(Flow {
                kind: 4,
                lip: frame.dst_ip,
                lport: v.dport,
                rip: frame.src_ip,
                rport: v.sport,
                state: 2,
                seq: isn.wrapping_add(1),
                ack: seq.wrapping_add(1),
                tx: Vec::new(),
                deadline: tick + 4000,
            });
            send_tcp(dev, frame.dst_ip, v.dport, frame.src_ip, v.sport, isn, seq.wrapping_add(1), TCP_SYN | TCP_ACK, &[], tick, events);
        } else if dev.kind == DeviceKind::Host {
            send_tcp(dev, frame.dst_ip, v.dport, frame.src_ip, v.sport, 0, seq.wrapping_add(1), TCP_RST | TCP_ACK, &[], tick, events);
            note(events, tick, dev, "tcp", &format!("TCP {}:{} closed", ip_str(frame.dst_ip), v.dport));
        }
    } else if let Some(i) = dev.flows.iter().position(|f| {
        f.kind == 4 && f.lport == v.dport && f.rport == v.sport && f.rip == frame.src_ip
    }) {
        server_tcp(dev, i, flags, seq, data, tick, events);
    }
}

fn client_tcp(dev: &mut Device, i: usize, flags: u8, seq: u32, _ack: u32, data: &[u8], _in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    let f = dev.flows[i].clone();
    if flags & TCP_RST != 0 {
        dev.flows.remove(i);
        let kind = if f.kind == 2 { "http" } else { "tcp" };
        note(events, tick, dev, kind, &format!("TCP {}:{} closed", ip_str(f.rip), f.rport));
        return;
    }
    if f.state == 1 && flags & TCP_SYN != 0 && flags & TCP_ACK != 0 {
        let next = f.seq.wrapping_add(1);
        if f.kind == 1 && f.tx.is_empty() {
            send_tcp(dev, f.lip, f.lport, f.rip, f.rport, next, seq.wrapping_add(1), TCP_ACK, &[], tick, events);
            dev.flows.remove(i);
            note(events, tick, dev, "tcp", &format!("TCP {}:{} open", ip_str(f.rip), f.rport));
            return;
        }
        dev.flows[i].state = 2;
        dev.flows[i].seq = next.wrapping_add(f.tx.len() as u32);
        dev.flows[i].ack = seq.wrapping_add(1);
        dev.flows[i].deadline = tick + 4000;
        let tx = f.tx.clone();
        send_tcp(dev, f.lip, f.lport, f.rip, f.rport, next, seq.wrapping_add(1), TCP_ACK | TCP_PSH, &tx, tick, events);
        return;
    }
    if !data.is_empty() {
        let text = String::from_utf8_lossy(data);
        dev.flows.remove(i);
        if f.kind == 2 {
            let status = text.split_whitespace().nth(1).unwrap_or("?");
            note(events, tick, dev, "http", &format!("HTTP {status} from {}:{} ({} bytes)", ip_str(f.rip), f.rport, data.len()));
        } else {
            note(events, tick, dev, "tcp", &format!("TCP {}:{} data ({} bytes)", ip_str(f.rip), f.rport, data.len()));
        }
    }
}

fn server_tcp(dev: &mut Device, i: usize, flags: u8, seq: u32, data: &[u8], tick: u64, events: &mut Vec<SimEvent>) {
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
    let ack = seq.wrapping_add(data.len() as u32).wrapping_add(if flags & TCP_FIN != 0 { 1 } else { 0 });
    send_tcp(dev, f.lip, f.lport, f.rip, f.rport, f.seq, ack, TCP_ACK | TCP_PSH | TCP_FIN, &bytes, tick, events);
    dev.flows.remove(i);
    note(events, tick, dev, "http", &format!("HTTP 200 to {}:{}", ip_str(f.rip), f.rport));
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
    let mac = dev.ports.iter().find_map(|p| p.ip.filter(|(a, _)| *a == src).map(|_| p.mac)).unwrap_or(0);
    // find mac of the addressed port
    let mac = dev
        .ports
        .iter()
        .find_map(|p| p.ip.filter(|(a, _)| *a == src).map(|_| p.mac))
        .unwrap_or(mac);
    let raw = eth::build_tcp(mac, 0, src, dst, 64, dev.ip_id, sport, dport, seq, ack, flags, payload);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.emit_ip(
        Frame {
            src_mac: mac,
            dst_mac: 0,
            src_ip: src,
            dst_ip: dst,
            ttl: 64,
            payload: Payload::Raw { ethertype: 0x0800 },
            raw: Some(raw),
            icmp_echo: None,
        },
        tick,
        events,
    );
}

fn send_unreach(dev: &mut Device, frame: &Frame, raw: &[u8], in_port: usize, tick: u64) {
    let Some(v) = view_ip(raw) else { return };
    let src = dev.ports.get(in_port).and_then(|p| p.ip.map(|(a, _)| a)).unwrap_or(frame.dst_ip);
    let smac = dev.ports.get(in_port).map(|p| p.mac).unwrap_or(0);
    // ICMP dest unreachable, code 3, quoting the original IP header + 8 bytes
    let quote_end = (v.off + v.ihl + 8).min(raw.len());
    let quoted = &raw[v.off..quote_end];
    let mut icmp = vec![3, 3, 0, 0, 0, 0, 0, 0];
    icmp.extend_from_slice(quoted);
    let c = eth::cksum(&icmp);
    icmp[2] = (c >> 8) as u8;
    icmp[3] = c as u8;
    let bytes = eth::build_udp(smac, frame.src_mac, src, frame.src_ip, 64, dev.ip_id, 0, 0, &[]); // placeholder, replaced below
    let _ = bytes;
    // Build a real ICMP packet with the same Ethernet/IPv4 layout as build_udp's header section.
    let mut out = Vec::new();
    out.extend_from_slice(&eth::mac_bytes(frame.src_mac));
    out.extend_from_slice(&eth::mac_bytes(smac));
    out.extend_from_slice(&0x0800u16.to_be_bytes());
    let total = 20 + icmp.len();
    out.push(0x45);
    out.push(0);
    out.extend_from_slice(&(total as u16).to_be_bytes());
    out.extend_from_slice(&dev.ip_id.to_be_bytes());
    out.extend_from_slice(&0x4000u16.to_be_bytes());
    out.push(64);
    out.push(1);
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&src);
    out.extend_from_slice(&frame.src_ip);
    let c = eth::cksum(&out[14..34]);
    out[24] = (c >> 8) as u8;
    out[25] = c as u8;
    out.extend_from_slice(&icmp);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.outbox.push((
        in_port,
        Frame {
            src_mac: smac,
            dst_mac: frame.src_mac,
            src_ip: src,
            dst_ip: frame.src_ip,
            ttl: 64,
            payload: Payload::Raw { ethertype: 0x0800 },
            raw: Some(out),
            icmp_echo: None,
        },
    ));
    let _ = tick;
}

fn alloc_lease(dev: &Device, port: usize, my: [u8; 4], mask: [u8; 4]) -> Option<[u8; 4]> {
    let (start, end) = if let Some((a, b)) = dev.pool {
        (a, b)
    } else {
        let net = crate::net_of(my, mask);
        let mut s = net;
        s[3] = s[3].saturating_add(10);
        let mut e = net;
        // last usable-ish octet
        let wild = !mask[3];
        e[3] = if wild == 255 { 250 } else { net[3].saturating_add(wild).saturating_sub(1) };
        if e[3] < s[3] {
            e[3] = s[3];
        }
        (s, e)
    };
    let mut ip = start;
    loop {
        let used = dev.leases.values().any(|v| *v == ip) || dev.ports.iter().any(|p| p.ip.map(|(a, _)| a) == Some(ip));
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
        let _ = port;
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
    xid: u32,
    yiaddr: [u8; 4],
    siaddr: [u8; 4],
    chaddr: [u8; 6],
    opts: Vec<(u8, Vec<u8>)>,
}

fn parse_dhcp(p: &[u8]) -> Option<DhcpMsg> {
    if p.len() < 240 {
        return None;
    }
    if p[236..240] != [99, 130, 83, 99] {
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
        xid: u32::from_be_bytes(p[4..8].try_into().ok()?),
        yiaddr: p[16..20].try_into().ok()?,
        siaddr: p[20..24].try_into().ok()?,
        chaddr: p[28..34].try_into().ok()?,
        opts,
    })
}

fn opt4(msg: &DhcpMsg, code: u8) -> Option<[u8; 4]> {
    msg.opts.iter().find(|(c, _)| *c == code).and_then(|(_, v)| v.as_slice().try_into().ok())
}

fn dhcp_packet(op: u8, xid: u32, mac: u64, yiaddr: [u8; 4], siaddr: [u8; 4], opts: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let mut p = vec![0u8; 240];
    p[0] = op; // 1 request, 2 reply
    p[1] = 1;
    p[2] = 6;
    p[4..8].copy_from_slice(&xid.to_be_bytes());
    p[10] = 0x80; // broadcast flag
    p[16..20].copy_from_slice(&yiaddr);
    p[20..24].copy_from_slice(&siaddr);
    p[28..34].copy_from_slice(&mac6(mac));
    p[236..240].copy_from_slice(&[99, 130, 83, 99]);
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

fn dns_query(name: &str) -> Vec<u8> {
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

fn dns_response(query: &[u8], name: &str, ip: [u8; 4], nx: bool) -> Vec<u8> {
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
    // pointer to name at offset 12
    r.extend_from_slice(&[0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 60, 0, 4]);
    r.extend_from_slice(&ip);
    let _ = name;
    r
}

fn dns_answer(msg: &[u8]) -> Option<(String, [u8; 4], bool)> {
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
    // skip question
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
        i += 5; // 0 + type + class
    }
    if msg.len() >= i + 16 && u16::from_be_bytes([msg[i + 2], msg[i + 3]]) == 1 {
        let ip = [msg[i + 12], msg[i + 13], msg[i + 14], msg[i + 15]];
        return Some((name, ip, false));
    }
    // fallback: if ancount > 0 take the last 4 bytes
    if msg.len() >= 16 && u16::from_be_bytes([msg[6], msg[7]]) > 0 {
        let n = msg.len();
        return Some((name, [msg[n - 4], msg[n - 3], msg[n - 2], msg[n - 1]], false));
    }
    None
}

fn udp_frame(dev: &mut Device, _port: usize, smac: u64, dmac: u64, src: [u8; 4], dst: [u8; 4], sport: u16, dport: u16, payload: &[u8]) -> Frame {
    let raw = eth::build_udp(smac, dmac, src, dst, 64, dev.ip_id, sport, dport, payload);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    Frame {
        src_mac: smac,
        dst_mac: dmac,
        src_ip: src,
        dst_ip: dst,
        ttl: 64,
        payload: Payload::Raw { ethertype: 0x0800 },
        raw: Some(raw),
        icmp_echo: None,
    }
}

fn l4_tuple(frame: &Frame) -> (u8, u16, u16, u16) {
    if let Some(raw) = &frame.raw {
        if let Some(v) = view_ip(raw) {
            return (v.proto, v.sport, v.dport, v.icmp_id);
        }
    }
    match frame.payload {
        Payload::IcmpEchoRequest | Payload::IcmpEchoReply => {
            let id = frame.icmp_echo.as_ref().filter(|e| e.len() >= 2).map(|e| u16::from_be_bytes([e[0], e[1]])).unwrap_or(0);
            (1, 0, 0, id)
        }
        _ => (0, 0, 0, 0),
    }
}

fn lookup_out(dev: &Device, dst: [u8; 4]) -> Option<usize> {
    // mirror Device::lookup's port choice without making it public twice
    let mut best: Option<(u32, usize)> = None;
    for (i, p) in dev.ports.iter().enumerate() {
        if let Some((addr, mask)) = p.ip {
            if crate::net_of(dst, mask) == crate::net_of(addr, mask) {
                let bits = crate::mask_bits(mask);
                if best.map(|(b, _)| b < bits).unwrap_or(true) {
                    best = Some((bits, i));
                }
            }
        }
    }
    for r in &dev.routes {
        if crate::net_of(dst, r.mask) == crate::net_of(r.prefix, r.mask) {
            let bits = crate::mask_bits(r.mask);
            if best.map(|(b, _)| b <= bits).unwrap_or(true) {
                best = Some((bits, r.out_port));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn first_ip(dev: &Device) -> Option<[u8; 4]> {
    dev.ports.iter().find_map(|p| p.ip.map(|(a, _)| a))
}

fn alloc_port(dev: &mut Device) -> u16 {
    let p = 40_000 + (dev.ip_id % 20_000) as u16;
    dev.ip_id = dev.ip_id.wrapping_add(1);
    p
}

fn next_id(dev: &mut Device) -> u32 {
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.ip_id as u32
}

fn mac6(m: u64) -> [u8; 6] {
    eth::mac_bytes(m)
}

fn u64_mac(m: &[u8; 6]) -> u64 {
    m.iter().fold(0u64, |a, &b| (a << 8) | b as u64)
}

fn mask_len(mask: [u8; 4]) -> u32 {
    crate::mask_bits(mask)
}

fn port_label(dev: &Device, port: usize) -> String {
    format!("port {port} ({})", dev.name)
}

fn note(events: &mut Vec<SimEvent>, tick: u64, dev: &Device, kind: &'static str, msg: &str) {
    events.push(SimEvent::new(tick, Some(dev.id), None, kind, format!("{}: {msg}", dev.name)));
}

pub fn dhcp_state(dev: &Device) -> &'static str {
    match dev.dhcp.as_ref().map(|c| c.state) {
        Some(ST_DISCOVER) => "discover",
        Some(ST_REQUEST) => "request",
        Some(ST_BOUND) => "bound",
        _ => "",
    }
}
