//! Per-device configuration setters.

use crate::device::{Ace, Acl, DeviceId, DeviceKind, NatRole, Route, RouteProto};
use crate::parse_ip;
use crate::LinkId;
use crate::services::StaticNat;

use super::Topology;

/// One access-list entry as string fields — the flat labfile/wasm
/// surface for `Topology::acl_add`. `proto` is 0 (any), 1, 6 or 17;
/// `sport`/`dport` 0 means wildcard.
pub struct AceArgs<'a> {
    pub num: u16,
    pub permit: bool,
    pub proto: u8,
    pub src: &'a str,
    pub src_wild: &'a str,
    pub dst: &'a str,
    pub dst_wild: &'a str,
    pub sport: u16,
    pub dport: u16,
}

impl Topology {
    pub fn set_power(&mut self, id: DeviceId, on: bool) {
        if let Some(d) = self.devices.get_mut(&id) {
            if on {
                d.power_on();
            } else {
                d.power_off();
            }
        }
        // Carrier ends at a powered-off PHY: refresh both ends of every
        // attached port so the C1/C2 pair fires on both consoles.
        let ports: Vec<(usize, LinkId)> = self
            .devices
            .get(&id)
            .map(|d| {
                d.ports
                    .iter()
                    .enumerate()
                    .filter_map(|(i, p)| p.link.map(|l| (i, l)))
                    .collect()
            })
            .unwrap_or_default();
        for (port, link) in ports {
            let peer = self.links.get(&link).map(|l| {
                if l.a.device == id {
                    (l.b.device, l.b.port)
                } else {
                    (l.a.device, l.a.port)
                }
            });
            self.refresh_link_state(id, port);
            if let Some((pd, pp)) = peer {
                self.refresh_link_state(pd, pp);
            }
        }
    }

    pub fn clear_ip(&mut self, dev: DeviceId, port: usize) {
        if let Some(p) = self
            .devices
            .get_mut(&dev)
            .and_then(|d| d.ports.get_mut(port))
        {
            p.ip = None;
        }
    }

    /// Remove the n-th static route (index into the snapshot's `routes`).
    pub fn remove_route(&mut self, dev: DeviceId, idx: usize) {
        if let Some(d) = self.devices.get_mut(&dev) {
            if idx < d.routes.len() {
                d.routes.remove(idx);
            }
        }
    }

    /// `shutdown` / `no shutdown` — the kind-gated admin-state setter.
    pub fn set_admin(&mut self, dev: DeviceId, port: usize, up: bool) -> bool {
        let tick = self.tick;
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        crate::config::set_admin(d, port, up, tick, &mut self.events).is_ok()
    }

    pub fn set_ip(&mut self, dev: DeviceId, port: usize, ip: &str, mask: &str) -> bool {
        let (Some(ip), Some(mask)) = (parse_ip(ip), parse_ip(mask)) else {
            return false;
        };
        let tick = self.tick;
        if let Some(d) = self.devices.get_mut(&dev) {
            if let Some(p) = d.ports.get_mut(port) {
                p.ip = Some((ip, mask));
                p.method = crate::device::IpMethod::Manual;
                d.sync_connected(tick);
                return true;
            }
        }
        false
    }

    /// Convenience: default route via `gw` on the matching connected port (or port 0).
    pub fn set_gateway(&mut self, dev: DeviceId, gw: &str) -> bool {
        let Some(gw_ip) = parse_ip(gw) else {
            return false;
        };
        let tick = self.tick;
        if let Some(d) = self.devices.get_mut(&dev) {
            let out_port = d
                .ports
                .iter()
                .enumerate()
                .find_map(|(i, p)| {
                    p.ip.and_then(|(addr, mask)| {
                        (crate::net_of(addr, mask) == crate::net_of(gw_ip, mask)).then_some(i)
                    })
                })
                .unwrap_or(0);
            d.set_default_route(gw_ip, out_port, tick);
            return true;
        }
        false
    }

