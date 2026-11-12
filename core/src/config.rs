//! Kind-gated configuration setters. The CLI dispatch path and the direct
//! wasm/labfile bypass path both land here, so the kind-gate oracle lives
//! inside the setters themselves.

use crate::device::{Ace, Acl, Device, DeviceKind, IpMethod, NatRole, Route, RouteProto, Vlan, VlanState};
use crate::events::{EvKind, SimEvent};
use crate::net_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetErr {
    KindGate,
    BadMask,
    Overlaps,
    Inconsistent,
    Conflict,
    Reserved,
    /// Control characters would inject lines on nvgen script replay.
    BadChars,
    /// Outside the enforced domain (e.g. vlan id 1-4094).
    BadValue,
}

pub const RESERVED_VLANS: [u16; 5] = [1, 1002, 1003, 1004, 1005];

/// Names and descriptions must not carry control characters — a `\n`
/// would land a second config line at column 0 on nvgen replay.
/// `is_control` misses the invisible/bidi format characters (Cf:
/// U+200B–U+200F, U+202A–U+202E, U+2060–U+2064, U+FEFF) and the
/// line/paragraph separators, so they are rejected alongside.
fn text_clean(s: &str) -> Result<(), SetErr> {
    if s.chars().any(|c| {
        c.is_control()
            || matches!(c,
                '\u{2028}' | '\u{2029}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{FEFF}')
    }) {
        Err(SetErr::BadChars)
    } else {
        Ok(())
    }
}

fn contig_mask(m: [u8; 4]) -> bool {
    let inv = !u32::from_be_bytes(m);
    inv == 0 || inv & inv.wrapping_add(1) == 0
}

fn modeful(d: &Device) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router | DeviceKind::Switch => Ok(()),
        _ => Err(SetErr::KindGate),
    }
}

pub fn set_hostname(d: &mut Device, name: &str) -> Result<(), SetErr> {
    modeful(d)?;
    text_clean(name)?;
    d.name = name.to_string();
    Ok(())
}

pub fn reset_hostname(d: &mut Device) -> Result<(), SetErr> {
    modeful(d)?;
    d.name = match d.kind {
        DeviceKind::Router => "Router",
        DeviceKind::Switch => "Switch",
        _ => "PC",
    }
    .to_string();
    Ok(())
}

pub fn set_pcname(d: &mut Device, name: &str) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Host => {
            text_clean(name)?;
            d.name = name.to_string();
            Ok(())
        }
        _ => Err(SetErr::KindGate),
    }
}

pub fn set_admin(
    d: &mut Device,
    port: usize,
    up: bool,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> Result<(), SetErr> {
    modeful(d)?;
    let Some(p) = d.ports.get_mut(port) else {
        return Err(SetErr::KindGate);
    };
    let was = p.admin_up && p.link_up;
    p.admin_up = up;
    let now = up && p.link_up;
    if now != was {
        port_state_events(d, port, now, tick, events);
        if !now {
            crate::ospf::port_down(d, port, tick, events);
        }
    }
    d.sync_connected(tick);
    Ok(())
}

/// C1/C2 link event pair plus the buffered-log line for a port-state change.
pub(crate) fn port_state_events(
    d: &mut Device,
    port: usize,
    up: bool,
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let ifname = Device::render_port_name(d.kind, port);
    let link = d.ports.get(port).and_then(|p| p.link);
    let dir = if up { "up" } else { "down" };
    let msgs = [
        format!("%LINK-5-CHANGED: Interface {ifname}, changed state to {dir}"),
        format!("%LINEPROTO-5-UPDOWN: Line protocol on Interface {ifname}, changed state to {dir}"),
    ];
    for msg in msgs {
        d.log.push_back(msg.clone());
        if d.log.len() > 64 {
            d.log.pop_front();
        }
        events.push(SimEvent::new(tick, d.id, link, EvKind::Link, msg));
    }
}

/// Shared path for every carrier/admin effective-state transition.
pub(crate) fn effective_changed(
    d: &mut Device,
    port: usize,
    was_up: bool,
    now_up: bool,
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    if was_up != now_up {
        port_state_events(d, port, now_up, tick, events);
        if !now_up {
            crate::ospf::port_down(d, port, tick, events);
        }
        d.sync_connected(tick);
    }
}

pub fn set_ip(
    d: &mut Device,
    port: usize,
    ip: [u8; 4],
    mask: [u8; 4],
    tick: u64,
) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    if !contig_mask(mask) {
        return Err(SetErr::BadMask);
    }
    for (i, p) in d.ports.iter().enumerate() {
        if i == port {
            continue;
        }
        if let Some((a, m)) = p.ip {
            if net_of(a, m) == net_of(ip, m) {
                return Err(SetErr::Overlaps);
            }
        }
    }
    let Some(p) = d.ports.get_mut(port) else {
        return Err(SetErr::KindGate);
    };
    p.ip = Some((ip, mask));
    p.method = IpMethod::Manual;
    d.sync_connected(tick);
    Ok(())
}

