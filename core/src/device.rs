use std::collections::HashMap;

use crate::events::{EvKind, SimEvent};
use crate::frame::{Frame, Payload, BROADCAST_MAC};
use crate::services::{local_l4, nat_egress, nat_ingress};
use crate::{ip_str, mask_bits, net_of, LinkId};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpMethod {
    None,
    Manual,
    Dhcp,
}

impl IpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            IpMethod::None => "none",
            IpMethod::Manual => "manual",
            IpMethod::Dhcp => "dhcp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    Static,
    Dynamic,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PortStats {
    pub rx: u64,
    pub tx: u64,
    pub rx_err: u64,
    pub tx_err: u64,
    pub drops: u64,
    pub cleared_tick: u64,
}

#[derive(Debug, Clone)]
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
    pub admin_up: bool,
    pub description: Option<String>,
    pub method: IpMethod,
    /// Carrier state driven by topology link ops.
    pub link_up: bool,
    pub stats: PortStats,
}

/// One extended ACL entry. `proto` 0 matches any. Wildcards are Cisco-style.
#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RouteProto {
    Connected,
    Static,
    Ospf,
}

#[derive(Debug, Clone, Copy)]
pub struct Route {
    pub prefix: [u8; 4],
    pub mask: [u8; 4],
    /// ignored for connected routes
    pub next_hop: [u8; 4],
    pub out_port: usize,
    pub connected: bool,
    /// Static beats OSPF at equal prefix length; connected always wins.
    pub proto: RouteProto,
    pub ad: u8,
    pub metric: u32,
    pub learned_tick: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct MacEntry {
    pub port: usize,
    pub created_tick: u64,
    pub etype: EntryType,
}

#[derive(Debug, Clone, Copy)]
pub struct ArpEntry {
    pub mac: u64,
    pub port: usize,
    pub created_tick: u64,
    pub etype: EntryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VlanState {
    Active,
    Suspend,
}

#[derive(Debug, Clone)]
pub struct Vlan {
    pub name: String,
    pub state: VlanState,
}

#[derive(Debug, Clone, Copy)]
pub struct Toggles {
    pub domain_lookup: bool,
    /// Constructor capability — immutable post-construction.
    pub routing_enabled: bool,
    pub proxy_arp: bool,
    pub directed_broadcast: bool,
    /// Live forwarding toggle `no ip routing` flips.
    pub ip_routing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DhcpExcluded {
    pub lo: [u8; 4],
    pub hi: Option<[u8; 4]>,
}

#[derive(Debug, Clone, Copy)]
pub struct LeaseEntry {
    pub ip: [u8; 4],
    pub granted_tick: u64,
    pub expires_tick: Option<u64>,
}

/// Frame held back while ARP resolves `resolve`.
#[derive(Debug, Clone)]
pub struct Pending {
    pub frame: Frame,
    pub out_port: usize,
    pub resolve: [u8; 4],
}

/// Core-side ping/traceroute probe state — drives the C3–C8 wording
/// through the shared event queue.
#[derive(Debug, Clone)]
pub enum Probe {
    Ping(PingProbe),
    Trace(TraceProbe),
}

#[derive(Debug, Clone)]
pub struct PingProbe {
    pub dst: [u8; 4],
    pub sent: u8,
    pub ok: u8,
    pub rtts: Vec<u64>,
    /// (launch tick, icmp seq) of the outstanding probe
    pub waiting: Option<(u64, u16)>,
    /// next launch tick
    pub next: u64,
}

#[derive(Debug, Clone)]
pub struct TraceProbe {
    pub dst: [u8; 4],
    pub hop: u8,
    /// (launch tick, icmp seq) of the outstanding probe
    pub waiting: Option<(u64, u16)>,
    pub next: u64,
    pub done: bool,
}

pub const PING_PROBES: u8 = 5;
/// Probe wait budget — sized so multi-hop paths (link delay × hops) still
/// fit; the C4 header text always says "2 seconds".
pub const PROBE_TIMEOUT: u64 = 40;
pub const TRACE_MAX_HOPS: u8 = 30;

impl Port {
    /// VLAN an untagged access frame belongs to (never 0).
    fn access_vid(&self) -> u16 {
        self.access_vlan.max(1)
    }