    pub fn add_route(
        &mut self,
        dev: DeviceId,
        prefix: &str,
        mask: &str,
        next_hop: &str,
        out_port: usize,
    ) -> bool {
        let (Some(prefix), Some(mask), Some(next_hop)) =
            (parse_ip(prefix), parse_ip(mask), parse_ip(next_hop))
        else {
            return false;
        };
        let tick = self.tick;
        if let Some(d) = self.devices.get_mut(&dev) {
            d.routes.push(Route {
                prefix,
                mask,
                next_hop,
                out_port,
                connected: false,
                proto: RouteProto::Static,
                ad: 1,
                metric: 0,
                learned_tick: tick,
            });
            return true;
        }
        false
    }

    pub fn set_services(&mut self, dev: DeviceId, dhcp: bool, dns: bool, http: bool) {
        if let Some(d) = self.devices.get_mut(&dev) {
            d.svc_dhcp = dhcp;
            d.svc_dns = dns;
            d.svc_http = http;
        }
    }

    pub fn set_dns(&mut self, dev: DeviceId, ip: &str) -> bool {
        if ip.is_empty() {
            if let Some(d) = self.devices.get_mut(&dev) {
                d.resolver = None;
            }
            return true;
        }
        let Some(ip) = parse_ip(ip) else { return false };
        if let Some(d) = self.devices.get_mut(&dev) {
            d.resolver = Some(ip);
            return true;
        }
        false
    }

    pub fn add_dns_record(&mut self, dev: DeviceId, name: &str, ip: &str) -> bool {
        let Some(ip) = parse_ip(ip) else { return false };
        let name = name.trim().trim_end_matches('.').to_ascii_lowercase();
        if name.is_empty() {
            return false;
        }
        if let Some(d) = self.devices.get_mut(&dev) {
            d.dns_records.retain(|(n, _)| n != &name);
            d.dns_records.push((name, ip));
            return true;
        }
        false
    }

    pub fn set_nat(&mut self, dev: DeviceId, port: usize, role: &str) -> bool {
        let role = match role {
            "inside" => NatRole::Inside,
            "outside" => NatRole::Outside,
            "off" | "none" | "" => NatRole::Off,
            _ => return false,
        };
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        if port >= d.ports.len() {
            return false;
        }
        if role == NatRole::Outside {
            for p in &mut d.ports {
                if p.nat == NatRole::Outside {
                    p.nat = NatRole::Off;
                }
            }
        }
        d.ports[port].nat = role;
        true
    }

    /// `proto` 0 is 1:1 (one map per router). 6 or 17 is a static port forward.
    pub fn nat_static(
        &mut self,
        dev: DeviceId,
        inside: &str,
        proto: u8,
        inside_port: u16,
        outside_port: u16,
    ) -> bool {
        let Some(ip) = parse_ip(inside) else {
            return false;
        };
        if proto != 0 && proto != 6 && proto != 17 {
            return false;
        }
        if proto != 0 && (inside_port == 0 || outside_port == 0) {
            return false;
        }
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        if proto == 0 {
            d.static_nat.retain(|s| s.proto != 0);
            d.static_nat.push(StaticNat {
                proto: 0,
                inside_ip: ip,
                inside_global: None,
                inside_port: 0,
                outside_port: 0,
            });
        } else {
            d.static_nat
                .retain(|s| !(s.proto == proto && s.outside_port == outside_port));
            d.static_nat.push(StaticNat {
                proto,
                inside_ip: ip,
                inside_global: None,
                inside_port,
                outside_port,
            });
        }
        true
    }

    pub fn nat_static_clear(
        &mut self,
        dev: DeviceId,
        inside: &str,
        proto: u8,
        outside_port: u16,
    ) -> bool {
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        let before = d.static_nat.len();
        if proto == 0 {
            let Some(ip) = parse_ip(inside) else {
                return false;
            };
            d.static_nat
                .retain(|s| !(s.proto == 0 && s.inside_ip == ip));
        } else {
            d.static_nat
                .retain(|s| !(s.proto == proto && s.outside_port == outside_port));
        }
        d.static_nat.len() != before
    }