/// `no ip address` — clears address and method, re-derives connected routes.
pub fn clear_ip(d: &mut Device, port: usize, tick: u64) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    let Some(p) = d.ports.get_mut(port) else {
        return Ok(false);
    };
    let had = p.ip.is_some();
    p.ip = None;
    p.method = IpMethod::None;
    d.sync_connected(tick);
    Ok(had)
}

/// Host `ip <ip> <mask> [<gw>]` — binds the sole host port.
pub fn set_host_ip(
    d: &mut Device,
    ip: [u8; 4],
    mask: [u8; 4],
    gw: Option<[u8; 4]>,
    tick: u64,
) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Host => {}
        _ => return Err(SetErr::KindGate),
    }
    if !contig_mask(mask) {
        return Err(SetErr::BadMask);
    }
    if d.ports.is_empty() {
        return Err(SetErr::KindGate);
    }
    d.ports[0].ip = Some((ip, mask));
    d.ports[0].method = IpMethod::Manual;
    if let Some(gw) = gw {
        d.set_default_route(gw, 0, tick);
    }
    d.sync_connected(tick);
    Ok(())
}

/// Host `ip dhcp` — marks the sole port dhcp and starts discovery.
pub fn host_dhcp(
    d: &mut Device,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Host => {}
        _ => return Err(SetErr::KindGate),
    }
    if let Some(p) = d.ports.first_mut() {
        p.method = IpMethod::Dhcp;
    }
    crate::services::start_dhcp(d, 0, tick, events);
    Ok(())
}

pub fn set_description(d: &mut Device, port: usize, text: &str) -> Result<(), SetErr> {
    modeful(d)?;
    text_clean(text)?;
    let Some(p) = d.ports.get_mut(port) else {
        return Err(SetErr::KindGate);
    };
    p.description = Some(text.to_string());
    Ok(())
}

pub fn clear_description(d: &mut Device, port: usize) -> Result<bool, SetErr> {
    modeful(d)?;
    let Some(p) = d.ports.get_mut(port) else {
        return Ok(false);
    };
    Ok(p.description.take().is_some())
}

fn switch_gate(d: &Device) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Switch => Ok(()),
        _ => Err(SetErr::KindGate),
    }
}

pub fn set_sw_mode(d: &mut Device, port: usize, trunk: bool) -> Result<(), SetErr> {
    switch_gate(d)?;
    let Some(p) = d.ports.get_mut(port) else {
        return Err(SetErr::KindGate);
    };
    p.trunk = trunk;
    d.mac_table.clear();
    Ok(())
}

pub fn reset_sw_mode(d: &mut Device, port: usize) -> Result<bool, SetErr> {
    switch_gate(d)?;
    let Some(p) = d.ports.get_mut(port) else {
        return Ok(false);
    };
    let was = p.trunk;
    p.trunk = false;
    if was {
        d.mac_table.clear();
    }
    Ok(was)
}

pub fn set_sw_access_vlan(d: &mut Device, port: usize, vid: u16) -> Result<(), SetErr> {
    switch_gate(d)?;
    if !(1..=4094).contains(&vid) {
        return Err(SetErr::BadValue);
    }
    let Some(p) = d.ports.get_mut(port) else {
        return Err(SetErr::KindGate);
    };
    p.access_vlan = vid;
    d.mac_table.clear();
    Ok(())
}

