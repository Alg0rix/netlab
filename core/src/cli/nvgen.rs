//! nvgen — canonical running-config text (K14 stanza order). Router and
//! switch only; hosts never run nvgen (no save/write/copy heads).

use crate::config::ToggleKind;
use crate::device::{Device, DeviceKind, NatRole, RouteProto, VlanState};
use crate::ip_str;

fn vlan_default_name(id: u16) -> String {
    format!("VLAN{id:04}")
}

fn admin_default(d: &Device) -> bool {
    d.kind != DeviceKind::Router
}

/// Canonical nvgen text for the device. A single `!` separates the
/// interface/vlan/ospf/route/acl/nat/dhcp stanza groups.
pub fn nvgen(d: &Device) -> String {
    let mut ls: Vec<String> = Vec::new();

    let def_host = match d.kind {
        DeviceKind::Router => "Router",
        DeviceKind::Switch => "Switch",
        _ => "PC",
    };
    if d.name != def_host {
        ls.push(format!("hostname {}", d.name));
    }

    if d.toggles.domain_lookup != ToggleKind::DomainLookup.default_for(d.kind) {
        ls.push("no ip domain-lookup".into());
    }
    if d.toggles.ip_routing != ToggleKind::IpRouting.default_for(d.kind) {
        if d.toggles.ip_routing {
            ls.push("ip routing".into());
        } else {
            ls.push("no ip routing".into());
        }
    }
    if d.toggles.proxy_arp != ToggleKind::ProxyArp.default_for(d.kind) {
        ls.push("no ip proxy-arp".into());
    }
    if d.toggles.directed_broadcast != ToggleKind::DirectedBroadcast.default_for(d.kind) {
        ls.push("ip directed-broadcast".into());
    }

    let if_groups: Vec<Vec<String>> = d
        .ports
        .iter()
        .enumerate()
        .filter_map(|(i, p)| if_stanza(d, i, p))
        .collect();
    if !if_groups.is_empty() {
        ls.push("!".into());
        for (n, stanza) in if_groups.iter().enumerate() {
            if n > 0 {
                ls.push("!".into());
            }
            ls.extend(stanza.iter().cloned());
        }
    }

    if d.kind == DeviceKind::Switch {
        let mut ids: Vec<u16> = d
            .vlans
            .iter()
            .filter(|(id, v)| {
                !(crate::config::RESERVED_VLANS.contains(id)
                    && v.name == vlan_default_name(**id)
                    && v.state == VlanState::Active)
            })
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        if !ids.is_empty() {
            ls.push("!".into());
            for id in ids {
                let v = &d.vlans[&id];
                ls.push(format!("vlan {id}"));
                if v.name != vlan_default_name(id) {
                    ls.push(format!(" name {}", v.name));
                }
                if v.state == VlanState::Suspend {
                    ls.push(" state suspend".into());
                }
            }
        }
    }

    if d.ospf.on {
        ls.push("!".into());
        ls.push(format!("router ospf {}", d.ospf.pid));
        if d.ospf.router_id.explicit {
            ls.push(format!(" router-id {}", ip_str(d.ospf.router_id.value)));
        }
        let mut nets: Vec<_> = d.ospf.nets.iter().collect();
        nets.sort_by(|a, b| (a.prefix, a.wild, a.area).cmp(&(b.prefix, b.wild, b.area)));
        for n in nets {
            ls.push(format!(
                " network {} {} area {}",
                ip_str(n.prefix),
                ip_str(n.wild),
                n.area
            ));
        }
    }

    let mut stat: Vec<_> = d
        .routes
        .iter()
        .filter(|r| r.proto == RouteProto::Static)
        .collect();
    stat.sort_by(|a, b| (a.prefix, a.mask, a.next_hop).cmp(&(b.prefix, b.mask, b.next_hop)));
    if !stat.is_empty() {
        ls.push("!".into());
        for r in stat {
            let mut line = format!(
                "ip route {} {} {}",
                ip_str(r.prefix),
                ip_str(r.mask),
                ip_str(r.next_hop)
            );
            if r.ad != 1 {
                line.push_str(&format!(" {}", r.ad));
            }
            ls.push(line);
        }
    }

    if !d.acls.is_empty() {
        ls.push("!".into());
        for acl in &d.acls {
            for a in &acl.entries {
                let act = if a.permit { "permit" } else { "deny" };
                let proto = match a.proto {
                    1 => "icmp",
                    6 => "tcp",
                    17 => "udp",
                    _ => "ip",
                };
                let src = ace_addr(a.src, a.src_wild);
                let dst = ace_addr(a.dst, a.dst_wild);
                ls.push(format!("access-list {} {} {} {} {}", acl.num, act, proto, src, dst));
            }
        }
    }

    let mut nats: Vec<_> = d.static_nat.iter().filter(|s| s.proto == 0).collect();
    nats.sort_by_key(|s| s.inside_ip);
    if !nats.is_empty() {
        ls.push("!".into());
        for s in nats {
            ls.push(format!(
                "ip nat inside source static {} {}",
                ip_str(s.inside_ip),
                ip_str(s.inside_global.unwrap_or([0; 4]))
            ));
        }
    }

    {
        let mut excl: Vec<_> = d.dhcp_excluded.iter().collect();
        excl.sort_by(|a, b| (a.lo, a.hi).cmp(&(b.lo, b.hi)));
        let svc_off = !d.svc_dhcp;
        if !excl.is_empty() || svc_off {
            ls.push("!".into());
            for e in excl {
                match e.hi {
                    Some(hi) => ls.push(format!(
                        "ip dhcp excluded-address {} {}",
                        ip_str(e.lo),
                        ip_str(hi)
                    )),
                    None => ls.push(format!("ip dhcp excluded-address {}", ip_str(e.lo))),
                }
            }
            if svc_off {
                ls.push("no service dhcp".into());
            }
        }
    }

    ls.push("end".into());
    ls.join("\n")
}

