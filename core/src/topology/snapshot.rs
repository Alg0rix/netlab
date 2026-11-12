//! Serializable snapshot of the whole topology — the DTO the UI renders.
//! Pure read of Device + Topology.tick. Presence contract: null = domain
//! absent, [] = exists but empty, Option<T> keys serialize null, never
//! missing. Every map-derived row is sorted by its declared key.

use serde::Serialize;

use crate::device::{Device, DeviceId, DeviceKind, EntryType, IpMethod, NatRole, RouteProto, VlanState};
use crate::link::LinkId;
use crate::ospf::NbrState;
use crate::{ip_str, mac_str};

use super::Topology;
#[derive(Serialize)]
pub struct Snapshot {
    pub tick: u64,
    pub devices: Vec<DeviceSnap>,
    pub links: Vec<LinkSnap>,
}

#[derive(Serialize)]
pub struct DeviceSnap {
    pub id: DeviceId,
    pub name: String,
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub powered: bool,
    pub tick: u64,
    pub ports: Vec<PortSnap>,
    pub routes: Vec<RouteSnap>,
    pub mac_table: Vec<MacSnap>,
    pub arp_table: Vec<ArpSnap>,
    /// Some on switches — null on router/host.
    pub vlans: Option<Vec<VlanSnap>>,
    /// Some iff an OSPF process exists — router only.
    pub ospf: Option<OspfSnap>,
    /// Some on routers incl. zero-config — null on switch/host.
    pub nat: Option<NatSnap>,
    /// Some on router/switch — null on host.
    pub dhcp: Option<DhcpSnap>,
    /// Host DHCP client seat — null where no client runs.
    pub dhcp_client: Option<String>,
    pub toggles: ToggleSnap,
    pub acls: Vec<AclSnap>,
    pub resolver: Option<String>,
    pub svc_dhcp: bool,
    pub svc_dns: bool,
    pub svc_http: bool,
    pub dns_records: Vec<(String, String)>,
}

#[derive(Serialize)]
pub struct PortSnap {
    pub name: String,
    pub link: Option<LinkId>,
    pub admin_up: bool,
    pub link_up: bool,
    pub ip: Option<String>,
    pub mask: Option<String>,
    /// "none" | "manual" | "dhcp"
    pub method: &'static str,
    pub description: Option<String>,
    pub mac: String,
    /// "" | "inside" | "outside"
    pub nat: &'static str,
    pub access_vlan: u16,
    pub trunk: bool,
    pub native_vlan: u16,
    /// null = all VLANs allowed on a trunk.
    pub allowed: Option<Vec<u16>>,
    pub in_acl: Option<u16>,
    pub out_acl: Option<u16>,
    pub helper: Option<String>,
    pub ospf_area: Option<u32>,
    pub rx: u64,
    pub tx: u64,
    pub drops: u64,
    pub rx_err: u64,
    pub tx_err: u64,
}

#[derive(Serialize)]
pub struct MacSnap {
    pub mac: String,
    pub port: usize,
    pub vlan: u16,
    pub created_tick: u64,
    /// "static" | "dynamic"
    pub etype: &'static str,
}

#[derive(Serialize)]
pub struct ArpSnap {
    pub ip: String,
    pub mac: String,
    pub port: usize,
    pub created_tick: u64,
    /// "static" | "dynamic"
    pub etype: &'static str,
}

#[derive(Serialize)]
pub struct RouteSnap {
    pub prefix: String,
    pub mask: String,
    pub next_hop: String,
    pub out_port: usize,
    /// "C" connected, "S" static, "O" OSPF
    pub proto: &'static str,
    pub ad: u8,
    pub metric: u32,
    pub learned_tick: u64,
}

#[derive(Serialize)]
pub struct VlanSnap {
    pub id: u16,
    pub name: String,
    /// "active" | "suspend"
    pub state: &'static str,
}

#[derive(Serialize)]
pub struct OspfSnap {
    pub pid: u16,
    pub router_id: OspfRouterIdSnap,
    pub networks: Vec<OspfNetSnap>,
    pub nbrs: Vec<OspfNbrSnap>,
}

#[derive(Serialize)]
pub struct OspfRouterIdSnap {
    pub value: String,
    pub explicit: bool,
}

#[derive(Serialize)]
pub struct OspfNetSnap {
    pub prefix: String,
    pub wild: String,
    pub area: u32,
}

#[derive(Serialize)]
pub struct OspfNbrSnap {
    pub router_id: String,
    pub addr: String,
    /// "O" | "FULL" | "-"
    pub state: &'static str,
    pub first_hello: u64,
    pub last_hello: u64,
}

#[derive(Serialize)]
pub struct NatSnap {
    pub statics: Vec<NatStaticSnap>,
    pub translations: Vec<NatTransSnap>,
}