pub fn reset_sw_access_vlan(d: &mut Device, port: usize) -> Result<bool, SetErr> {
    switch_gate(d)?;
    let Some(p) = d.ports.get_mut(port) else {
        return Ok(false);
    };
    let was = p.access_vlan != 1;
    p.access_vlan = 1;
    if was {
        d.mac_table.clear();
    }
    Ok(was)
}

pub fn set_sw_trunk_allowed(
    d: &mut Device,
    port: usize,
    vids: Vec<u16>,
) -> Result<(), SetErr> {
    switch_gate(d)?;
    if vids.is_empty() || vids.iter().any(|v| !(1..=4094).contains(v)) {
        return Err(SetErr::BadValue);
    }
    let Some(p) = d.ports.get_mut(port) else {
        return Err(SetErr::KindGate);
    };
    p.allowed = Some(vids);
    Ok(())
}

pub fn reset_sw_trunk_allowed(d: &mut Device, port: usize) -> Result<bool, SetErr> {
    switch_gate(d)?;
    let Some(p) = d.ports.get_mut(port) else {
        return Ok(false);
    };
    Ok(p.allowed.take().is_some())
}

pub fn add_route(
    d: &mut Device,
    net: [u8; 4],
    mask: [u8; 4],
    nh: [u8; 4],
    ad: Option<u8>,
    tick: u64,
) -> Result<(), SetErr> {
    modeful(d)?;
    if !contig_mask(mask) {
        return Err(SetErr::BadMask);
    }
    let net = net_of(net, mask);
    let out_port = d
        .ports
        .iter()
        .enumerate()
        .find_map(|(i, p)| {
            p.ip.and_then(|(a, m)| (net_of(nh, m) == net_of(a, m)).then_some(i))
        })
        .or_else(|| d.lookup(nh).map(|r| r.out_port))
        .unwrap_or(0);
    let ad = ad.unwrap_or(1);
    match d.routes.iter_mut().find(|r| {
        r.proto == RouteProto::Static
            && r.prefix == net
            && r.mask == mask
            && r.next_hop == nh
    }) {
        Some(r) => {
            r.ad = ad;
            r.learned_tick = tick;
        }
        None => d.routes.push(Route {
            prefix: net,
            mask,
            next_hop: nh,
            out_port,
            connected: false,
            proto: RouteProto::Static,
            ad,
            metric: 0,
            learned_tick: tick,
        }),
    }
    Ok(())
}

pub fn remove_route_by(
    d: &mut Device,
    net: [u8; 4],
    mask: [u8; 4],
    nh: Option<[u8; 4]>,
) -> Result<bool, SetErr> {
    modeful(d)?;
    let net = net_of(net, mask);
    let before = d.routes.len();
    d.routes.retain(|r| {
        !(r.proto == RouteProto::Static
            && r.prefix == net
            && r.mask == mask
            && nh.map(|n| r.next_hop == n).unwrap_or(true))
    });
    Ok(d.routes.len() != before)
}

fn vlan_name_default(id: u16) -> String {
    format!("VLAN{id:04}")
}

pub fn enter_vlan(d: &mut Device, id: u16) -> Result<(), SetErr> {
    switch_gate(d)?;
    d.vlans.entry(id).or_insert_with(|| Vlan {
        name: vlan_name_default(id),
        state: VlanState::Active,
    });
    Ok(())
}

pub fn remove_vlan(d: &mut Device, id: u16) -> Result<bool, SetErr> {
    switch_gate(d)?;
    if RESERVED_VLANS.contains(&id) {
        return Err(SetErr::Reserved);
    }
    Ok(d.vlans.remove(&id).is_some())
}

pub fn set_vlan_name(d: &mut Device, id: u16, name: &str) -> Result<(), SetErr> {
    switch_gate(d)?;
    text_clean(name)?;
    let v = d.vlans.entry(id).or_insert_with(|| Vlan {
        name: vlan_name_default(id),
        state: VlanState::Active,
    });
    v.name = name.to_string();
    Ok(())
}

