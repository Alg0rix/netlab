use std::collections::HashMap;

use crate::events::SimEvent;
use crate::frame::{Frame, Payload, BROADCAST_MAC};
use crate::{ip_str, mac_str, mask_bits, net_of, LinkId};

pub type DeviceId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Host,
    Switch,
    Router,
    /// External NIO bridge — each port maps to a real-Ethernet interface on an
    /// emulated node (Dynamips/v86). Frames arriving on a port are encoded to
    /// wire bytes for JS; wire bytes injected by JS are pushed onto the link.
    Ext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatRole {
    Off,
    Inside,
    Outside,
}

#[derive(Debug)]
pub struct Port {
    pub link: Option<LinkId>,
    pub mac: u64,
    /// (address, mask) — L3 ports only
    pub ip: Option<([u8; 4], [u8; 4])>,
    pub nat: NatRole,
    /// Access VLAN. Every port starts as access VLAN 1.
    pub access_vlan: u16,
    pub trunk: bool,
    pub native_vlan: u16,
    /// None means every VLAN is allowed on a trunk.
    pub allowed: Option<Vec<u16>>,
    pub in_acl: Option<u16>,
    pub out_acl: Option<u16>,
    /// DHCP relay target. Limited broadcasts are not routed unless this is set.
    pub helper: Option<[u8; 4]>,
}

/// One extended ACL entry. `proto` 0 matches any. Wildcards are Cisco-style.
#[derive(Debug, Clone)]
pub struct Ace {
    pub permit: bool,
    pub proto: u8,
    pub src: [u8; 4],
    pub src_wild: [u8; 4],
    pub dst: [u8; 4],
    pub dst_wild: [u8; 4],
    pub sport: Option<u16>,
    pub dport: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct Acl {
    pub num: u16,
    pub entries: Vec<Ace>,
}

#[derive(Debug, Clone, Copy)]
pub struct Route {
    pub prefix: [u8; 4],
    pub mask: [u8; 4],
    /// ignored for connected routes
    pub next_hop: [u8; 4],
    pub out_port: usize,
    pub connected: bool,
    /// 0 = static, 1 = OSPF. Connected routes always beat both.
    pub proto: u8,
}

/// Frame held back while ARP resolves `resolve`.
#[derive(Debug)]
pub struct Pending {
    pub frame: Frame,
    pub out_port: usize,
    pub resolve: [u8; 4],
}

#[derive(Debug)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    pub x: f64,
    pub y: f64,
    pub ports: Vec<Port>,
    /// switch: (mac, vlan) -> port index
    pub mac_table: HashMap<(u64, u16), usize>,
    pub acls: Vec<Acl>,
    pub ospf: crate::ospf::Ospf,
    pub arp_cache: HashMap<[u8; 4], u64>,
    pub routes: Vec<Route>,
    pub pending: Vec<Pending>,
    /// frames to push onto the wire at the end of the tick
    pub outbox: Vec<(usize, Frame)>,
    /// ext: encoded wire bytes queued for the JS bridge — (port, eth frame)
    pub ext_out: Vec<(usize, Vec<u8>)>,
    /// false = node stopped: drops everything it receives, sends nothing
    pub powered: bool,
    pub ip_id: u16,
    pub svc_dhcp: bool,
    pub svc_dns: bool,
    pub svc_http: bool,
    /// extra TCP ports that complete a handshake (firewall probes)
    pub listen: Vec<u16>,
    pub resolver: Option<[u8; 4]>,
    pub dns_records: Vec<(String, [u8; 4])>,
    /// optional DHCP pool (start, end); otherwise derived from the server address
    pub pool: Option<([u8; 4], [u8; 4])>,
    pub leases: HashMap<u64, [u8; 4]>,
    pub dhcp: Option<crate::services::DhcpClient>,
    pub flows: Vec<crate::services::Flow>,
    pub nat: Vec<crate::services::NatEnt>,
}

