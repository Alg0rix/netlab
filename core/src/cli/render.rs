//! Show bank — honest engine state, sorted rows, no invented columns.
//! Map-derived rows follow the declared sort keys (item 29/30).

use super::nvgen;
use super::types::{Op, ShowKind};
use super::Cli;
use crate::device::{Device, DeviceKind, IpMethod, RouteProto, VlanState};
use crate::ip_str;

fn mac_str(mac: u64) -> String {
    format!(
        "{:02x}{:02x}.{:02x}{:02x}.{:02x}{:02x}",
        (mac >> 40) & 0xff,
        (mac >> 32) & 0xff,
        (mac >> 24) & 0xff,
        (mac >> 16) & 0xff,
        (mac >> 8) & 0xff,
        mac & 0xff
    )
}

fn plen(mask: [u8; 4]) -> u8 {
    mask.iter().map(|b| b.count_ones() as u8).sum()
}

fn lines(ls: impl IntoIterator<Item = String>) -> Vec<Op> {
    ls.into_iter().map(Op::text).collect()
}

pub fn show(cli: &Cli, d: &Device, k: ShowKind, tick: u64) -> Vec<Op> {
    match k {
        ShowKind::IpIntBrief => ip_int_brief(d),
        ShowKind::IpRoute => ip_route(d),
        ShowKind::Arp => arp(d, tick),
        ShowKind::MacAddrTable => mac_table(d),
        ShowKind::AccessLists => access_lists(d),
        ShowKind::Ospf => ospf(d),
        ShowKind::OspfNeighbor => ospf_neighbor(d, tick),
        ShowKind::OspfInterface => ospf_interface(d),
        ShowKind::NatTranslations => nat_translations(d),
        ShowKind::Dhcp => dhcp(d, tick),
        ShowKind::Protocols => protocols(d),
        ShowKind::Interfaces => interfaces(d),
        ShowKind::Switchport => switchport(d),
        ShowKind::Vlan => vlan(d),
        ShowKind::Status => status(d, tick),
        ShowKind::History => history(cli),
        ShowKind::Version => version(d),
        ShowKind::StartupConfig => startup_config(cli),
        ShowKind::RunningConfig => running_config(d),
        ShowKind::LoggingBuffered => logging(d),
        ShowKind::HostIp => host_ip(d),
    }
}

fn ip_int_brief(d: &Device) -> Vec<Op> {
    let mut ls = vec![
        "Interface              IP-Address      OK? Method Status                Protocol".to_string(),
    ];
    for (i, p) in d.ports.iter().enumerate() {
        let name = Device::render_port_name(d.kind, i);
        let addr = p.ip.map(|(a, _)| ip_str(a)).unwrap_or_else(|| "unassigned".into());
        let method = match p.method {
            IpMethod::None => "unset",
            IpMethod::Manual => "manual",
            IpMethod::Dhcp => "DHCP",
        };
        let status = if p.admin_up { "up" } else { "administratively down" };
        let proto = if p.admin_up && p.link_up { "up" } else { "down" };
        ls.push(format!(
            "{name:<23}{addr:<16}YES {method:<7}{status:<22}{proto}"
        ));
    }
    lines(ls)
}

fn proto_letter(r: &crate::device::Route) -> &'static str {
    match r.proto {
        RouteProto::Connected => "C",
        RouteProto::Static => "S",
        RouteProto::Ospf => "O",
    }
}

fn ip_route(d: &Device) -> Vec<Op> {
    let mut rs: Vec<_> = d.routes.iter().collect();
    rs.sort_by(|a, b| (a.prefix, a.mask, a.next_hop).cmp(&(b.prefix, b.mask, b.next_hop)));
    let mut ls = vec![
        "Codes: C - connected, S - static, O - OSPF".to_string(),
        String::new(),
    ];
    for r in rs {
        let name = Device::render_port_name(d.kind, r.out_port);
        let prefix = format!("{}/{}", ip_str(r.prefix), plen(r.mask));
        if r.connected {
            ls.push(format!("C    {prefix:<16} is directly connected, {name}"));
        } else {
            ls.push(format!(
                "{}    {prefix:<16} [{}/{}] via {}, {}",
                proto_letter(r),
                r.ad,
                r.metric,
                ip_str(r.next_hop),
                name
            ));
        }
    }
    if ls.len() == 2 {
        ls.push("% No routes learned".into());
    }
    lines(ls)
}