    /// VLAN a trunk port tags natively (never 0).
    fn native_vid(&self) -> u16 {
        self.native_vlan.max(1)
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    pub id: DeviceId,
    pub name: String,
    pub kind: DeviceKind,
    pub x: f64,
    pub y: f64,
    pub ports: Vec<Port>,
    /// switch: (mac, vlan) -> port index
    pub mac_table: HashMap<(u64, u16), MacEntry>,
    pub acls: Vec<Acl>,
    pub ospf: crate::ospf::Ospf,
    pub arp_cache: HashMap<[u8; 4], ArpEntry>,
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
    pub leases: HashMap<u64, LeaseEntry>,
    pub dhcp: Option<crate::services::DhcpClient>,
    pub flows: Vec<crate::services::Flow>,
    pub nat: Vec<crate::services::NatEnt>,
    /// 1:1 maps (proto 0) and static port forwards (tcp/udp).
    pub static_nat: Vec<crate::services::StaticNat>,
    /// switch: vlan id -> configured row; seeded {1,1002-1005} at construction
    pub vlans: HashMap<u16, Vlan>,
    pub toggles: Toggles,
    pub dhcp_excluded: Vec<DhcpExcluded>,
    /// buffered syslog lines (`show logging buffered`)
    pub log: std::collections::VecDeque<String>,
    /// in-flight ping/trace job, `Topology::ping`/`trace` arm it
    pub probe: Option<Probe>,
    /// monotonic icmp seq for probe echoes — a reply is honored only while
    /// the exact seq is outstanding, so late replies from a previous block
    /// cannot leak into a new one
    pub probe_seq: u16,
}

impl Device {
    pub fn new(
        id: DeviceId,
        name: String,
        kind: DeviceKind,
        x: f64,
        y: f64,
        macs: Vec<u64>,
    ) -> Self {
        let admin_up = kind != DeviceKind::Router;
        let toggles = match kind {
            DeviceKind::Router => Toggles {
                domain_lookup: true,
                routing_enabled: true,
                proxy_arp: true,
                directed_broadcast: false,
                ip_routing: true,
            },
            DeviceKind::Switch => Toggles {
                domain_lookup: true,
                routing_enabled: true,
                proxy_arp: true,
                directed_broadcast: false,
                ip_routing: false,
            },
            _ => Toggles {
                domain_lookup: true,
                routing_enabled: false,
                proxy_arp: false,
                directed_broadcast: false,
                ip_routing: false,
            },
        };
        let mut vlans = HashMap::new();
        if kind == DeviceKind::Switch {
            for (id, name) in [
                (1u16, "default"),
                (1002, "fddi-default"),
                (1003, "token-ring"),
                (1004, "fddinet"),
                (1005, "trnet"),
            ] {
                vlans.insert(
                    id,
                    Vlan {
                        name: name.to_string(),
                        state: VlanState::Active,
                    },
                );
            }
        }
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
                    admin_up,
                    description: None,
                    method: IpMethod::None,
                    link_up: false,
                    stats: PortStats::default(),
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
            static_nat: Vec::new(),
            acls: Vec::new(),
            ospf: crate::ospf::Ospf::default(),
            vlans,
            toggles,
            dhcp_excluded: Vec::new(),
            log: std::collections::VecDeque::new(),
            probe: None,
            probe_seq: 0,
        }
    }

    pub fn free_port(&self) -> Option<usize> {
        self.ports.iter().position(|p| p.link.is_none())
    }

    pub(crate) fn owns_ip(&self, ip: [u8; 4]) -> bool {
        self.ports.iter().any(|p| p.ip.map(|(a, _)| a) == Some(ip))
    }

    /// First configured IPv4 address, for locally originated packets.
    pub(crate) fn first_ip(&self) -> Option<[u8; 4]> {
        self.ports.iter().find_map(|p| p.ip.map(|(a, _)| a))
    }

    /// MAC of the port that owns `ip`, for wire builders that need it early.
    pub(crate) fn mac_of_ip(&self, ip: [u8; 4]) -> Option<u64> {
        self.ports
            .iter()
            .find_map(|p| p.ip.filter(|(a, _)| *a == ip).map(|_| p.mac))
    }

