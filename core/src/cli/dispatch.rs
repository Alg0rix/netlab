//! Dispatch — one Act arm per spec item 15. SetErr → `%` rejection at
//! ONE mapping site. `clear` never sets mutated — the tick-pump refresh
//! channel carries the visible change.


use super::types::{
    AceSpec, Act, AddrSpec, ClearKind, Intent, Mutation, NoTarget, Op, SwSpec,
};
use super::{modes, nvgen, render, Cli};
use crate::config::{self, SetErr};
use crate::device::{Ace, Device, RouteProto};
use crate::events::SimEvent;

/// `%`-style rejection line — the single SetErr mapping site.
pub fn set_err_op(e: SetErr) -> Op {
    Op::err(match e {
        SetErr::KindGate => "% Feature not supported on this device",
        SetErr::BadMask => "% Bad mask",
        SetErr::Overlaps => "% overlaps",
        SetErr::Inconsistent => "%Inconsistent",
        SetErr::Conflict => "% Conflicts with existing state",
        SetErr::Reserved => "% Reserved VLAN — cannot modify",
        SetErr::BadChars => "% Illegal characters in input",
        SetErr::BadValue => "% Invalid value",
    })
}

fn map_err(r: Result<impl Sized, SetErr>, out: &mut Vec<Op>) -> bool {
    match r {
        Ok(_) => true,
        Err(e) => {
            out.push(set_err_op(e));
            false
        }
    }
}

fn ace_of(spec: &AceSpec) -> Ace {
    let (src, src_wild) = match spec.src {
        AddrSpec::Any => ([0; 4], [255; 4]),
        AddrSpec::Host(h) => (h, [0; 4]),
        AddrSpec::Net { ip, wild } => (ip, wild),
    };
    let (dst, dst_wild) = match spec.dst {
        AddrSpec::Any => ([0; 4], [255; 4]),
        AddrSpec::Host(h) => (h, [0; 4]),
        AddrSpec::Net { ip, wild } => (ip, wild),
    };
    Ace {
        permit: spec.permit,
        proto: spec.proto,
        src,
        src_wild,
        dst,
        dst_wild,
        sport: None,
        dport: None,
    }
}