#[derive(Serialize)]
pub struct NatStaticSnap {
    pub inside_local: String,
    pub inside_global: Option<String>,
}

#[derive(Serialize)]
pub struct NatTransSnap {
    pub inside_local: String,
    pub inside_global: String,
    pub outside: String,
    pub proto: u8,
    pub inside_port: u16,
    pub outside_port: u16,
    pub created_tick: u64,
}

#[derive(Serialize)]
pub struct DhcpSnap {
    pub service: bool,
    pub excluded: Vec<DhcpExcludedSnap>,
    pub leases: Vec<LeaseSnap>,
}

#[derive(Serialize)]
pub struct DhcpExcludedSnap {
    pub lo: String,
    pub hi: Option<String>,
}

#[derive(Serialize)]
pub struct LeaseSnap {
    pub ip: String,
    pub mac: String,
    pub granted_tick: u64,
    pub expires_tick: Option<u64>,
}

#[derive(Serialize)]
pub struct ToggleSnap {
    pub domain_lookup: bool,
    pub routing_enabled: bool,
    pub proxy_arp: bool,
    pub directed_broadcast: bool,
    pub ip_routing: bool,
}

#[derive(Serialize)]
pub struct AceSnap {
    pub permit: bool,
    pub proto: u8,
    pub src: String,
    pub src_wild: String,
    pub dst: String,
    pub dst_wild: String,
    pub sport: u16,
    pub dport: u16,
}

#[derive(Serialize)]
pub struct AclSnap {
    pub num: u16,
    pub entries: Vec<AceSnap>,
}

#[derive(Serialize)]
pub struct LinkSnap {
    pub id: LinkId,
    pub a: DeviceId,
    pub b: DeviceId,
    pub a_port: usize,
    pub b_port: usize,
    pub up: bool,
    pub capture: bool,
    pub delay: u32,
    pub jitter: u32,
    pub loss: u8,
    pub mtu: u16,
    pub speed: &'static str,
    pub frames: Vec<FrameSnap>,
}

#[derive(Serialize)]
pub struct FrameSnap {
    /// 0.0 at `a`, 1.0 at `b`
    pub t: f32,
    /// +1 travelling a→b, -1 travelling b→a
    pub dir: i8,
    pub color: &'static str,
    pub label: String,
}

fn etype(e: EntryType) -> &'static str {
    match e {
        EntryType::Static => "static",
        EntryType::Dynamic => "dynamic",
    }
}

fn ip_key(ip: [u8; 4]) -> u128 {
    u32::from_be_bytes(ip) as u128
}

/// Deterministic row order through ONE sort instantiation: every
/// numeric/IP key packs into u128, so all map-derived collections share
/// the (u128, idx) sort — the dns name sort below is the second and
/// last. Rows are ranked on raw domain keys, then mapped to strings
/// once (no parse-back, no per-compare allocation).
fn rank<T>(it: impl Iterator<Item = T>, k: impl Fn(&T) -> u128) -> Vec<T> {
    let v: Vec<T> = it.collect();
    let mut ord: Vec<(u128, u32)> = v
        .iter()
        .enumerate()
        .map(|(i, x)| (k(x), i as u32))
        .collect();
    ord.sort_unstable();
    let mut slots: Vec<Option<T>> = v.into_iter().map(Some).collect();
    ord.into_iter()
        .map(|(_, i)| slots[i as usize].take().unwrap())
        .collect()
}

