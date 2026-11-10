//! Serializable snapshot of the whole topology — the DTO the UI renders.

use serde::Serialize;

use crate::device::{DeviceId, DeviceKind, NatRole, RouteProto};
use crate::link::LinkId;
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
    pub ports: Vec<PortSnap>,
    pub routes: Vec<RouteSnap>,
    pub powered: bool,
    /// (ip, mac)
    pub arp: Vec<(String, String)>,
    /// (mac, port, vlan) — switches
    pub macs: Vec<(String, usize, u16)>,
    pub acls: Vec<AclSnap>,
    pub ospf_id: String,
    pub ospf_nets: Vec<OspfNetSnap>,
    pub ospf_nbrs: Vec<String>,
    pub dhcp: &'static str,
    pub resolver: Option<String>,
    pub svc_dhcp: bool,
    pub svc_dns: bool,
    pub svc_http: bool,
    pub dns_records: Vec<(String, String)>,
    pub static_nat: Vec<StaticNatSnap>,
}

#[derive(Serialize)]
pub struct StaticNatSnap {
    pub proto: u8,
    pub inside: String,
    pub inside_port: u16,
    pub outside_port: u16,
}

#[derive(Serialize)]
pub struct RouteSnap {
    pub prefix: String,
    pub mask: String,
    pub next_hop: String,
    pub out_port: usize,
    /// "S" static, "O" OSPF
    pub proto: &'static str,
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
pub struct OspfNetSnap {
    pub prefix: String,
    pub wild: String,
    pub area: u32,
}

#[derive(Serialize)]
pub struct PortSnap {
    pub ip: Option<String>,
    pub mask: Option<String>,
    pub mac: String,
    pub link: Option<LinkId>,
    /// "" | "inside" | "outside"
    pub nat: &'static str,
    pub access_vlan: u16,
    pub trunk: bool,
    pub native_vlan: u16,
    /// Comma-separated VIDs, empty means all (trunk) or unused (access).
    pub allowed: String,
    pub in_acl: u16,
    pub out_acl: u16,
    pub helper: String,
    /// OSPF area on this interface, when `ip ospf area` is set.
    pub ospf_area: Option<u32>,
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

impl Topology {
    pub fn snapshot(&self) -> Snapshot {
        let devices = self
            .devices
            .values()
            .map(|d| DeviceSnap {
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
                ports: d
                    .ports
                    .iter()
                    .enumerate()
                    .map(|(p_i, p)| PortSnap {
                        ip: p.ip.map(|(a, _)| ip_str(a)),
                        mask: p.ip.map(|(_, m)| ip_str(m)),
                        mac: mac_str(p.mac),
                        link: p.link,
                        nat: match p.nat {
                            NatRole::Inside => "inside",
                            NatRole::Outside => "outside",
                            NatRole::Off => "",
                        },
                        access_vlan: p.access_vlan,
                        trunk: p.trunk,
                        native_vlan: p.native_vlan,
                        allowed: p
                            .allowed
                            .as_ref()
                            .map(|v| {
                                v.iter()
                                    .map(|n| n.to_string())
                                    .collect::<Vec<_>>()
                                    .join(",")
                            })
                            .unwrap_or_default(),
                        in_acl: p.in_acl.unwrap_or(0),
                        out_acl: p.out_acl.unwrap_or(0),
                        helper: p.helper.map(ip_str).unwrap_or_default(),
                        ospf_area: d.ospf.if_area.get(p_i).copied().flatten(),
                    })
                    .collect(),
                routes: d
                    .routes
                    .iter()
                    .map(|r| RouteSnap {
                        prefix: ip_str(r.prefix),
                        mask: ip_str(r.mask),
                        next_hop: ip_str(r.next_hop),
                        out_port: r.out_port,
                        proto: if r.proto == RouteProto::Ospf {
                            "O"
                        } else {
                            "S"
                        },
                    })
                    .collect(),
                powered: d.powered,
                arp: d
                    .arp_cache
                    .iter()
                    .map(|(ip, mac)| (ip_str(*ip), mac_str(*mac)))
                    .collect(),
                macs: d
                    .mac_table
                    .iter()
                    .map(|((mac, vlan), p)| (mac_str(*mac), *p, *vlan))
                    .collect(),
                acls: d
                    .acls
                    .iter()
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
                    .collect(),
                ospf_id: if d.ospf.on {
                    ip_str(d.ospf.router_id)
                } else {
                    String::new()
                },
                ospf_nets: d
                    .ospf
                    .nets
                    .iter()
                    .map(|n| OspfNetSnap {
                        prefix: ip_str(n.prefix),
                        wild: ip_str(n.wild),
                        area: n.area,
                    })
                    .collect(),
                ospf_nbrs: d
                    .ospf
                    .nbrs
                    .iter()
                    .map(|n| format!("{} area {}", ip_str(n.ip), n.area))
                    .collect(),
                dhcp: crate::services::dhcp_state(d),
                resolver: d.resolver.map(ip_str),
                svc_dhcp: d.svc_dhcp,
                svc_dns: d.svc_dns,
                svc_http: d.svc_http,
                dns_records: d
                    .dns_records
                    .iter()
                    .map(|(n, ip)| (n.clone(), ip_str(*ip)))
                    .collect(),
                static_nat: d
                    .static_nat
                    .iter()
                    .map(|s| StaticNatSnap {
                        proto: s.proto,
                        inside: ip_str(s.inside_ip),
                        inside_port: s.inside_port,
                        outside_port: s.outside_port,
                    })
                    .collect(),
            })
            .collect();
        let links = self
            .links
            .values()
            .map(|l| LinkSnap {
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
