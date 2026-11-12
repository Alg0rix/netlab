//! bind() — builds the Act at match time from positional argv, ModeCtx
//! slots, and the neg/do bits the matcher recorded. *Ask tags run their
//! named dialog-builder and wrap the result as Act::Begin.

use std::net::Ipv4Addr;

use super::pending;
use super::types::{
    AceSpec, Act, ActTag, AddrSpec, Arg, ClearKind, HostSvc, Intent, ModeCtx,
    ModeOp, Mutation, NegKind, NoTarget, SwSpec,
};
use crate::device::{Device, DeviceKind, NatRole, VlanState};

fn ip(a: &Arg) -> [u8; 4] {
    match a {
        Arg::Ip(v) | Arg::Mask(v) => v.octets(),
        _ => [0; 4],
    }
}

fn num(a: &Arg) -> u32 {
    match a {
        Arg::Num(v) => *v,
        _ => 0,
    }
}

fn port(a: &Arg) -> usize {
    match a {
        Arg::Port(p) => *p,
        _ => 0,
    }
}

fn text(a: &Arg) -> String {
    match a {
        Arg::Str(s) | Arg::Name(s) => s.clone(),
        _ => String::new(),
    }
}

/// The four {a|b} discriminating-Lit argv: action, proto, dir, state ride
/// as Arg::Str(canonical lit).
fn str_at(argv: &[Arg], i: usize) -> &str {
    match argv.get(i) {
        Some(Arg::Str(s)) => s.as_str(),
        _ => "",
    }
}

fn proto_of(s: &str) -> u8 {
    match s {
        "icmp" => 1,
        "tcp" => 6,
        "udp" => 17,
        _ => 0,
    }
}

/// An <addr> seat: Str("any") | Str("host"),Ip | Ip,Mask(wild).
fn addr_at(argv: &[Arg], i: usize) -> AddrSpec {
    match argv.get(i) {
        Some(Arg::Str(s)) if s == "any" => AddrSpec::Any,
        Some(Arg::Str(s)) if s == "host" => AddrSpec::Host(ip(argv.get(i + 1).unwrap())),
        Some(Arg::Ip(a)) => {
            let wild = ip(argv.get(i + 1).unwrap());
            AddrSpec::Net { ip: a.octets(), wild }
        }
        _ => AddrSpec::Any,
    }
}

/// addr seats occupy 1 or 2 argv slots.
fn addr_len(argv: &[Arg], i: usize) -> usize {
    match argv.get(i) {
        Some(Arg::Str(_)) => 1,
        Some(Arg::Ip(_)) => 2,
        _ => 0,
    }
}

fn ace_at(argv: &[Arg]) -> AceSpec {
    // Num(list), Str(action), Str(proto), <src>, <dst>
    let src_i = 3;
    let dst_i = src_i + addr_len(argv, src_i);
    AceSpec {
        permit: str_at(argv, 1) == "permit",
        proto: proto_of(str_at(argv, 2)),
        src: addr_at(argv, src_i),
        dst: addr_at(argv, dst_i),
    }
}