    /// Replace the default route with one via `gw` out `out_port`.
    pub(crate) fn set_default_route(&mut self, gw: [u8; 4], out_port: usize, tick: u64) {
        self.routes.retain(|r| r.prefix != [0, 0, 0, 0]);
        self.routes.push(Route {
            prefix: [0, 0, 0, 0],
            mask: [0, 0, 0, 0],
            next_hop: gw,
            out_port,
            connected: false,
            proto: RouteProto::Static,
            ad: 1,
            metric: 0,
            learned_tick: tick,
        });
    }

    /// Canonical netlab port name for this kind (IfMap key + name authority).
    pub fn canonical_port_name(kind: DeviceKind, port: usize) -> String {
        match kind {
            DeviceKind::Router => format!("g0/{port}"),
            DeviceKind::Switch => format!("fa0/{}", port + 1),
            DeviceKind::Host => format!("eth{port}"),
            DeviceKind::Ext => format!("e{port}"),
        }
    }

    /// IOS display name used by renderers and link/syslog wording.
    pub fn render_port_name(kind: DeviceKind, port: usize) -> String {
        match kind {
            DeviceKind::Router => format!("GigabitEthernet0/{port}"),
            DeviceKind::Switch => format!("FastEthernet0/{}", port + 1),
            DeviceKind::Host => format!("eth{port}"),
            DeviceKind::Ext => format!("Ethernet{port}"),
        }
    }

    pub fn port_names(&self) -> Vec<String> {
        (0..self.ports.len())
            .map(|i| Self::canonical_port_name(self.kind, i))
            .collect()
    }

    /// True while the vlan row exists and is `active`; hosts carry no map.
    pub fn vlan_active(&self, vid: u16) -> bool {
        if self.kind != DeviceKind::Switch {
            return true;
        }
        self.vlans
            .get(&vid)
            .map(|v| v.state == VlanState::Active)
            .unwrap_or(false)
    }

    /// Re-derive connected routes from `admin_up && link_up && addressed` ports.
    /// An unchanged tuple keeps its `learned_tick`; a (re)appearing row stamps `tick`.
    pub fn sync_connected(&mut self, tick: u64) {
        let keep: Vec<Route> = self
            .routes
            .iter()
            .filter(|r| r.proto != RouteProto::Connected)
            .cloned()
            .collect();
        let mut next = keep;
        for (i, p) in self.ports.iter().enumerate() {
            let Some((addr, mask)) = p.ip else { continue };
            if !p.admin_up || !p.link_up {
                continue;
            }
            let prefix = net_of(addr, mask);
            let learned_tick = self
                .routes
                .iter()
                .find(|r| {
                    r.proto == RouteProto::Connected && r.prefix == prefix && r.mask == mask
                })
                .map(|r| r.learned_tick)
                .unwrap_or(tick);
            next.push(Route {
                prefix,
                mask,
                next_hop: addr,
                out_port: i,
                connected: true,
                proto: RouteProto::Connected,
                ad: 0,
                metric: 0,
                learned_tick,
            });
        }
        self.routes = next;
    }

    /// Power off: drop all runtime state. Config (routes, IPs, NAT rules,
    /// DNS records, OSPF networks, bound DHCP leases) survives.
    pub(crate) fn power_off(&mut self) {
        self.powered = false;
        self.outbox.clear();
        self.ext_out.clear();
        self.pending.clear();
        self.arp_cache.clear();
        self.mac_table.clear();
        self.flows.clear();
        self.nat.clear();
        self.ospf.reset();
        self.probe = None;
        if self
            .dhcp
            .as_ref()
            .map(|c| c.state != crate::services::DhcpState::Bound)
            .unwrap_or(false)
        {
            self.dhcp = None;
        }
    }

    pub(crate) fn power_on(&mut self) {
        self.powered = true;
    }