fn arp(d: &Device, tick: u64) -> Vec<Op> {
    let mut rows: Vec<_> = d.arp_cache.iter().collect();
    rows.sort_by_key(|(ip, _)| *ip);
    let mut ls = vec![
        "Protocol  Address          Age (min)  Hardware Addr   Type   Interface".to_string(),
    ];
    for (ip, e) in rows {
        let age = tick.saturating_sub(e.created_tick) / 60;
        let ty = match e.etype {
            crate::device::EntryType::Static => "static",
            crate::device::EntryType::Dynamic => "dynamic",
        };
        ls.push(format!(
            "Internet  {:<15}{:<11}{:<16}{:<7}{}",
            ip_str(*ip),
            format!("{age}"),
            mac_str(e.mac),
            format!("ARPA/{ty}"),
            Device::render_port_name(d.kind, e.port)
        ));
    }
    lines(ls)
}

fn mac_table(d: &Device) -> Vec<Op> {
    let mut rows: Vec<_> = d.mac_table.iter().collect();
    rows.sort_by_key(|(_, e)| (e.port, 0));
    let mut rows2: Vec<((u64, u16), &crate::device::MacEntry)> =
        rows.into_iter().map(|(k, e)| (*k, e)).collect();
    rows2.sort_by(|((m1, _), e1), ((m2, _), e2)| (e1.port, *m1).cmp(&(e2.port, *m2)));
    let mut ls = vec![
        "          Mac Address Table".to_string(),
        "-------------------------------------------".to_string(),
        String::new(),
        "Vlan    Mac Address       Type        Ports".to_string(),
        "----    -----------       --------    -----".to_string(),
    ];
    for ((mac, vid), e) in rows2 {
        let ty = match e.etype {
            crate::device::EntryType::Static => "static",
            crate::device::EntryType::Dynamic => "dynamic",
        };
        ls.push(format!(
            "{vid:<8}{:<18}{:<12}{}",
            mac_str(mac),
            ty,
            Device::render_port_name(d.kind, e.port)
        ));
    }
    lines(ls)
}