impl Device {
    pub fn new(id: DeviceId, name: String, kind: DeviceKind, x: f64, y: f64, macs: Vec<u64>) -> Self {
        Self {
            id,
            name,
            kind,
            x,
            y,
            ports: macs
                .into_iter()
                .map(|mac| Port {
                    link: None,
                    mac,
                    ip: None,
                    nat: NatRole::Off,
                    access_vlan: 1,
                    trunk: false,
                    native_vlan: 1,
                    allowed: None,
                    in_acl: None,
                    out_acl: None,
                    helper: None,
                })
                .collect(),
            mac_table: HashMap::new(),
            arp_cache: HashMap::new(),
            routes: Vec::new(),
            pending: Vec::new(),
            outbox: Vec::new(),
            ext_out: Vec::new(),
            powered: true,
            ip_id: 1,
            svc_dhcp: false,
            svc_dns: false,
            svc_http: false,
            listen: Vec::new(),
            resolver: None,
            dns_records: Vec::new(),
            pool: None,
            leases: HashMap::new(),
            dhcp: None,
            flows: Vec::new(),
            nat: Vec::new(),
            acls: Vec::new(),
            ospf: crate::ospf::Ospf::default(),
        }
    }

    pub fn free_port(&self) -> Option<usize> {
        self.ports.iter().position(|p| p.link.is_none())
    }

    fn owns_ip(&self, ip: [u8; 4]) -> bool {
        self.ports.iter().any(|p| p.ip.map(|(a, _)| a) == Some(ip))
    }

    /// Longest prefix, then connected, then static (proto 0) over OSPF (proto 1).
    pub(crate) fn lookup(&self, dst: [u8; 4]) -> Option<Route> {
        let mut best: Option<Route> = None;
        let take = |cand: &Route, best: &Route| {
            let (cb, bb) = (mask_bits(cand.mask), mask_bits(best.mask));
            if cb != bb {
                return cb > bb;
            }
            if cand.connected != best.connected {
                return cand.connected;
            }
            cand.proto <= best.proto
        };
        for (i, p) in self.ports.iter().enumerate() {
            if let Some((addr, mask)) = p.ip {
                if net_of(dst, mask) == net_of(addr, mask) {
                    let r = Route {
                        prefix: net_of(addr, mask),
                        mask,
                        next_hop: dst,
                        out_port: i,
                        connected: true,
                        proto: 0,
                    };
                    if best.as_ref().map(|b| take(&r, b)).unwrap_or(true) {
                        best = Some(r);
                    }
                }
            }
        }
        for r in &self.routes {
            if net_of(dst, r.mask) == net_of(r.prefix, r.mask)
                && best.as_ref().map(|b| take(r, b)).unwrap_or(true)
            {
                best = Some(*r);
            }
        }
        best
    }