fn ace_addr(ip: [u8; 4], wild: [u8; 4]) -> String {
    if wild == [255; 4] {
        "any".into()
    } else if wild == [0; 4] {
        format!("host {}", ip_str(ip))
    } else {
        format!("{} {}", ip_str(ip), ip_str(wild))
    }
}

/// One interface stanza, or None when every field is kind-default.
fn if_stanza(d: &Device, i: usize, p: &crate::device::Port) -> Option<Vec<String>> {
    let mut member: Vec<String> = Vec::new();
    if let Some(desc) = &p.description {
        member.push(format!(" description {desc}"));
    }
    if d.kind == DeviceKind::Switch {
        if p.trunk {
            member.push(" switchport mode trunk".into());
        }
        if p.access_vlan != 1 {
            member.push(format!(" switchport access vlan {}", p.access_vlan));
        }
        if let Some(allowed) = &p.allowed {
            let v: Vec<String> = allowed.iter().map(|x| x.to_string()).collect();
            member.push(format!(" switchport trunk allowed vlan {}", v.join(",")));
        }
    }
    if let Some((ip, mask)) = p.ip {
        member.push(format!(" ip address {} {}", ip_str(ip), ip_str(mask)));
    }
    if let Some(n) = p.in_acl {
        member.push(format!(" ip access-group {n} in"));
    }
    if let Some(n) = p.out_acl {
        member.push(format!(" ip access-group {n} out"));
    }
    match p.nat {
        NatRole::Inside => member.push(" ip nat inside".into()),
        NatRole::Outside => member.push(" ip nat outside".into()),
        NatRole::Off => {}
    }
    // admin line always emits — exempt from defaults suppression
    if p.admin_up {
        member.push(" no shutdown".into());
    } else {
        member.push(" shutdown".into());
    }

    // 'configured' iff any member line is non-default — the admin line
    // counts when it differs from the kind default
    let differs = member.len() > 1
        || (member.len() == 1 && p.admin_up != admin_default(d));
    if !differs {
        return None;
    }
    let mut stanza = vec![format!("interface {}", Device::render_port_name(d.kind, i))];
    stanza.extend(member);
    Some(stanza)
}