fn access_lists(d: &Device) -> Vec<Op> {
    let mut ls = Vec::new();
    for acl in &d.acls {
        ls.push(format!("Extended IP access list {}", acl.num));
        for (i, a) in acl.entries.iter().enumerate() {
            let act = if a.permit { "permit" } else { "deny" };
            let proto = match a.proto {
                1 => "icmp",
                6 => "tcp",
                17 => "udp",
                _ => "ip",
            };
            let src = ace_addr(a.src, a.src_wild);
            let dst = ace_addr(a.dst, a.dst_wild);
            ls.push(format!("    {} {} {} {} {}", (i + 1) * 10, act, proto, src, dst));
        }
    }
    if ls.is_empty() {
        ls.push("% No access lists configured".into());
    }
    lines(ls)
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

fn ospf(d: &Device) -> Vec<Op> {
    if !d.ospf.on {
        return lines(["% OSPF not configured".to_string()]);
    }
    let mut ls = vec![format!(
        " Routing Process \"ospf {}\" with ID {}",
        d.ospf.pid,
        ip_str(d.ospf.router_id.value)
    )];
    let mut nets: Vec<_> = d.ospf.nets.iter().collect();
    nets.sort_by(|a, b| (a.prefix, a.wild).cmp(&(b.prefix, b.wild)));
    for n in nets {
        ls.push(format!(
            "    Network {} {} area {}",
            ip_str(n.prefix),
            ip_str(n.wild),
            n.area
        ));
    }
    lines(ls)
}

fn ospf_neighbor(d: &Device, tick: u64) -> Vec<Op> {
    if !d.ospf.on {
        return lines(["% OSPF not configured".to_string()]);
    }
    let mut rows: Vec<_> = d.ospf.nbrs.iter().collect();
    rows.sort_by_key(|n| n.rid);
    let mut ls = vec![
        "Neighbor ID     Pri   State           Dead Time   Address         Interface".to_string(),
    ];
    for n in rows {
        let dead = n.last_hello.saturating_add(40).saturating_sub(tick);
        ls.push(format!(
            "{:<16}1     {:<16}{:<12}{:<16}{}",
            ip_str(n.rid),
            n.state.as_str(),
            format!("{dead}"),
            ip_str(n.ip),
            Device::render_port_name(d.kind, n.port)
        ));
    }
    lines(ls)
}

fn ospf_interface(d: &Device) -> Vec<Op> {
    if !d.ospf.on {
        return lines(["% OSPF not configured".to_string()]);
    }
    let mut ls = Vec::new();
    for i in 0..d.ports.len() {
        if let Some(area) = crate::ospf::area_of(d, i) {
            let p = &d.ports[i];
            ls.push(format!(
                "{} is {}, line protocol is {}",
                Device::render_port_name(d.kind, i),
                if p.admin_up { "up" } else { "down" },
                if p.link_up { "up" } else { "down" }
            ));
            ls.push(format!("  Internet Address {}, Area {}", p.ip.map(|(a, _)| ip_str(a)).unwrap_or_else(|| "unassigned".into()), area));
        }
    }
    if ls.is_empty() {
        ls.push("% No OSPF-enabled interfaces".into());
    }
    lines(ls)
}

fn nat_translations(d: &Device) -> Vec<Op> {
    let mut ls = vec![
        "Pro Inside global      Inside local       Outside local      Outside global".to_string(),
    ];
    let mut stat: Vec<_> = d.static_nat.iter().collect();
    stat.sort_by_key(|s| s.inside_ip);
    for s in stat {
        ls.push(format!(
            "--- {:<21}{}",
            ip_str(s.inside_global.unwrap_or([0; 4])),
            ip_str(s.inside_ip)
        ));
    }
    let mut trans: Vec<_> = d.nat.iter().collect();
    trans.sort_by_key(|e| e.inside_ip);
    for e in trans {
        let proto = match e.proto {
            6 => "tcp",
            17 => "udp",
            1 => "icmp",
            _ => "---",
        };
        ls.push(format!(
            "{} {:<21}{}",
            proto,
            ip_str(e.peer_ip),
            ip_str(e.inside_ip)
        ));
    }
    lines(ls)
}

fn dhcp(d: &Device, tick: u64) -> Vec<Op> {
    let mut rows: Vec<_> = d.leases.iter().collect();
    rows.sort_by_key(|(_, e)| e.ip);
    let mut ls = vec![
        "IP address       Client-Identifier        Lease expiration     Type".to_string(),
    ];
    for (mac, e) in rows {
        let exp = e
            .expires_tick
            .map(|t| format!("{}s", t.saturating_sub(tick)))
            .unwrap_or_else(|| "Infinite".into());
        ls.push(format!(
            "{:<17}{:<25}{:<21}Automatic",
            ip_str(e.ip),
            mac_str(*mac),
            exp
        ));
    }
    if ls.len() == 1 {
        ls.push("% No DHCP bindings".into());
    }
    lines(ls)
}

fn protocols(d: &Device) -> Vec<Op> {
    let mut ls = Vec::new();
    if d.toggles.ip_routing {
        ls.push("Routing Protocol is enabled".to_string());
    } else {
        ls.push("Routing Protocol is disabled".to_string());
    }
    if d.ospf.on {
        ls.push(format!("Routing Protocol is \"ospf {}\"", d.ospf.pid));
    }
    lines(ls)
}

fn interfaces(d: &Device) -> Vec<Op> {
    let mut ls = Vec::new();
    for (i, p) in d.ports.iter().enumerate() {
        let name = Device::render_port_name(d.kind, i);
        let state = if p.admin_up && p.link_up {
            "up"
        } else if !p.admin_up {
            "administratively down"
        } else {
            "down"
        };
        ls.push(format!("{name} is {state}, line protocol is {state}"));
        if let Some(desc) = &p.description {
            ls.push(format!("  Description: {desc}"));
        }
        ls.push(format!(
            "  MTU 1500 bytes, {} packets input, {} packets output",
            p.stats.rx, p.stats.tx
        ));
    }
    lines(ls)
}

fn switchport(d: &Device) -> Vec<Op> {
    let mut ls = Vec::new();
    for (i, p) in d.ports.iter().enumerate() {
        let name = Device::render_port_name(d.kind, i);
        let mode = if p.trunk { "trunk" } else { "access" };
        ls.push(format!("Name: {name}"));
        ls.push("Switchport: Enabled".into());
        ls.push(format!("Administrative Mode: static {mode}"));
        ls.push(format!("Operational Mode: {mode}"));
        ls.push(format!("Access Mode VLAN: {}", p.access_vlan));
        if p.trunk {
            let allowed = p
                .allowed
                .as_ref()
                .map(|v| {
                    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
                })
                .unwrap_or_else(|| "ALL".into());
            ls.push(format!("Trunking VLANs Allowed: {allowed}"));
        }
    }
    lines(ls)
}

fn vlan(d: &Device) -> Vec<Op> {
    let mut ids: Vec<_> = d.vlans.keys().copied().collect();
    ids.sort_unstable();
    let mut ls = vec![
        "VLAN Name                             Status    Ports".to_string(),
        "---- -------------------------------- --------- -------------------------------".to_string(),
    ];
    for id in ids {
        let v = &d.vlans[&id];
        let st = match v.state {
            VlanState::Active => "active",
            VlanState::Suspend => "suspended",
        };
        let ports: Vec<String> = d
            .ports
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.trunk && p.access_vlan == id)
            .map(|(i, _)| Device::render_port_name(d.kind, i))
            .collect();
        ls.push(format!(
            "{id:<5}{:<38}{:<10}{}",
            v.name,
            st,
            ports.join(", ")
        ));
    }
    lines(ls)
}