    /// Send an IP frame toward `dst_ip`; ARP-resolves the next hop or queues it.
    pub fn emit_ip(&mut self, mut frame: Frame, tick: u64, events: &mut Vec<SimEvent>) {
        let Some(route) = self.lookup(frame.dst_ip) else {
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                None,
                "drop",
                format!("{}: no route to {}", self.name, ip_str(frame.dst_ip)),
            ));
            return;
        };
        let next_hop = if route.connected {
            frame.dst_ip
        } else {
            route.next_hop
        };
        if !ip_allowed(self, self.ports[route.out_port].out_acl, &frame) {
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                self.ports[route.out_port].link,
                "drop",
                format!("{}: ACL denied {}", self.name, frame.label()),
            ));
            return;
        }
        let port = &self.ports[route.out_port];
        frame.src_mac = port.mac;
        if frame.src_ip == [0, 0, 0, 0] {
            if let Some((addr, _)) = port.ip {
                frame.src_ip = addr;
            }
        }
        if let Some(&mac) = self.arp_cache.get(&next_hop) {
            frame.dst_mac = mac;
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                port.link,
                "tx",
                format!("{}: {} via {}", self.name, frame.label(), mac_str(mac)),
            ));
            self.outbox.push((route.out_port, frame));
        } else {
            let arp = Frame {
                src_mac: port.mac,
                dst_mac: BROADCAST_MAC,
                src_ip: port.ip.map(|(a, _)| a).unwrap_or(frame.src_ip),
                dst_ip: next_hop,
                ttl: 1,
                payload: Payload::ArpRequest { target_ip: next_hop },
                raw: None,
                icmp_echo: None, vlan: 0,
            };
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                port.link,
                "arp_req",
                format!("{}: ARP who-has {}", self.name, ip_str(next_hop)),
            ));
            self.pending.push(Pending {
                frame,
                out_port: route.out_port,
                resolve: next_hop,
            });
            self.outbox.push((route.out_port, arp));
        }
    }

    /// A frame arrived on `in_port`.
    pub fn receive(&mut self, frame: Frame, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
        match self.kind {
            DeviceKind::Switch => self.switch_receive(frame, in_port, tick, events),
            DeviceKind::Ext => {
                // encode to wire bytes and hand to the JS bridge
                if let Some(bytes) = crate::eth::encode(&frame) {
                    events.push(SimEvent::new(
                        tick,
                        Some(self.id),
                        self.ports[in_port].link,
                        "ext_tx",
                        format!("{}: -> eth[{}] {} bytes", self.name, in_port, bytes.len()),
                    ));
                    self.ext_out.push((in_port, bytes));
                }
            }
            _ => self.l3_receive(frame, in_port, tick, events),
        }
    }

    /// ext only: real wire bytes arrived from the emulated node — decode and
    /// push onto the link on `port`.
    pub fn ext_inject(&mut self, port: usize, bytes: &[u8], tick: u64, events: &mut Vec<SimEvent>) {
        if self.kind != DeviceKind::Ext || port >= self.ports.len() {
            return;
        }
        if let Some(frame) = crate::eth::decode(bytes) {
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                self.ports[port].link,
                "ext_rx",
                format!("{}: eth[{}] <- {}", self.name, port, frame.label()),
            ));
            self.outbox.push((port, frame));
        }
    }

    fn switch_receive(
        &mut self,
        mut frame: Frame,
        in_port: usize,
        tick: u64,
        events: &mut Vec<SimEvent>,
    ) {
        let Some(vid) = self.ingress_vlan(in_port, frame.vlan) else {
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                self.ports.get(in_port).and_then(|p| p.link),
                "drop",
                format!("{}: VLAN {} not allowed on port {}", self.name, frame.vlan, in_port),
            ));
            return;
        };
        frame.vlan = vid;
        let src_is_mcast = ((frame.src_mac >> 40) & 1) == 1 || frame.src_mac == 0;
        let key = (frame.src_mac, vid);
        if !src_is_mcast && self.mac_table.get(&key) != Some(&in_port) {
            self.mac_table.insert(key, in_port);
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                None,
                "learn",
                format!("{}: learned {} vlan {} on port {}", self.name, mac_str(frame.src_mac), vid, in_port),
            ));
        }
        let dst_is_mcast = ((frame.dst_mac >> 40) & 1) == 1 || frame.dst_mac == BROADCAST_MAC;
        let learned = self.mac_table.get(&(frame.dst_mac, vid)).copied();
        match learned {
            Some(out) if !dst_is_mcast && out == in_port => {}
            Some(out) if !dst_is_mcast && out != in_port && self.port_carries(out, vid) => {
                events.push(SimEvent::new(
                    tick,
                    Some(self.id),
                    self.ports[out].link,
                    "forward",
                    format!("{}: {} vlan {} -> port {}", self.name, frame.label(), vid, out),
                ));
                self.queue_sw(out, frame, vid);
            }
            _ => {
                let ports: Vec<usize> = self
                    .ports
                    .iter()
                    .enumerate()
                    .filter(|(i, p)| *i != in_port && p.link.is_some() && self.port_carries(*i, vid))
                    .map(|(i, _)| i)
                    .collect();
                let n = ports.len();
                for i in ports {
                    self.queue_sw(i, frame.clone(), vid);
                }
                events.push(SimEvent::new(
                    tick,
                    Some(self.id),
                    None,
                    "flood",
                    format!("{}: flooded {} vlan {} to {} ports", self.name, frame.label(), vid, n),
                ));
            }
        }
    }

    fn ingress_vlan(&self, port: usize, tag: u16) -> Option<u16> {
        let p = self.ports.get(port)?;
        if !p.trunk {
            let vid = p.access_vlan.max(1);
            if tag == 0 || tag == vid {
                return Some(vid);
            }
            return None;
        }
        let vid = if tag == 0 { p.native_vlan.max(1) } else { tag };
        if p.allowed.as_ref().map(|a| a.contains(&vid)).unwrap_or(true) {
            Some(vid)
        } else {
            None
        }
    }

    fn port_carries(&self, port: usize, vid: u16) -> bool {
        let Some(p) = self.ports.get(port) else { return false };
        if !p.trunk {
            return p.access_vlan.max(1) == vid;
        }
        p.allowed.as_ref().map(|a| a.contains(&vid)).unwrap_or(true)
    }

    fn queue_sw(&mut self, port: usize, mut frame: Frame, vid: u16) {
        let tag = self.ports.get(port).map(|p| {
            if !p.trunk {
                0
            } else if vid == p.native_vlan.max(1) {
                0
            } else {
                vid
            }
        }).unwrap_or(0);
        crate::eth::set_vlan(&mut frame, tag);
        self.outbox.push((port, frame));
    }

    fn l3_receive(&mut self, mut frame: Frame, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
        let port_mac = self.ports[in_port].mac;
        let ip_mcast = (frame.dst_mac >> 24) == 0x0100_5e;
        if frame.dst_mac != port_mac && frame.dst_mac != BROADCAST_MAC && !(self.kind == DeviceKind::Router && ip_mcast) {
            return; // not addressed to us
        }
        if matches!(frame.payload, Payload::IcmpEchoRequest | Payload::IcmpEchoReply | Payload::Raw { ethertype: 0x0800 })
            && !ip_allowed(self, self.ports[in_port].in_acl, &frame)
        {
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                self.ports[in_port].link,
                "drop",
                format!("{}: ACL denied {}", self.name, frame.label()),
            ));
            return;
        }
        // opportunistic ARP learning
        if frame.src_ip != [0, 0, 0, 0] {
            self.arp_cache.insert(frame.src_ip, frame.src_mac);
            self.flush_pending(frame.src_ip, frame.src_mac, tick, events);
        }

        match frame.payload {
            Payload::ArpRequest { target_ip } => {
                if self.owns_ip(target_ip) {
                    let reply = Frame {
                        src_mac: port_mac,
                        dst_mac: frame.src_mac,
                        src_ip: target_ip,
                        dst_ip: frame.src_ip,
                        ttl: 1,
                        payload: Payload::ArpReply { target_ip },
                raw: None,
                icmp_echo: None, vlan: 0,
                    };
                    events.push(SimEvent::new(
                        tick,
                        Some(self.id),
                        self.ports[in_port].link,
                        "arp_rep",
                        format!(
                            "{}: {} is-at {}",
                            self.name,
                            ip_str(target_ip),
                            mac_str(port_mac)
                        ),
                    ));
                    self.outbox.push((in_port, reply));
                }
            }
            Payload::ArpReply { .. } => {
                events.push(SimEvent::new(
                    tick,
                    Some(self.id),
                    None,
                    "learn",
                    format!(
                        "{}: ARP {} is-at {}",
                        self.name,
                        ip_str(frame.src_ip),
                        mac_str(frame.src_mac)
                    ),
                ));
            }
            Payload::IcmpEchoRequest => {
                if crate::services::nat_ingress(self, &mut frame, in_port) {
                    self.forward(frame, Some(in_port), tick, events);
                } else if self.owns_ip(frame.dst_ip) {
                    let reply = Frame {
                        src_mac: port_mac,
                        dst_mac: frame.src_mac,
                        src_ip: frame.dst_ip,
                        dst_ip: frame.src_ip,
                        ttl: 64,
                        payload: Payload::IcmpEchoReply,
                raw: None,
                icmp_echo: frame.icmp_echo.clone(), vlan: 0,
                    };
                    events.push(SimEvent::new(
                        tick,
                        Some(self.id),
                        self.ports[in_port].link,
                        "echo_rep",
                        format!("{}: reply to {}", self.name, ip_str(frame.src_ip)),
                    ));
                    self.outbox.push((in_port, reply));
                } else if self.kind == DeviceKind::Router && !is_mcast_ip(frame.dst_ip) {
                    self.forward(frame, Some(in_port), tick, events);
                }
            }
            Payload::Raw { ethertype: 0x0800 } => {
                if crate::ospf::ingest(self, &frame, in_port, tick, events) {
                    // link-local OSPF; not routed
                } else if is_mcast_ip(frame.dst_ip) {
                    // multicast stays on the link
                } else if crate::services::nat_ingress(self, &mut frame, in_port) {
                    self.forward(frame, Some(in_port), tick, events);
                } else if crate::services::local_l4(self, &frame, in_port, tick, events) {
                    // answered or consumed (DHCP, DNS, TCP, UDP)
                } else if self.kind == DeviceKind::Router {
                    self.forward(frame, Some(in_port), tick, events);
                }
            }
            Payload::Raw { .. } => {}
            Payload::IcmpEchoReply => {
                if crate::services::nat_ingress(self, &mut frame, in_port) {
                    self.forward(frame, Some(in_port), tick, events);
                } else if self.owns_ip(frame.dst_ip) {
                    events.push(SimEvent::new(
                        tick,
                        Some(self.id),
                        None,
                        "deliver",
                        format!(
                            "{}: reply from {} ttl={}",
                            self.name,
                            ip_str(frame.src_ip),
                            frame.ttl
                        ),
                    ));
                } else if self.kind == DeviceKind::Router && !is_mcast_ip(frame.dst_ip) {
                    self.forward(frame, Some(in_port), tick, events);
                }
            }
        }
    }

    fn forward(&mut self, mut frame: Frame, in_port: Option<usize>, tick: u64, events: &mut Vec<SimEvent>) {
        frame.ttl = frame.ttl.saturating_sub(1);
        if frame.ttl == 0 {
            events.push(SimEvent::new(
                tick,
                Some(self.id),
                None,
                "drop",
                format!("{}: TTL expired for {}", self.name, frame.label()),
            ));
            return;
        }
        crate::services::nat_egress(self, &mut frame, in_port);
        if let Some(raw) = frame.raw.as_mut() {
            crate::eth::sync_ipv4(raw, frame.src_ip, frame.dst_ip, frame.ttl, None, None, None);
        }
        self.emit_ip(frame, tick, events);
    }

    fn flush_pending(
        &mut self,
        ip: [u8; 4],
        mac: u64,
        tick: u64,
        events: &mut Vec<SimEvent>,
    ) {
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].resolve == ip {
                let p = self.pending.remove(i);
                let mut f = p.frame;
                f.dst_mac = mac;
                events.push(SimEvent::new(
                    tick,
                    Some(self.id),
                    self.ports[p.out_port].link,
                    "tx",
                    format!("{}: {} (ARP resolved)", self.name, f.label()),
                ));
                self.outbox.push((p.out_port, f));
            } else {
                i += 1;
            }
        }
    }
}