impl Topology {
    pub fn snapshot(&self) -> Snapshot {
        let devices = rank(
            self.devices.values().map(|d| (d.id, d)),
            |(id, _)| *id as u128,
        )
        .into_iter()
        .map(|(_, d)| {
                let routes: Vec<RouteSnap> = rank(
                    d.routes.iter(),
                    |r| (ip_key(r.prefix) << 64) | (ip_key(r.mask) << 32) | ip_key(r.next_hop),
                )
                .into_iter()
                .map(|r| RouteSnap {
                    prefix: ip_str(r.prefix),
                    mask: ip_str(r.mask),
                    next_hop: ip_str(r.next_hop),
                    out_port: r.out_port,
                    proto: match r.proto {
                        RouteProto::Ospf => "O",
                        RouteProto::Static => "S",
                        RouteProto::Connected => "C",
                    },
                    ad: r.ad,
                    metric: r.metric,
                    learned_tick: r.learned_tick,
                })
                .collect();

                let mac_table: Vec<MacSnap> = rank(
                    d.mac_table.iter(),
                    |((mac, vlan), e)| {
                        ((e.port as u128) << 80) | ((*mac as u128) << 16) | (*vlan as u128)
                    },
                )
                .into_iter()
                .map(|((mac, vlan), e)| MacSnap {
                    mac: mac_str(*mac),
                    port: e.port,
                    vlan: *vlan,
                    created_tick: e.created_tick,
                    etype: etype(e.etype),
                })
                .collect();

                let arp_table: Vec<ArpSnap> = rank(d.arp_cache.iter(), |(ip, _)| ip_key(**ip))
                    .into_iter()
                    .map(|(ip, e)| ArpSnap {
                        ip: ip_str(*ip),
                        mac: mac_str(e.mac),
                        port: e.port,
                        created_tick: e.created_tick,
                        etype: etype(e.etype),
                    })
                    .collect();

                let vlans = (d.kind == DeviceKind::Switch).then(|| {
                    rank(d.vlans.iter(), |(id, _)| **id as u128)
                        .into_iter()
                        .map(|(id, v)| VlanSnap {
                            id: *id,
                            name: v.name.clone(),
                            state: match v.state {
                                VlanState::Active => "active",
                                VlanState::Suspend => "suspend",
                            },
                        })
                        .collect()
                });

                let ospf = (d.kind == DeviceKind::Router && d.ospf.on).then(|| {
                    let networks: Vec<OspfNetSnap> = rank(
                        d.ospf.nets.iter(),
                        |n| (ip_key(n.prefix) << 64) | (ip_key(n.wild) << 32) | (n.area as u128),
                    )
                    .into_iter()
                    .map(|n| OspfNetSnap {
                        prefix: ip_str(n.prefix),
                        wild: ip_str(n.wild),
                        area: n.area,
                    })
                    .collect();
                    let nbrs: Vec<OspfNbrSnap> = rank(d.ospf.nbrs.iter(), |n| ip_key(n.rid))
                        .into_iter()
                        .map(|n| OspfNbrSnap {
                            router_id: ip_str(n.rid),
                            addr: ip_str(n.ip),
                            state: match n.state {
                                NbrState::O => "O",
                                NbrState::Full => "FULL",
                                NbrState::Down => "-",
                            },
                            first_hello: n.first_hello,
                            last_hello: n.last_hello,
                        })
                        .collect();
                    OspfSnap {
                        pid: d.ospf.pid,
                        router_id: OspfRouterIdSnap {
                            value: ip_str(d.ospf.router_id.value),
                            explicit: d.ospf.router_id.explicit,
                        },
                        networks,
                        nbrs,
                    }
                });

                let nat = (d.kind == DeviceKind::Router).then(|| {
                    let statics: Vec<NatStaticSnap> = rank(
                        d.static_nat.iter().filter(|s| s.proto == 0),
                        |s| ip_key(s.inside_ip),
                    )
                    .into_iter()
                    .map(|s| NatStaticSnap {
                        inside_local: ip_str(s.inside_ip),
                        inside_global: s.inside_global.map(ip_str),
                    })
                    .collect();
                    let outside_ip = d
                        .ports
                        .iter()
                        .find(|p| p.nat == NatRole::Outside)
                        .and_then(|p| p.ip.map(|(a, _)| a));
                    let translations: Vec<NatTransSnap> = rank(
                        d.nat.iter(),
                        |n| {
                            (ip_key(n.inside_ip) << 32)
                                | ((n.inside_port as u128) << 16)
                                | (n.outside_port as u128)
                        },
                    )
                    .into_iter()
                    .map(|n| NatTransSnap {
                        inside_local: ip_str(n.inside_ip),
                        inside_global: outside_ip.map(ip_str).unwrap_or_default(),
                        outside: ip_str(n.peer_ip),
                        proto: n.proto,
                        inside_port: n.inside_port,
                        outside_port: n.outside_port,
                        created_tick: n.created_tick,
                    })
                    .collect();
                    NatSnap {
                        statics,
                        translations,
                    }
                });

                let dhcp = matches!(d.kind, DeviceKind::Router | DeviceKind::Switch).then(|| {
                    let excluded: Vec<DhcpExcludedSnap> = rank(
                        d.dhcp_excluded.iter(),
                        |e| (ip_key(e.lo) << 32) | ip_key(e.hi.unwrap_or([0; 4])),
                    )
                    .into_iter()
                    .map(|e| DhcpExcludedSnap {
                        lo: ip_str(e.lo),
                        hi: e.hi.map(ip_str),
                    })
                    .collect();
                    let leases: Vec<LeaseSnap> = rank(d.leases.iter(), |(_, l)| ip_key(l.ip))
                        .into_iter()
                        .map(|(mac, l)| LeaseSnap {
                            ip: ip_str(l.ip),
                            mac: mac_str(*mac),
                            granted_tick: l.granted_tick,
                            expires_tick: l.expires_tick,
                        })
                        .collect();
                    DhcpSnap {
                        service: d.svc_dhcp,
                        excluded,
                        leases,
                    }
                });

                let acls: Vec<AclSnap> = rank(d.acls.iter(), |a| a.num as u128)
                    .into_iter()
                    .map(|a| AclSnap {
                        num: a.num,
                        entries: a
                            .entries
                            .iter()
                            .map(|e| AceSnap {
                                permit: e.permit,
                                proto: e.proto,
                                src: ip_str(e.src),
                                src_wild: ip_str(e.src_wild),
                                dst: ip_str(e.dst),
                                dst_wild: ip_str(e.dst_wild),
                                sport: e.sport.unwrap_or(0),
                                dport: e.dport.unwrap_or(0),
                            })
                            .collect(),
                    })
                    .collect();

                let mut dns_records: Vec<(String, String)> = d
                    .dns_records
                    .iter()
                    .map(|(n, ip)| (n.clone(), ip_str(*ip)))
                    .collect();
                dns_records.sort_unstable_by(|a, b| a.0.cmp(&b.0));

                DeviceSnap {
                    id: d.id,
                    name: d.name.clone(),
                    kind: match d.kind {
                        DeviceKind::Host => "host",
                        DeviceKind::Switch => "switch",
                        DeviceKind::Router => "router",
                        DeviceKind::Ext => "ext",
                    },
                    x: d.x,
                    y: d.y,
                    powered: d.powered,
                    tick: self.tick,
                    ports: d
                        .ports
                        .iter()
                        .enumerate()
                        .map(|(p_i, p)| PortSnap {
                            name: Device::render_port_name(d.kind, p_i),
                            link: p.link,
                            admin_up: p.admin_up,
                            link_up: p.link_up,
                            ip: p.ip.map(|(a, _)| ip_str(a)),
                            mask: p.ip.map(|(_, m)| ip_str(m)),
                            method: match p.method {
                                IpMethod::None => "none",
                                IpMethod::Manual => "manual",
                                IpMethod::Dhcp => "dhcp",
                            },
                            description: p.description.clone(),
                            mac: mac_str(p.mac),
                            nat: match p.nat {
                                NatRole::Inside => "inside",
                                NatRole::Outside => "outside",
                                NatRole::Off => "",
                            },
                            access_vlan: p.access_vlan,
                            trunk: p.trunk,
                            native_vlan: p.native_vlan,
                            allowed: p.allowed.clone(),
                            in_acl: p.in_acl,
                            out_acl: p.out_acl,
                            helper: p.helper.map(ip_str),
                            ospf_area: d.ospf.if_area.get(p_i).copied().flatten(),
                            rx: p.stats.rx,
                            tx: p.stats.tx,
                            drops: p.stats.drops,
                            rx_err: p.stats.rx_err,
                            tx_err: p.stats.tx_err,
                        })
                        .collect(),
                    routes,
                    mac_table,
                    arp_table,
                    vlans,
                    ospf,
                    nat,
                    dhcp,
                    dhcp_client: d
                        .dhcp
                        .as_ref()
                        .map(|_| crate::services::dhcp_state(d).to_string()),
                    toggles: ToggleSnap {
                        domain_lookup: d.toggles.domain_lookup,
                        routing_enabled: d.toggles.routing_enabled,
                        proxy_arp: d.toggles.proxy_arp,
                        directed_broadcast: d.toggles.directed_broadcast,
                        ip_routing: d.toggles.ip_routing,
                    },
                    acls,
                    resolver: d.resolver.map(ip_str),
                    svc_dhcp: d.svc_dhcp,
                    svc_dns: d.svc_dns,
                    svc_http: d.svc_http,
                    dns_records,
                }
            })
            .collect();
        let links = rank(
            self.links.values().map(|l| (l.id, l)),
            |(id, _)| *id as u128,
        )
        .into_iter()
        .map(|(_, l)| LinkSnap {
                id: l.id,
                a: l.a.device,
                b: l.b.device,
                a_port: l.a.port,
                b_port: l.b.port,
                up: l.up,
                capture: l.capture,
                delay: l.delay,
                jitter: l.jitter,
                loss: l.loss,
                mtu: l.mtu,
                speed: l.speed.as_str(),
                frames: l
                    .in_flight
                    .iter()
                    .map(|f| FrameSnap {
                        t: if f.to == l.b {
                            f.progress as f32 / f.budget.max(1) as f32
                        } else {
                            1.0 - f.progress as f32 / f.budget.max(1) as f32
                        },
                        dir: if f.to == l.b { 1 } else { -1 },
                        color: f.frame.color_key(),
                        label: f.frame.label(),
                    })
                    .collect(),
            })
            .collect();
        Snapshot {
            tick: self.tick,
            devices,
            links,
        }
    }
}