    pub fn set_switchport(
        &mut self,
        dev: DeviceId,
        port: usize,
        trunk: bool,
        access: u16,
        native: u16,
        allowed: &str,
    ) -> bool {
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        if d.kind != DeviceKind::Switch {
            return false;
        }
        let Some(p) = d.ports.get_mut(port) else {
            return false;
        };
        p.trunk = trunk;
        p.access_vlan = if access == 0 { 1 } else { access.min(4094) };
        p.native_vlan = if native == 0 { 1 } else { native.min(4094) };
        let list: Vec<u16> = allowed
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("all"))
            .filter_map(|s| s.parse::<u16>().ok())
            .filter(|v| *v > 0 && *v < 4095)
            .collect();
        p.allowed = if list.is_empty() { None } else { Some(list) };
        d.mac_table.clear();
        true
    }

    /// Empty `ip` clears the helper. Limited broadcasts are still not routed.
    pub fn set_helper(&mut self, dev: DeviceId, port: usize, ip: &str) -> bool {
        let helper = if ip.is_empty() {
            None
        } else {
            match parse_ip(ip) {
                Some(ip) => Some(ip),
                None => return false,
            }
        };
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        let Some(p) = d.ports.get_mut(port) else {
            return false;
        };
        p.helper = helper;
        true
    }

    pub fn acl_clear(&mut self, dev: DeviceId, num: u16) {
        if let Some(d) = self.devices.get_mut(&dev) {
            d.acls.retain(|a| a.num != num);
            for p in &mut d.ports {
                if p.in_acl == Some(num) {
                    p.in_acl = None;
                }
                if p.out_acl == Some(num) {
                    p.out_acl = None;
                }
            }
        }
    }

    pub fn acl_add(&mut self, dev: DeviceId, spec: AceArgs<'_>) -> bool {
        let (Some(src), Some(src_wild), Some(dst), Some(dst_wild)) = (
            parse_ip(spec.src),
            parse_ip(spec.src_wild),
            parse_ip(spec.dst),
            parse_ip(spec.dst_wild),
        ) else {
            return false;
        };
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        let ace = Ace {
            permit: spec.permit,
            proto: spec.proto,
            src,
            src_wild,
            dst,
            dst_wild,
            sport: (spec.sport != 0).then_some(spec.sport),
            dport: (spec.dport != 0).then_some(spec.dport),
        };
        if let Some(a) = d.acls.iter_mut().find(|a| a.num == spec.num) {
            a.entries.push(ace);
        } else {
            d.acls.push(Acl {
                num: spec.num,
                entries: vec![ace],
            });
        }
        true
    }

    /// `dir` is "in", "out", "off-in" or "off-out". `num` 0 with off clears.
    pub fn set_access_group(&mut self, dev: DeviceId, port: usize, num: u16, dir: &str) -> bool {
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        let Some(p) = d.ports.get_mut(port) else {
            return false;
        };
        match dir {
            "in" => p.in_acl = Some(num),
            "out" => p.out_acl = Some(num),
            "off-in" => p.in_acl = None,
            "off-out" => p.out_acl = None,
            _ => return false,
        }
        true
    }

    pub fn ospf_network(&mut self, dev: DeviceId, prefix: &str, wild: &str, area: u32) -> bool {
        let (Some(prefix), Some(wild)) = (parse_ip(prefix), parse_ip(wild)) else {
            return false;
        };
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        if !d.ospf.on {
            d.ospf.on = true;
            d.ospf.pid = 1;
            d.ospf.router_id = crate::ospf::derive_router_id(d);
        }
        d.ospf
            .nets
            .retain(|n| !(n.prefix == prefix && n.wild == wild));
        d.ospf
            .nets
            .push(crate::ospf::OspfNet { prefix, wild, area });
        true
    }

    pub fn ospf_interface(&mut self, dev: DeviceId, port: usize, area: Option<u32>) -> bool {
        let Some(d) = self.devices.get_mut(&dev) else {
            return false;
        };
        if port >= d.ports.len() {
            return false;
        }
        if d.ospf.if_area.len() < d.ports.len() {
            d.ospf.if_area.resize(d.ports.len(), None);
        }
        d.ospf.if_area[port] = area;
        if area.is_some() && !d.ospf.on {
            d.ospf.on = true;
            d.ospf.pid = 1;
            d.ospf.router_id = crate::ospf::derive_router_id(d);
        }
        true
    }

    pub fn ospf_clear(&mut self, dev: DeviceId) {
        if let Some(d) = self.devices.get_mut(&dev) {
            d.ospf = crate::ospf::Ospf::default();
            d.routes.retain(|r| r.proto != RouteProto::Ospf);
        }
    }
}