/// Execute an Act. Returns (ops, mutated).
pub fn dispatch(
    cli: &mut Cli,
    d: &mut Device,
    act: Act,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> (Vec<Op>, bool) {
    let mut out = Vec::new();
    let mutated = run(cli, d, act, tick, events, &mut out);
    (out, mutated)
}

fn run(
    cli: &mut Cli,
    d: &mut Device,
    act: Act,
    tick: u64,
    events: &mut Vec<SimEvent>,
    out: &mut Vec<Op>,
) -> bool {
    match act {
        Act::Mutate(m) => mutate(d, m, tick, events, out),
        Act::No(t) | Act::Default(t) => negate(d, t, tick, events, out),
        Act::Session(s) => {
            match s {
                crate::cli::types::SessionAct::HistorySize(n) => {
                    cli.history_max = n as usize;
                    while cli.history.len() > cli.history_max {
                        cli.history.pop_front();
                    }
                }
            }
            false
        }
        Act::HostExtra(k) => {
            out.push(Op::err(format!(
                "% {}: not supported in this lab",
                k.tool()
            )));
            false
        }
        Act::Show(k) => {
            out.extend(render::show(cli, d, k, tick));
            false
        }
        Act::Clear(k) => {
            clear(d, k, tick);
            false
        }
        Act::Mode(op) => match modes::transition(cli, d, op, tick) {
            Ok(ops) => {
                out.extend(ops);
                false
            }
            Err(e) => {
                out.push(set_err_op(e));
                false
            }
        },
        Act::Do(inner) => run(cli, d, *inner, tick, events, out),
        Act::Begin(p) => {
            cli.pending = Some(p);
            false
        }
        Act::Intent(i) => {
            intent(cli, d, i, out);
            false
        }
        Act::Translate { word } => {
            out.extend(translate(d, &word));
            false
        }
    }
}

fn intent(cli: &mut Cli, d: &Device, i: Intent, out: &mut Vec<Op>) {
    match i {
        Intent::NvramWrite { text } => {
            let text = if text.is_empty() { nvgen::nvgen(d) } else { text };
            cli.startup_text = Some(text.clone());
            out.push(Op::text("Building configuration..."));
            out.push(Op::ok("[OK]"));
            out.push(Op::Intent(Intent::NvramWrite { text }));
        }
        Intent::NvramErase => {
            cli.startup_text = None;
            out.push(Op::Intent(Intent::NvramErase));
        }
        other => out.push(Op::Intent(other)),
    }
}

/// The translate render — domain_lookup gate lives here.
fn translate(d: &Device, word: &str) -> Vec<Op> {
    if d.toggles.domain_lookup {
        vec![
            Op::text(format!(
                "Translating \"{word}\" ...domain server (255.255.255.255)"
            )),
            Op::err("% Unknown command or computer name, or unable to find computer address"),
        ]
    } else {
        vec![Op::err(
            "% Unknown command or computer name, or unable to find computer address",
        )]
    }
}

fn mutate(
    d: &mut Device,
    m: Mutation,
    tick: u64,
    events: &mut Vec<SimEvent>,
    out: &mut Vec<Op>,
) -> bool {
    match m {
        Mutation::Hostname(name) => {
            if !map_err(config::set_hostname(d, &name), out) {
                return false;
            }
            // paired Intent::Rename in the same out
            out.push(Op::Intent(Intent::Rename { name }));
            true
        }
        Mutation::Pcname(name) => map_err(config::set_pcname(d, &name), out),
        Mutation::SetAdmin { port, up } => {
            map_err(config::set_admin(d, port, up, tick, events), out)
        }
        Mutation::SetIp { port, ip, mask } => {
            map_err(config::set_ip(d, port, ip, mask, tick), out)
        }
        Mutation::HostIp { ip, mask, gw } => {
            map_err(config::set_host_ip(d, ip, mask, gw, tick), out)
        }
        Mutation::HostDhcp => map_err(config::host_dhcp(d, tick, events), out),
        Mutation::IfDescription { port, text } => {
            map_err(config::set_description(d, port, &text), out)
        }
        Mutation::Switchport { port, spec } => map_err(
            match spec {
                SwSpec::Mode(trunk) => config::set_sw_mode(d, port, trunk),
                SwSpec::AccessVlan(vid) => config::set_sw_access_vlan(d, port, vid),
                SwSpec::TrunkAllowed(vids) => {
                    config::set_sw_trunk_allowed(d, port, vids)
                }
            },
            out,
        ),
        Mutation::AddRoute { net, mask, nh, ad } => {
            map_err(config::add_route(d, net, mask, nh, ad, tick), out)
        }
        Mutation::Vlan { id, name, state } => {
            if let Err(e) = config::enter_vlan(d, id) {
                out.push(set_err_op(e));
                return false;
            }
            if let Some(n) = name {
                if let Err(e) = config::set_vlan_name(d, id, &n) {
                    out.push(set_err_op(e));
                    return false;
                }
            }
            if let Some(st) = state {
                if let Err(e) = config::set_vlan_state(d, id, st) {
                    out.push(set_err_op(e));
                    return false;
                }
            }
            true
        }
        Mutation::AclAce { list, ace } => {
            map_err(config::acl_ace_add(d, list, ace_of(&ace)), out)
        }
        Mutation::IpAccessGroup { port, list, dir_in } => {
            map_err(config::set_access_group(d, port, list, dir_in), out)
        }
        Mutation::OspfNet { net, wild, area, .. } => {
            map_err(config::ospf_net_add(d, net, wild, area), out)
        }
        Mutation::OspfRouterId { rid, .. } => {
            map_err(config::ospf_router_id_set(d, rid), out)
        }
        Mutation::NatInsideStatic { inside_local, inside_global } => map_err(
            config::nat_inside_static_add(d, inside_local, inside_global),
            out,
        ),
        Mutation::NatIfRole { port, side } => {
            map_err(config::set_nat_role(d, port, side), out)
        }
        Mutation::DhcpService(on) => map_err(config::set_dhcp_service(d, on), out),
        Mutation::DhcpExcluded { lo, hi } => {
            map_err(config::dhcp_excluded_add(d, lo, hi), out)
        }
        Mutation::Toggle(k, on) => map_err(config::set_toggle(d, k, on), out),
    }
}

/// No/Default arms — tuple-match removal; a miss is a silent
/// non-mutating Ok. The bool the setters return IS the mutated signal.
fn negate(
    d: &mut Device,
    t: NoTarget,
    tick: u64,
    events: &mut Vec<SimEvent>,
    out: &mut Vec<Op>,
) -> bool {
    let r: Result<bool, SetErr> = match t {
        NoTarget::Hostname => config::reset_hostname(d).map(|_| true),
        NoTarget::IfDescription { port } => config::clear_description(d, port),
        NoTarget::IfIp { port } => config::clear_ip(d, port, tick),
        NoTarget::Route { net, mask, nh } => config::remove_route_by(d, net, mask, nh),
        NoTarget::Vlan { id } => config::remove_vlan(d, id),
        NoTarget::VlanName { id } => config::reset_vlan_name(d, id),
        NoTarget::Acl { list } => config::acl_remove_list(d, list),
        NoTarget::AclAce { list, ace } => config::acl_ace_remove(d, list, &ace_of(&ace)),
        NoTarget::IpAccessGroup { port, list, dir_in } => {
            config::clear_access_group(d, port, list, dir_in)
        }
        NoTarget::OspfNet { net, wild, area, .. } => {
            config::ospf_net_remove(d, net, wild, area, tick, events)
        }
        NoTarget::OspfRouterId { .. } => config::ospf_router_id_clear(d),
        NoTarget::OspfProcess { pid } => config::ospf_remove(d, pid),
        NoTarget::NatInsideStatic { inside_local } => {
            config::nat_inside_static_remove(d, inside_local)
        }
        NoTarget::NatIfSide { port, side } => config::clear_nat_role(d, port, side),
        NoTarget::DhcpExcluded { lo } => config::dhcp_excluded_remove(d, lo),
        NoTarget::SwAccessVlan { port } => config::reset_sw_access_vlan(d, port),
        NoTarget::SwTrunkAllowed { port } => config::reset_sw_trunk_allowed(d, port),
        NoTarget::SwMode { port } => config::reset_sw_mode(d, port),
        NoTarget::VlanState { id } => {
            config::set_vlan_state(d, id, crate::device::VlanState::Active).map(|_| true)
        }
        NoTarget::IfAdmin { port } => config::set_admin(d, port, true, tick, events).map(|_| true),
        NoTarget::ServiceDhcp => config::set_dhcp_service(d, false).map(|_| true),
        NoTarget::Toggle(k) => config::set_toggle(d, k, false).map(|_| true),
    };
    match r {
        Ok(changed) => changed,
        Err(e) => {
            out.push(set_err_op(e));
            false
        }
    }
}

fn clear(d: &mut Device, k: ClearKind, tick: u64) {
    match k {
        ClearKind::Arp => d.arp_cache.clear(),
        ClearKind::MacTable => d.mac_table.clear(),
        ClearKind::NatTranslation => d.nat.clear(),
        ClearKind::OspfProcess => d.ospf.reset(),
        ClearKind::IpRouteAll => {
            d.routes.retain(|r| r.proto == RouteProto::Connected);
            d.sync_connected(tick);
        }
        ClearKind::Counters { port } => {
            for (i, p) in d.ports.iter_mut().enumerate() {
                if port.map(|want| want == i).unwrap_or(true) {
                    p.stats = Default::default();
                    p.stats.cleared_tick = tick;
                }
            }
        }
    }
}