    /// Reload-replay restore: the device becomes `base` — the snapshot the
    /// adapter took after the lab document applied — except live carrier
    /// bindings and canvas position, which are not configuration, and
    /// in-flight state a reload drops. Connected routes re-derive from the
    /// restored interfaces at the end.
    pub fn restore_baseline(&mut self, base: &Device, tick: u64) {
        let live: Vec<(Option<LinkId>, bool)> =
            self.ports.iter().map(|p| (p.link, p.link_up)).collect();
        let (x, y) = (self.x, self.y);
        *self = base.clone();
        self.x = x;
        self.y = y;
        for (p, (link, link_up)) in self.ports.iter_mut().zip(live) {
            p.link = link;
            p.link_up = link_up;
        }
        self.outbox.clear();
        self.ext_out.clear();
        self.pending.clear();
        self.arp_cache.clear();
        self.mac_table.clear();
        self.flows.clear();
        self.nat.clear();
        self.ospf.reset();
        self.probe = None;
        self.sync_connected(tick);
    }

    /// Longest prefix, then connected, then static over OSPF.
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
    /// Returns false when no route exists or an ACL denies the frame.
    pub fn emit_ip(&mut self, mut frame: Frame, _tick: u64) -> bool {
        let Some(route) = self.lookup(frame.dst_ip) else {
            return false;
        };
        let next_hop = if route.connected {
            frame.dst_ip
        } else {
            route.next_hop
        };
        if !ip_allowed(self, self.ports[route.out_port].out_acl, &frame) {
            return false;
        }
        let port = &self.ports[route.out_port];
        frame.src_mac = port.mac;
        if frame.src_ip == [0, 0, 0, 0] {
            if let Some((addr, _)) = port.ip {
                frame.src_ip = addr;
            }
        }
        if let Some(mac) = self.arp_cache.get(&next_hop).map(|e| e.mac) {
            frame.dst_mac = mac;
            self.outbox.push((route.out_port, frame));
        } else {
            let mut arp = Frame::ip(
                port.ip.map(|(a, _)| a).unwrap_or(frame.src_ip),
                next_hop,
                1,
                Payload::ArpRequest {
                    target_ip: next_hop,
                },
            );
            arp.src_mac = port.mac;
            arp.dst_mac = BROADCAST_MAC;
            self.pending.push(Pending {
                frame,
                out_port: route.out_port,
                resolve: next_hop,
            });
            self.outbox.push((route.out_port, arp));
        }
        true
    }

    /// A frame arrived on `in_port`.
    pub fn receive(&mut self, frame: Frame, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) {
        if let Some(p) = self.ports.get_mut(in_port) {
            if !p.admin_up || !p.link_up {
                p.stats.drops += 1;
                return;
            }
            p.stats.rx += 1;
        }
        match self.kind {
            DeviceKind::Switch => self.switch_receive(frame, in_port, tick),
            DeviceKind::Ext => {
                // encode to wire bytes and hand to the JS bridge
                if let Some(bytes) = crate::eth::encode(&frame) {
                    self.ext_out.push((in_port, bytes));
                }
            }
            _ => self.l3_receive(frame, in_port, tick, events),
        }
    }

    /// ext only: real wire bytes arrived from the emulated node — decode and
    /// push onto the link on `port`.
    pub fn ext_inject(&mut self, port: usize, bytes: &[u8]) {
        if !self.powered || self.kind != DeviceKind::Ext || port >= self.ports.len() {
            return;
        }
        if let Some(frame) = crate::eth::decode(bytes) {
            self.outbox.push((port, frame));
        }
    }

    fn switch_receive(&mut self, mut frame: Frame, in_port: usize, tick: u64) {
        let Some(vid) = self.ingress_vlan(in_port, frame.vlan) else {
            return;
        };
        frame.vlan = vid;
        let src_is_mcast = ((frame.src_mac >> 40) & 1) == 1 || frame.src_mac == 0;
        let key = (frame.src_mac, vid);
        if !src_is_mcast {
            match self.mac_table.get_mut(&key) {
                Some(e) if e.port == in_port => e.created_tick = tick,
                _ => {
                    self.mac_table.insert(
                        key,
                        MacEntry {
                            port: in_port,
                            created_tick: tick,
                            etype: EntryType::Dynamic,
                        },
                    );
                }
            }
        }
        let dst_is_mcast = ((frame.dst_mac >> 40) & 1) == 1 || frame.dst_mac == BROADCAST_MAC;
        let learned = self.mac_table.get(&(frame.dst_mac, vid)).map(|e| e.port);
        match learned {
            Some(out) if !dst_is_mcast && out == in_port => {}
            Some(out) if !dst_is_mcast && out != in_port && self.port_carries(out, vid) => {
                self.queue_sw(out, frame, vid);
            }
            _ => {
                let ports: Vec<usize> = self
                    .ports
                    .iter()
                    .enumerate()
                    .filter(|(i, p)| {
                        *i != in_port && p.link.is_some() && self.port_carries(*i, vid)
                    })
                    .map(|(i, _)| i)
                    .collect();
                for i in ports {
                    self.queue_sw(i, frame.clone(), vid);
                }
            }
        }
    }