fn status(d: &Device, tick: u64) -> Vec<Op> {
    let kind = match d.kind {
        DeviceKind::Router => "router",
        DeviceKind::Switch => "switch",
        DeviceKind::Host => "host",
        DeviceKind::Ext => "ext",
    };
    lines([
        format!("{} ({})", d.name, kind),
        format!("{} ports, tick {}", d.ports.len(), tick),
        format!("powered: {}", d.powered),
    ])
}

fn history(cli: &Cli) -> Vec<Op> {
    if cli.history.is_empty() {
        return lines(["% No history in this session".to_string()]);
    }
    lines(cli.history.iter().cloned())
}

fn version(d: &Device) -> Vec<Op> {
    let kind = match d.kind {
        DeviceKind::Router => "router",
        DeviceKind::Switch => "switch",
        DeviceKind::Host => "host",
        DeviceKind::Ext => "ext",
    };
    lines([
        "netlab-core CLI engine".to_string(),
        format!("device kind: {kind}"),
    ])
}

fn startup_config(cli: &Cli) -> Vec<Op> {
    match &cli.startup_text {
        None => lines(["%% Non-volatile configuration memory is not present".to_string()]),
        Some(t) => lines(t.lines().map(|l| l.to_string())),
    }
}

fn running_config(d: &Device) -> Vec<Op> {
    lines(nvgen::nvgen(d).lines().map(|l| l.to_string()))
}

fn logging(d: &Device) -> Vec<Op> {
    if d.log.is_empty() {
        return lines(["% Log buffer empty".to_string()]);
    }
    lines(d.log.iter().cloned())
}

/// `show ip` on a host — the VPCS-style address block.
fn host_ip(d: &Device) -> Vec<Op> {
    let p = match d.ports.first() {
        Some(p) => p,
        None => return lines(["% No interface".to_string()]),
    };
    let mut fields = vec![format!("NAME        : {}", d.name)];
    match p.ip {
        Some((ip, mask)) => {
            fields.push(format!("IP/MASK     : {}/{}", ip_str(ip), plen(mask)));
        }
        None => fields.push("IP/MASK     : 0.0.0.0/0".into()),
    }
    if let Some(gw) = d
        .routes
        .iter()
        .find(|r| r.prefix == [0; 4] && r.mask == [0; 4])
        .map(|r| r.next_hop)
    {
        fields.push(format!("GATEWAY     : {}", ip_str(gw)));
    }
    fields.push(format!("MAC         : {}", mac_str(p.mac)));
    if p.method == IpMethod::Dhcp {
        fields.push("DHCP".into());
    }
    lines([fields.join("\n")])
}