pub fn reset_vlan_name(d: &mut Device, id: u16) -> Result<bool, SetErr> {
    switch_gate(d)?;
    let Some(v) = d.vlans.get_mut(&id) else {
        return Ok(false);
    };
    v.name = vlan_name_default(id);
    Ok(true)
}

pub fn set_vlan_state(d: &mut Device, id: u16, state: VlanState) -> Result<(), SetErr> {
    switch_gate(d)?;
    let v = d.vlans.entry(id).or_insert_with(|| Vlan {
        name: vlan_name_default(id),
        state: VlanState::Active,
    });
    v.state = state;
    Ok(())
}

pub fn acl_ace_add(d: &mut Device, num: u16, ace: Ace) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    if !d.acls.iter().any(|a| a.num == num) {
        d.acls.push(Acl {
            num,
            entries: Vec::new(),
        });
    }
    let list = d.acls.iter_mut().find(|a| a.num == num).unwrap();
    if !list.entries.iter().any(|e| e == &ace) {
        list.entries.push(ace);
    }
    Ok(())
}

pub fn acl_remove_list(d: &mut Device, num: u16) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    let before = d.acls.len();
    d.acls.retain(|a| a.num != num);
    Ok(d.acls.len() != before)
}

pub fn acl_ace_remove(d: &mut Device, num: u16, ace: &Ace) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    let Some(list) = d.acls.iter_mut().find(|a| a.num == num) else {
        return Ok(false);
    };
    let before = list.entries.len();
    list.entries.retain(|e| e != ace);
    Ok(list.entries.len() != before)
}

/// `dir` true = in. One binding per (port, dir) — trivially idempotent.
pub fn set_access_group(
    d: &mut Device,
    port: usize,
    num: u16,
    dir_in: bool,
) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    let Some(p) = d.ports.get_mut(port) else {
        return Err(SetErr::KindGate);
    };
    if dir_in {
        p.in_acl = Some(num);
    } else {
        p.out_acl = Some(num);
    }
    Ok(())
}

/// The no-form clears only a binding that actually points at `num`;
/// the list itself may dangle — preserved by design.
pub fn clear_access_group(
    d: &mut Device,
    port: usize,
    num: u16,
    dir_in: bool,
) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    let Some(p) = d.ports.get_mut(port) else {
        return Ok(false);
    };
    let slot = if dir_in { &mut p.in_acl } else { &mut p.out_acl };
    if *slot == Some(num) {
        *slot = None;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn ospf_enter(d: &mut Device, pid: u16) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    if d.ospf.on && d.ospf.pid != pid {
        return Err(SetErr::Conflict);
    }
    if !d.ospf.on {
        d.ospf.on = true;
        d.ospf.pid = pid;
        d.ospf.router_id = crate::ospf::derive_router_id(d);
    }
    Ok(())
}

pub fn ospf_remove(d: &mut Device, pid: u16) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    if d.ospf.on && d.ospf.pid == pid {
        d.ospf = crate::ospf::Ospf::default();
        d.routes.retain(|r| r.proto != RouteProto::Ospf);
        return Ok(true);
    }
    Ok(false)
}

/// Key (net, wild) — a repeat add replaces the area in place.
pub fn ospf_net_add(
    d: &mut Device,
    net: [u8; 4],
    wild: [u8; 4],
    area: u32,
) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    match d
        .ospf
        .nets
        .iter_mut()
        .find(|n| n.prefix == net && n.wild == wild)
    {
        Some(n) => n.area = area,
        None => d.ospf.nets.push(crate::ospf::OspfNet {
            prefix: net,
            wild,
            area,
        }),
    }
    Ok(())
}

/// Removing the last network covering a neighbor's port drives it Down.
pub fn ospf_net_remove(
    d: &mut Device,
    net: [u8; 4],
    wild: [u8; 4],
    area: u32,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    let before = d.ospf.nets.len();
    d.ospf
        .nets
        .retain(|n| !(n.prefix == net && n.wild == wild && n.area == area));
    let removed = d.ospf.nets.len() != before;
    if removed {
        crate::ospf::nbrs_off_networks(d, tick, events);
    }
    Ok(removed)
}

pub fn ospf_router_id_set(d: &mut Device, rid: [u8; 4]) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    d.ospf.router_id = crate::ospf::OspfRouterId {
        value: rid,
        explicit: true,
    };
    Ok(())
}