    fn ingress_vlan(&self, port: usize, tag: u16) -> Option<u16> {
        let p = self.ports.get(port)?;
        if !p.trunk {
            let vid = p.access_vid();
            if tag == 0 || tag == vid {
                return Some(vid);
            }
            return None;
        }
        let vid = if tag == 0 { p.native_vid() } else { tag };
        if p.allowed.as_ref().map(|a| a.contains(&vid)).unwrap_or(true) {
            Some(vid)
        } else {
            None
        }
    }

    fn port_carries(&self, port: usize, vid: u16) -> bool {
        let Some(p) = self.ports.get(port) else {
            return false;
        };
        if !p.trunk {
            return p.access_vid() == vid;
        }
        p.allowed.as_ref().map(|a| a.contains(&vid)).unwrap_or(true)
    }

    fn queue_sw(&mut self, port: usize, mut frame: Frame, vid: u16) {
        let tag = self
            .ports
            .get(port)
            .map(|p| {
                if !p.trunk || vid == p.native_vid() {
                    0
                } else {
                    vid
                }
            })
            .unwrap_or(0);
        crate::eth::set_vlan(&mut frame, tag);
        self.outbox.push((port, frame));
    }

    fn l3_receive(
        &mut self,
        mut frame: Frame,
        in_port: usize,
        tick: u64,
        events: &mut Vec<SimEvent>,
    ) {
        let port_mac = self.ports[in_port].mac;
        let ip_mcast = (frame.dst_mac >> 24) == 0x0001_005e;
        if frame.dst_mac != port_mac
            && frame.dst_mac != BROADCAST_MAC
            && !(self.kind == DeviceKind::Router && ip_mcast)
        {
            return;
        }
        if matches!(
            frame.payload,
            Payload::IcmpEchoRequest
                | Payload::IcmpEchoReply
                | Payload::IcmpTimeExceeded
                | Payload::Raw { ethertype: 0x0800 }
        ) && !ip_allowed(self, self.ports[in_port].in_acl, &frame)
        {
            return;
        }
        // opportunistic ARP learning
        if frame.src_ip != [0, 0, 0, 0] {
            self.arp_cache.insert(
                frame.src_ip,
                ArpEntry {
                    mac: frame.src_mac,
                    port: in_port,
                    created_tick: tick,
                    etype: EntryType::Dynamic,
                },
            );
            self.flush_pending(frame.src_ip, frame.src_mac);
        }

        match frame.payload {
            Payload::ArpRequest { target_ip } => {
                if self.owns_ip(target_ip) || crate::services::nat_global(self, target_ip) {
                    let mut reply =
                        Frame::ip(target_ip, frame.src_ip, 1, Payload::ArpReply { target_ip });
                    reply.src_mac = port_mac;
                    reply.dst_mac = frame.src_mac;
                    self.outbox.push((in_port, reply));
                }
            }
            Payload::ArpReply { .. } => {}
            Payload::IcmpEchoRequest => {
                if nat_ingress(self, &mut frame, in_port) {
                    self.forward(frame, Some(in_port), tick);
                } else if self.owns_ip(frame.dst_ip) {
                    let mut reply =
                        Frame::ip(frame.dst_ip, frame.src_ip, 64, Payload::IcmpEchoReply);
                    reply.src_mac = port_mac;
                    reply.dst_mac = frame.src_mac;
                    reply.icmp_echo = frame.icmp_echo.clone();
                    self.outbox.push((in_port, reply));
                } else {
                    self.forward_if_router(frame, in_port, tick);
                }
            }
            Payload::Raw { ethertype: 0x0800 } => {
                if crate::ospf::ingest(self, &frame, in_port, tick, events) {
                    // link-local OSPF; not routed
                } else if is_mcast_ip(frame.dst_ip) {
                    // multicast stays on the link
                } else if nat_ingress(self, &mut frame, in_port) {
                    self.forward(frame, Some(in_port), tick);
                } else if local_l4(self, &frame, in_port, tick, events) {
                    // answered or consumed (DHCP, DNS, TCP, UDP)
                } else {
                    self.forward_if_router(frame, in_port, tick);
                }
            }
            Payload::Raw { .. } => {}
            Payload::IcmpTimeExceeded => {
                if nat_ingress(self, &mut frame, in_port) {
                    self.forward(frame, Some(in_port), tick);
                } else if self.owns_ip(frame.dst_ip) {
                    self.probe_hop(&frame, tick, events);
                } else {
                    self.forward_if_router(frame, in_port, tick);
                }
            }
            Payload::IcmpEchoReply => {
                if nat_ingress(self, &mut frame, in_port) {
                    self.forward(frame, Some(in_port), tick);
                } else if self.owns_ip(frame.dst_ip) {
                    self.probe_reply(&frame, tick, events);
                } else {
                    self.forward_if_router(frame, in_port, tick);
                }
            }
        }
    }