fn is_mcast_ip(ip: [u8; 4]) -> bool {
    (224..240).contains(&ip[0])
}

pub(crate) fn wild_match(ip: [u8; 4], addr: [u8; 4], wild: [u8; 4]) -> bool {
    ip.iter().zip(addr).zip(wild).all(|((&b, a), w)| (b ^ a) & !w == 0)
}

/// Inbound/outbound extended ACL. No list, or a missing list, permits.
pub(crate) fn ip_allowed(dev: &Device, acl: Option<u16>, frame: &Frame) -> bool {
    let Some(num) = acl else { return true };
    let Some(list) = dev.acls.iter().find(|a| a.num == num) else { return true };
    let (proto, sport, dport) = l4_of(frame);
    for e in &list.entries {
        if e.proto != 0 && e.proto != proto {
            continue;
        }
        if !wild_match(frame.src_ip, e.src, e.src_wild) || !wild_match(frame.dst_ip, e.dst, e.dst_wild) {
            continue;
        }
        if e.sport.is_some() && e.sport != Some(sport) {
            continue;
        }
        if e.dport.is_some() && e.dport != Some(dport) {
            continue;
        }
        return e.permit;
    }
    false
}

fn l4_of(frame: &Frame) -> (u8, u16, u16) {
    if let Some(raw) = &frame.raw {
        if let Some(v) = crate::eth::view_ip(raw) {
            return (v.proto, v.sport, v.dport);
        }
    }
    match frame.payload {
        Payload::IcmpEchoRequest | Payload::IcmpEchoReply => (1, 0, 0),
        _ => (0, 0, 0),
    }
}