pub fn ospf_router_id_clear(d: &mut Device) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    d.ospf.router_id = crate::ospf::derive_router_id(d);
    Ok(true)
}

/// Key inside_local — a repeat add replaces inside_global.
pub fn nat_inside_static_add(
    d: &mut Device,
    il: [u8; 4],
    ig: [u8; 4],
) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    match d
        .static_nat
        .iter_mut()
        .find(|s| s.proto == 0 && s.inside_ip == il)
    {
        Some(s) => s.inside_global = Some(ig),
        None => d.static_nat.push(crate::services::StaticNat {
            proto: 0,
            inside_ip: il,
            inside_global: Some(ig),
            inside_port: 0,
            outside_port: 0,
        }),
    }
    Ok(())
}

pub fn nat_inside_static_remove(d: &mut Device, il: [u8; 4]) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    let before = d.static_nat.len();
    d.static_nat
        .retain(|s| !(s.proto == 0 && s.inside_ip == il));
    Ok(d.static_nat.len() != before)
}

pub fn set_nat_role(d: &mut Device, port: usize, role: NatRole) -> Result<(), SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    if role == NatRole::Outside {
        for (i, p) in d.ports.iter_mut().enumerate() {
            if i != port && p.nat == NatRole::Outside {
                p.nat = NatRole::Off;
            }
        }
    }
    let Some(p) = d.ports.get_mut(port) else {
        return Err(SetErr::KindGate);
    };
    p.nat = role;
    Ok(())
}

pub fn clear_nat_role(d: &mut Device, port: usize, role: NatRole) -> Result<bool, SetErr> {
    match d.kind {
        DeviceKind::Router => {}
        _ => return Err(SetErr::KindGate),
    }
    let Some(p) = d.ports.get_mut(port) else {
        return Ok(false);
    };
    if p.nat == role {
        p.nat = NatRole::Off;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn set_dhcp_service(d: &mut Device, on: bool) -> Result<(), SetErr> {
    modeful(d)?;
    d.svc_dhcp = on;
    Ok(())
}

/// Key lo — a repeat add replaces hi.
pub fn dhcp_excluded_add(
    d: &mut Device,
    lo: [u8; 4],
    hi: Option<[u8; 4]>,
) -> Result<(), SetErr> {
    modeful(d)?;
    match d.dhcp_excluded.iter_mut().find(|e| e.lo == lo) {
        Some(e) => e.hi = hi,
        None => d.dhcp_excluded.push(crate::device::DhcpExcluded { lo, hi }),
    }
    Ok(())
}

pub fn dhcp_excluded_remove(d: &mut Device, lo: [u8; 4]) -> Result<bool, SetErr> {
    modeful(d)?;
    let before = d.dhcp_excluded.len();
    d.dhcp_excluded.retain(|e| e.lo != lo);
    Ok(d.dhcp_excluded.len() != before)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleKind {
    DomainLookup,
    IpRouting,
    ProxyArp,
    DirectedBroadcast,
}

impl ToggleKind {
    pub fn default_for(self, kind: DeviceKind) -> bool {
        match self {
            ToggleKind::DomainLookup => true,
            ToggleKind::IpRouting => kind == DeviceKind::Router,
            ToggleKind::ProxyArp => kind != DeviceKind::Host,
            ToggleKind::DirectedBroadcast => false,
        }
    }
}

pub fn set_toggle(d: &mut Device, k: ToggleKind, on: bool) -> Result<(), SetErr> {
    match (d.kind, k) {
        (DeviceKind::Router | DeviceKind::Switch, ToggleKind::DomainLookup) => {
            d.toggles.domain_lookup = on
        }
        (DeviceKind::Router | DeviceKind::Switch, ToggleKind::IpRouting) => {
            d.toggles.ip_routing = on
        }
        (DeviceKind::Router, ToggleKind::ProxyArp) => d.toggles.proxy_arp = on,
        (DeviceKind::Router, ToggleKind::DirectedBroadcast) => {
            d.toggles.directed_broadcast = on
        }
        _ => return Err(SetErr::KindGate),
    }
    Ok(())
}