    /// Routers forward non-multicast frames they didn't answer.
    /// Transit forwarding runs only while `ip routing` is set.
    fn forward_if_router(&mut self, frame: Frame, in_port: usize, tick: u64) {
        if self.kind == DeviceKind::Router && self.toggles.ip_routing && !is_mcast_ip(frame.dst_ip)
        {
            self.forward(frame, Some(in_port), tick);
        }
    }

    fn forward(&mut self, mut frame: Frame, in_port: Option<usize>, tick: u64) {
        frame.ttl = frame.ttl.saturating_sub(1);
        if frame.ttl == 0 {
            if let Some(port) = in_port {
                if let Some((src, _)) = self.ports.get(port).and_then(|p| p.ip) {
                    let mut exceeded =
                        Frame::ip(src, frame.src_ip, 64, Payload::IcmpTimeExceeded);
                    exceeded.icmp_echo = frame.icmp_echo.clone();
                    self.emit_ip(exceeded, tick);
                }
            }
            return;
        }
        nat_egress(self, &mut frame, in_port, tick);
        if let Some(raw) = frame.raw.as_mut() {
            crate::eth::sync_ipv4(raw, frame.src_ip, frame.dst_ip, frame.ttl, None, None, None);
        }
        self.emit_ip(frame, tick);
    }

    fn flush_pending(&mut self, ip: [u8; 4], mac: u64) {
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].resolve == ip {
                let p = self.pending.remove(i);
                let mut f = p.frame;
                f.dst_mac = mac;
                self.outbox.push((p.out_port, f));
            } else {
                i += 1;
            }
        }
    }

    /// Arm a five-probe ping block (C4 header now, glyphs and C5 via poll).
    pub fn probe_ping(&mut self, dst: [u8; 4], tick: u64, events: &mut Vec<SimEvent>) {
        let note = |events: &mut Vec<SimEvent>, msg: String| {
            events.push(SimEvent::new(tick, self.id, None, EvKind::Ping, msg));
        };
        note(
            events,
            format!(
                "Sending {PING_PROBES}, 100-byte ICMP Echos to {}, timeout is 2 seconds:",
                ip_str(dst)
            ),
        );
        note(events, "Type escape sequence to abort.".to_string());
        self.probe = Some(Probe::Ping(PingProbe {
            dst,
            sent: 0,
            ok: 0,
            rtts: Vec::new(),
            waiting: None,
            next: tick,
        }));
    }

    /// Arm a traceroute job (C6 header now, C7/C8 rows via poll).
    pub fn probe_trace(&mut self, dst: [u8; 4], tick: u64, events: &mut Vec<SimEvent>) {
        events.push(SimEvent::new(
            tick,
            self.id,
            None,
            EvKind::Trace,
            format!("Tracing the route to {}", ip_str(dst)),
        ));
        self.probe = Some(Probe::Trace(TraceProbe {
            dst,
            hop: 1,
            waiting: None,
            next: tick,
            done: false,
        }));
    }