/// Per-tag bind. Returns Err(tok_index) on a malformed argv shape —
/// a defect (die class), never silently ignored.
pub fn bind(
    tag: ActTag,
    argv: &[Arg],
    ctx: &ModeCtx,
    neg: Option<NegKind>,
    dw: bool,
    d: &Device,
) -> Result<Act, usize> {
    let act = match tag {
        ActTag::Enable => Act::Mode(ModeOp::Enable),
        ActTag::Disable => Act::Mode(ModeOp::Disable),
        ActTag::Configure => Act::Mode(ModeOp::Configure),
        ActTag::End => Act::Mode(ModeOp::End),
        ActTag::Exit | ActTag::Logout => Act::Mode(ModeOp::Exit),
        ActTag::HistorySize => Act::Session(crate::cli::types::SessionAct::HistorySize(
            num(&argv[0]) as u16,
        )),
        ActTag::EnterIf => Act::Mode(ModeOp::EnterIf(port(&argv[0]))),
        ActTag::EnterVlan => Act::Mode(ModeOp::EnterVlan(num(&argv[0]) as u16)),
        ActTag::EnterRouterOspf => {
            Act::Mode(ModeOp::EnterRouterOspf(num(&argv[0]) as u16))
        }

        ActTag::Hostname => Act::Mutate(Mutation::Hostname(text(&argv[0]))),
        ActTag::Pcname => Act::Mutate(Mutation::Pcname(text(&argv[0]))),
        ActTag::IfAdmin => {
            let port = ctx.if_port.ok_or(0usize)?;
            let up = match neg {
                Some(NegKind::No) => true,
                Some(NegKind::Default) => d.kind != DeviceKind::Router,
                None => false,
            };
            Act::Mutate(Mutation::SetAdmin { port, up })
        }
        ActTag::SetIp => Act::Mutate(Mutation::SetIp {
            port: ctx.if_port.ok_or(0usize)?,
            ip: ip(&argv[0]),
            mask: ip(&argv[1]),
        }),
        ActTag::HostIp => {
            // ip <ip>/<plen> [<gw>] | ip <ip> <mask> [<gw>]
            let (addr, mask, next) = match &argv[0] {
                Arg::Prefix(a, plen) => {
                    let shift = 32u32.saturating_sub(*plen as u32);
                    let m = if shift >= 32 { 0 } else { u32::MAX << shift };
                    (a.octets(), m.to_be_bytes(), 1usize)
                }
                Arg::Ip(a) => (a.octets(), ip(&argv[1]), 2usize),
                _ => ([0; 4], [0; 4], 0usize),
            };
            let gw = argv.get(next).map(ip);
            Act::Mutate(Mutation::HostIp { ip: addr, mask, gw })
        }
        ActTag::HostDhcp => Act::Mutate(Mutation::HostDhcp),
        ActTag::IfDescription => Act::Mutate(Mutation::IfDescription {
            port: ctx.if_port.ok_or(0usize)?,
            text: text(&argv[0]),
        }),
        ActTag::SwModeAccess | ActTag::SwModeTrunk => Act::Mutate(Mutation::Switchport {
            port: ctx.if_port.ok_or(0usize)?,
            spec: SwSpec::Mode(tag == ActTag::SwModeTrunk),
        }),
        ActTag::SwAccessVlan => Act::Mutate(Mutation::Switchport {
            port: ctx.if_port.ok_or(0usize)?,
            spec: SwSpec::AccessVlan(num(&argv[0]) as u16),
        }),
        ActTag::SwTrunkAllowed => Act::Mutate(Mutation::Switchport {
            port: ctx.if_port.ok_or(0usize)?,
            spec: SwSpec::TrunkAllowed(match &argv[0] {
                Arg::Vlans(v) => v.clone(),
                _ => Vec::new(),
            }),
        }),
        ActTag::AddRoute => Act::Mutate(Mutation::AddRoute {
            net: ip(&argv[0]),
            mask: ip(&argv[1]),
            nh: ip(&argv[2]),
            ad: argv.get(3).map(|a| num(a) as u8),
        }),
        ActTag::VlanName => Act::Mutate(Mutation::Vlan {
            id: ctx.vlan_id.ok_or(0usize)?,
            name: Some(text(&argv[0])),
            state: None,
        }),
        ActTag::VlanStateSet => {
            let state = match neg {
                Some(NegKind::No) | Some(NegKind::Default) => VlanState::Active,
                None => {
                    if str_at(argv, 0) == "suspend" {
                        VlanState::Suspend
                    } else {
                        VlanState::Active
                    }
                }
            };
            Act::Mutate(Mutation::Vlan {
                id: ctx.vlan_id.ok_or(0usize)?,
                name: None,
                state: Some(state),
            })
        }
        ActTag::AclAceAdd => Act::Mutate(Mutation::AclAce {
            list: num(&argv[0]) as u16,
            ace: ace_at(argv),
        }),
        ActTag::IpAccessGroup => Act::Mutate(Mutation::IpAccessGroup {
            port: ctx.if_port.ok_or(0usize)?,
            list: num(&argv[0]) as u16,
            dir_in: str_at(argv, 1) == "in",
        }),
        ActTag::OspfNetAdd => Act::Mutate(Mutation::OspfNet {
            pid: ctx.ospf_pid.ok_or(0usize)?,
            net: ip(&argv[0]),
            wild: ip(&argv[1]),
            area: num(&argv[2]),
        }),
        ActTag::OspfRouterIdSet => Act::Mutate(Mutation::OspfRouterId {
            pid: ctx.ospf_pid.ok_or(0usize)?,
            rid: ip(&argv[0]),
        }),
        ActTag::NatInsideStaticAdd => Act::Mutate(Mutation::NatInsideStatic {
            inside_local: ip(&argv[0]),
            inside_global: ip(&argv[1]),
        }),
        ActTag::NatIfInside | ActTag::NatIfOutside => {
            Act::Mutate(Mutation::NatIfRole {
                port: ctx.if_port.ok_or(0usize)?,
                side: if tag == ActTag::NatIfInside {
                    NatRole::Inside
                } else {
                    NatRole::Outside
                },
            })
        }
        ActTag::ServiceDhcp => {
            let on = match neg {
                Some(NegKind::No) => false,
                Some(NegKind::Default) => true,
                None => true,
            };
            Act::Mutate(Mutation::DhcpService(on))
        }
        ActTag::DhcpExcludedAdd => Act::Mutate(Mutation::DhcpExcluded {
            lo: ip(&argv[0]),
            hi: argv.get(1).map(ip),
        }),
        ActTag::Toggle(k) => {
            let on = match neg {
                Some(NegKind::No) => false,
                Some(NegKind::Default) => k.default_for(d.kind),
                None => true,
            };
            Act::Mutate(Mutation::Toggle(k, on))
        }

        // negation slice — the neg bit decides No vs Default at negt()
        ActTag::NegHostname => negt(NoTarget::Hostname, neg),
        ActTag::NegIfDescription => negt(NoTarget::IfDescription {
            port: ctx.if_port.ok_or(0usize)?,
        }, neg),
        ActTag::NegIfIp => negt(NoTarget::IfIp {
            port: ctx.if_port.ok_or(0usize)?,
        }, neg),
        ActTag::NegRoute => negt(NoTarget::Route {
            net: ip(&argv[0]),
            mask: ip(&argv[1]),
            nh: argv.get(2).map(ip),
        }, neg),
        ActTag::NegVlan => negt(NoTarget::Vlan {
            id: num(&argv[0]) as u16,
        }, neg),
        ActTag::NegVlanName => negt(NoTarget::VlanName {
            id: ctx.vlan_id.ok_or(0usize)?,
        }, neg),
        ActTag::NegAcl => negt(NoTarget::Acl {
            list: num(&argv[0]) as u16,
        }, neg),
        ActTag::NegAclAce => negt(NoTarget::AclAce {
            list: num(&argv[0]) as u16,
            ace: ace_at(argv),
        }, neg),
        ActTag::NegIpAccessGroup => negt(NoTarget::IpAccessGroup {
            port: ctx.if_port.ok_or(0usize)?,
            list: num(&argv[0]) as u16,
            dir_in: str_at(argv, 1) == "in",
        }, neg),
        ActTag::NegOspfNet => negt(NoTarget::OspfNet {
            pid: ctx.ospf_pid.ok_or(0usize)?,
            net: ip(&argv[0]),
            wild: ip(&argv[1]),
            area: num(&argv[2]),
        }, neg),
        ActTag::NegOspfRouterId => negt(NoTarget::OspfRouterId {
            pid: ctx.ospf_pid.ok_or(0usize)?,
        }, neg),
        ActTag::NegOspfProcess => negt(NoTarget::OspfProcess {
            pid: num(&argv[0]) as u16,
        }, neg),
        ActTag::NegNatInsideStatic => negt(NoTarget::NatInsideStatic {
            inside_local: ip(&argv[0]),
        }, neg),
        ActTag::NegNatIfSide => negt(NoTarget::NatIfSide {
            port: ctx.if_port.ok_or(0usize)?,
            side: if str_at(argv, 0) == "inside" {
                NatRole::Inside
            } else {
                NatRole::Outside
            },
        }, neg),
        ActTag::NegDhcpExcluded => negt(NoTarget::DhcpExcluded {
            lo: ip(&argv[0]),
        }, neg),
        ActTag::NegSwAccessVlan => negt(NoTarget::SwAccessVlan {
            port: ctx.if_port.ok_or(0usize)?,
        }, neg),
        ActTag::NegSwTrunkAllowed => negt(NoTarget::SwTrunkAllowed {
            port: ctx.if_port.ok_or(0usize)?,
        }, neg),
        ActTag::NegSwMode => negt(NoTarget::SwMode {
            port: ctx.if_port.ok_or(0usize)?,
        }, neg),

        ActTag::Show(k) => Act::Show(k),
        ActTag::Clear(k) => {
            let k = match (k, argv.first()) {
                (ClearKind::Counters { .. }, Some(Arg::Port(p))) => {
                    ClearKind::Counters { port: Some(*p) }
                }
                (k, _) => k,
            };
            Act::Clear(k)
        }
        ActTag::HostExtra(k) => Act::HostExtra(k),
        ActTag::HostDns => Act::Mutate(Mutation::HostDns {
            ip: argv.first().map(ip),
        }),
        ActTag::HostNslookup => Act::Mutate(Mutation::HostNslookup {
            name: text(&argv[0]),
        }),
        ActTag::HostCurl => {
            // `curl <ip> [port]` — `ip:port` in one token is also legal.
            let target = text(&argv[0]);
            let (h, p) = match target.split_once(':') {
                Some((h, p)) => (h.to_string(), p.parse().unwrap_or(80)),
                None => (target, argv.get(1).map(num).map(|n| n as u16).unwrap_or(80)),
            };
            let addr: Ipv4Addr = h.parse().map_err(|_| 0usize)?;
            Act::Mutate(Mutation::HostCurl {
                ip: addr.octets(),
                port: p,
            })
        }
        ActTag::HostNc => {
            let udp = matches!(argv.first(), Some(Arg::Str(s)) if s == "-u");
            let i = usize::from(udp);
            Act::Mutate(Mutation::HostNc {
                udp,
                ip: ip(&argv[i]),
                port: num(&argv[i + 1]) as u16,
                payload: argv.get(i + 2).map(text).unwrap_or_default(),
            })
        }
        ActTag::HostServerSvc => {
            let svc = match str_at(argv, 0) {
                "dhcp" => HostSvc::Dhcp,
                "dns" => HostSvc::Dns,
                _ => HostSvc::Http,
            };
            Act::Mutate(Mutation::HostService {
                svc,
                on: str_at(argv, 1) == "on",
            })
        }
        ActTag::HostDnsRecord => Act::Mutate(Mutation::HostDnsRecord {
            name: text(&argv[1]),
            ip: ip(&argv[2]),
        }),
        ActTag::Ping => {
            Act::Intent(Intent::Ping { target: Ipv4Addr::from(ip(&argv[0])) })
        }
        ActTag::PingHiddenC => {
            Act::Intent(Intent::Ping { target: Ipv4Addr::from(ip(&argv[1])) })
        }
        ActTag::Traceroute => {
            Act::Intent(Intent::Trace { target: Ipv4Addr::from(ip(&argv[0])) })
        }
        ActTag::WriteMem => Act::Intent(Intent::NvramWrite { text: String::new() }),
        ActTag::CopyRunStartAsk => Act::Begin(pending::copy_run_start()),
        ActTag::WriteEraseAsk => Act::Begin(pending::write_erase()),
        ActTag::EraseStartupAsk => Act::Begin(pending::write_erase()),
        ActTag::ReloadAsk => Act::Begin(pending::reload_chain()),
    };

    Ok(if dw { Act::Do(Box::new(act)) } else { act })
}

/// Neg* seats wrap their NoTarget under Act::No / Act::Default — the
/// distinct arms preserve provenance for ?/diagnostics.
fn negt(t: NoTarget, neg: Option<NegKind>) -> Act {
    match neg.unwrap_or(NegKind::No) {
        NegKind::No => Act::No(t),
        NegKind::Default => Act::Default(t),
    }
}