    /// An echo reply reached this device — resolve an outstanding probe.
    /// It counts only when it comes from the probe's target AND carries the
    /// outstanding icmp seq, so a late reply from an earlier block cannot
    /// leak into a new one.
    fn probe_reply(&mut self, frame: &Frame, tick: u64, events: &mut Vec<SimEvent>) {
        let src = frame.src_ip;
        let seq = frame.icmp_echo.as_deref()
            .and_then(|b| b.get(2..4))
            .map(|b| u16::from_be_bytes([b[0], b[1]]));
        match self.probe.as_mut() {
            Some(Probe::Ping(p))
                if matches!(p.waiting, Some((_, s)) if Some(s) == seq) && src == p.dst =>
            {
                let rtt = tick - p.waiting.take().map(|(t, _)| t).unwrap_or(tick);
                p.ok += 1;
                p.rtts.push(rtt);
                p.next = tick + 1;
                events.push(SimEvent::new(tick, self.id, None, EvKind::Ping, "!".into()));
            }
            Some(Probe::Trace(t))
                if matches!(t.waiting, Some((_, s)) if Some(s) == seq) && src == t.dst =>
            {
                let rtt = tick - t.waiting.take().map(|(v, _)| v).unwrap_or(tick);
                let hop = t.hop;
                t.done = true;
                events.push(SimEvent::new(
                    tick,
                    self.id,
                    None,
                    EvKind::Trace,
                    format!("{hop}  {}  {rtt} msec", ip_str(src)),
                ));
            }
            _ => {}
        }
    }

    /// A time-exceeded reached this device — resolve the current trace hop.
    /// Only honored when it carries the outstanding probe's icmp seq.
    fn probe_hop(&mut self, frame: &Frame, tick: u64, events: &mut Vec<SimEvent>) {
        let src = frame.src_ip;
        let seq = frame.icmp_echo.as_deref()
            .and_then(|b| b.get(2..4))
            .map(|b| u16::from_be_bytes([b[0], b[1]]));
        if let Some(Probe::Trace(t)) = self.probe.as_mut() {
            if matches!(t.waiting, Some((_, s)) if Some(s) == seq) {
                let sent = t.waiting.take().map(|(v, _)| v).unwrap_or(tick);
                let rtt = tick - sent;
                let hop = t.hop;
                t.hop += 1;
                t.next = tick + 1;
                events.push(SimEvent::new(
                    tick,
                    self.id,
                    None,
                    EvKind::Trace,
                    format!("{hop}  {}  {rtt} msec", ip_str(src)),
                ));
            }
        }
    }

    /// An ICMP unreachable reached this device while a probe waited.
    pub fn probe_unreach(&mut self, tick: u64, events: &mut Vec<SimEvent>) {
        match self.probe.as_mut() {
            Some(Probe::Ping(p)) if p.waiting.is_some() => {
                p.waiting = None;
                p.next = tick + 1;
                events.push(SimEvent::new(tick, self.id, None, EvKind::Ping, "U".into()));
            }
            Some(Probe::Trace(t)) if t.waiting.is_some() => {
                t.waiting = None;
                let hop = t.hop;
                t.hop += 1;
                t.next = tick + 1;
                events.push(SimEvent::new(
                    tick,
                    self.id,
                    None,
                    EvKind::Trace,
                    format!("{hop}  *"),
                ));
            }
            _ => {}
        }
    }

    /// Per-tick probe driver — launches probes, times them out, closes blocks.
    pub fn poll_probe(&mut self, tick: u64, events: &mut Vec<SimEvent>) {
        if !self.powered {
            self.probe = None;
            return;
        }
        let launch = match self.probe.as_mut() {
            Some(Probe::Ping(p)) => {
                if let Some((sent, _)) = p.waiting {
                    if tick.saturating_sub(sent) >= PROBE_TIMEOUT {
                        p.waiting = None;
                        p.next = tick + 1;
                        events.push(SimEvent::new(
                            tick,
                            self.id,
                            None,
                            EvKind::Ping,
                            ".".into(),
                        ));
                    }
                }
                if p.waiting.is_none() && tick >= p.next && p.sent < PING_PROBES {
                    self.probe_seq = self.probe_seq.wrapping_add(1);
                    let seq = self.probe_seq;
                    p.sent += 1;
                    p.waiting = Some((tick, seq));
                    Some((p.dst, 64, seq))
                } else {
                    None
                }
            }
            Some(Probe::Trace(t)) => {
                if let Some((sent, _)) = t.waiting {
                    if tick.saturating_sub(sent) >= PROBE_TIMEOUT {
                        t.waiting = None;
                        let hop = t.hop;
                        t.hop += 1;
                        t.next = tick + 1;
                        events.push(SimEvent::new(
                            tick,
                            self.id,
                            None,
                            EvKind::Trace,
                            format!("{hop}  *"),
                        ));
                    }
                }
                if t.waiting.is_none() && !t.done && tick >= t.next {
                    if t.hop > TRACE_MAX_HOPS {
                        t.done = true;
                        None
                    } else {
                        self.probe_seq = self.probe_seq.wrapping_add(1);
                        let seq = self.probe_seq;
                        t.waiting = Some((tick, seq));
                        Some((t.dst, t.hop, seq))
                    }
                } else {
                    None
                }
            }
            None => None,
        };
        if let Some((dst, ttl, seq)) = launch {
            let mut echo = Frame::ip([0; 4], dst, ttl, Payload::IcmpEchoRequest);
            echo.icmp_echo = Some(vec![0x70, 0x70, (seq >> 8) as u8, seq as u8]);
            let ok = self.emit_ip(echo, tick);
            if !ok {
                match self.probe.as_mut() {
                    Some(Probe::Ping(p)) => {
                        p.waiting = None;
                        p.next = tick + 1;
                        events.push(SimEvent::new(
                            tick,
                            self.id,
                            None,
                            EvKind::Ping,
                            "U".into(),
                        ));
                    }
                    Some(Probe::Trace(t)) => {
                        t.waiting = None;
                        let hop = t.hop;
                        t.hop += 1;
                        t.next = tick + 1;
                        events.push(SimEvent::new(
                            tick,
                            self.id,
                            None,
                            EvKind::Trace,
                            format!("{hop}  *"),
                        ));
                    }
                    _ => {}
                }
            }
        }
        if let Some(Probe::Ping(p)) = self.probe.as_ref() {
            if p.sent >= PING_PROBES && p.waiting.is_none() {
                let (ok, rtts) = (p.ok, p.rtts.clone());
                self.probe = None;
                let msg = if ok == 0 {
                    format!("Success rate is 0 percent (0/{PING_PROBES})")
                } else {
                    let min = rtts.iter().min().copied().unwrap_or(0);
                    let max = rtts.iter().max().copied().unwrap_or(0);
                    let avg = rtts.iter().sum::<u64>() / rtts.len().max(1) as u64;
                    format!(
                        "Success rate is {} percent ({ok}/{PING_PROBES}), round-trip min/avg/max = {min}/{avg}/{max} ms",
                        ok as u64 * 100 / PING_PROBES as u64,
                    )
                };
                events.push(SimEvent::new(tick, self.id, None, EvKind::Ping, msg));
            }
        }
        if let Some(Probe::Trace(t)) = self.probe.as_ref() {
            if t.done {
                self.probe = None;
            }
        }
    }
}

fn is_mcast_ip(ip: [u8; 4]) -> bool {
    (224..240).contains(&ip[0])
}

pub(crate) fn wild_match(ip: [u8; 4], addr: [u8; 4], wild: [u8; 4]) -> bool {
    ip.iter()
        .zip(addr)
        .zip(wild)
        .all(|((&b, a), w)| (b ^ a) & !w == 0)
}

/// Inbound/outbound extended ACL. No list, or a missing list, permits.
pub(crate) fn ip_allowed(dev: &Device, acl: Option<u16>, frame: &Frame) -> bool {
    let Some(num) = acl else { return true };
    let Some(list) = dev.acls.iter().find(|a| a.num == num) else {
        return true;
    };
    let (proto, sport, dport, _icmp_id) = crate::services::l4_tuple(frame);
    for e in &list.entries {
        if e.proto != 0 && e.proto != proto {
            continue;
        }
        if !wild_match(frame.src_ip, e.src, e.src_wild)
            || !wild_match(frame.dst_ip, e.dst, e.dst_wild)
        {
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
