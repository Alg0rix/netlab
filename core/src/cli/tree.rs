//! Grammar statics — plain `static` slices, const-constructible, zero
//! runtime construction. Shared subtrees are named statics referenced by
//! multiple roots. All payload data rides argv via bind().

use crate::config::ToggleKind;
use super::types::{ActTag, Edge, GNode, ShowKind, ClearKind, HostExtraKind, Tok, EMPTY_KIDS};

const fn g(
    tok: Tok,
    act: Option<ActTag>,
    kids: &'static [GNode],
    help: Option<&'static str>,
) -> GNode {
    GNode { tok, help, hidden: false, act, kids, edge: Edge::None, emits: false }
}

const fn gh(
    tok: Tok,
    act: Option<ActTag>,
    kids: &'static [GNode],
    help: Option<&'static str>,
) -> GNode {
    GNode { tok, help, hidden: true, act, kids, edge: Edge::None, emits: false }
}

const fn gd(
    lit: &'static str,
    act: Option<ActTag>,
    kids: &'static [GNode],
    help: Option<&'static str>,
) -> GNode {
    GNode {
        tok: Tok::Lit(lit),
        help,
        hidden: false,
        act,
        kids,
        edge: Edge::None,
        emits: true,
    }
}

const fn ge(
    lit: &'static str,
    edge: Edge,
    kids: &'static [GNode],
    help: Option<&'static str>,
) -> GNode {
    GNode {
        tok: Tok::Lit(lit),
        help,
        hidden: false,
        act: None,
        kids,
        edge,
        emits: false,
    }
}

const fn l(lit: &'static str, act: ActTag, help: Option<&'static str>) -> GNode {
    g(Tok::Lit(lit), Some(act), EMPTY_KIDS, help)
}

/// Shared nodes are `const` items — they inline into every referencing
/// static (slices can't embed another static by value).
const CLEAR_COUNTERS_NODE: GNode = GNode {
    tok: Tok::Lit("counters"),
    help: Some("interface counters"),
    hidden: false,
    act: Some(ActTag::Clear(ClearKind::Counters { port: None })),
    kids: &[GNode {
        tok: Tok::IfName,
        help: Some("<if>"),
        hidden: false,
        act: Some(ActTag::Clear(ClearKind::Counters { port: None })),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
};

const NO_ACCESS_LIST_NODE: GNode = GNode {
    tok: Tok::Lit("access-list"),
    help: Some("ACL"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Range { lo: 1, hi: 2699 },
        help: Some("<1-2699>"),
        hidden: false,
        act: Some(ActTag::NegAcl),
        kids: NO_ACE_ACTION,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
};

const IF_IP_AG_NODE: GNode = GNode {
    tok: Tok::Lit("access-group"),
    help: Some("ACL binding"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Range { lo: 1, hi: 2699 },
        help: Some("<1-2699>"),
        hidden: false,
        act: None,
        kids: IF_AG_DIR,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
};

const IF_NAT_NODE: GNode = GNode {
    tok: Tok::Lit("nat"),
    help: Some("NAT"),
    hidden: false,
    act: None,
    kids: IF_NAT_DIR,
    edge: Edge::None,
    emits: false,
};

const NO_IF_AG_NODE: GNode = GNode {
    tok: Tok::Lit("access-group"),
    help: Some("ACL binding"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Range { lo: 1, hi: 2699 },
        help: Some("<1-2699>"),
        hidden: false,
        act: None,
        kids: NO_IF_AG_DIR,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
};

const NO_IF_NAT_NODE: GNode = GNode {
    tok: Tok::Lit("nat"),
    help: Some("NAT"),
    hidden: false,
    act: None,
    kids: NO_IF_NAT_DIR,
    edge: Edge::None,
    emits: false,
};

static SHOW_IP_OSPF_KIDS: &[GNode] = &[
    l("neighbor", ActTag::Show(ShowKind::OspfNeighbor), Some("neighbors")),
    l("interface", ActTag::Show(ShowKind::OspfInterface), Some("interfaces")),
];

static SHOW_IP_DHCP: &[GNode] =
    &[l("binding", ActTag::Show(ShowKind::Dhcp), Some("bindings"))];

static SHOW_IP_INTERFACE: &[GNode] =
    &[l("brief", ActTag::Show(ShowKind::IpIntBrief), Some("brief"))];

static SHOW_IP_NAT: &[GNode] =
    &[l("translations", ActTag::Show(ShowKind::NatTranslations), Some("translations"))];

static SHOW_IP_KIDS: &[GNode] = &[
    l("access-lists", ActTag::Show(ShowKind::AccessLists), Some("ACLs")),
    GNode {
        tok: Tok::Lit("dhcp"),
        help: Some("DHCP"),
        hidden: false,
        act: None,
        kids: SHOW_IP_DHCP,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("interface"),
        help: Some("interfaces"),
        hidden: false,
        act: None,
        kids: SHOW_IP_INTERFACE,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("nat"),
        help: Some("NAT"),
        hidden: false,
        act: None,
        kids: SHOW_IP_NAT,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("ospf"),
        help: Some("OSPF"),
        hidden: false,
        act: Some(ActTag::Show(ShowKind::Ospf)),
        kids: SHOW_IP_OSPF_KIDS,
        edge: Edge::None,
        emits: false,
    },
    l("protocols", ActTag::Show(ShowKind::Protocols), Some("protocols")),
    l("route", ActTag::Show(ShowKind::IpRoute), Some("routing table")),
];

static SHOW_LOGGING: &[GNode] =
    &[l("buffered", ActTag::Show(ShowKind::LoggingBuffered), Some("buffered log"))];

static SHOW_MAC: &[GNode] =
    &[l("address-table", ActTag::Show(ShowKind::MacAddrTable), Some("MAC table"))];

/// router show bank — no mac/switchport/vlan heads.
static SHOW_KIDS_ROUTER: &[GNode] = &[
    l("arp", ActTag::Show(ShowKind::Arp), Some("ARP table")),
    l("history", ActTag::Show(ShowKind::History), Some("history")),
    l("interfaces", ActTag::Show(ShowKind::Interfaces), Some("interfaces")),
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: SHOW_IP_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("logging"),
        help: Some("logging"),
        hidden: false,
        act: None,
        kids: SHOW_LOGGING,
        edge: Edge::None,
        emits: false,
    },
    l("running-config", ActTag::Show(ShowKind::RunningConfig), Some("running config")),
    l("startup-config", ActTag::Show(ShowKind::StartupConfig), Some("startup config")),
    l("status", ActTag::Show(ShowKind::Status), Some("status")),
    l("version", ActTag::Show(ShowKind::Version), Some("version")),
];

/// switch show bank — adds mac / switchport / vlan.
static SHOW_KIDS_SWITCH: &[GNode] = &[
    l("arp", ActTag::Show(ShowKind::Arp), Some("ARP table")),
    l("history", ActTag::Show(ShowKind::History), Some("history")),
    l("interfaces", ActTag::Show(ShowKind::Interfaces), Some("interfaces")),
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: SHOW_IP_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("logging"),
        help: Some("logging"),
        hidden: false,
        act: None,
        kids: SHOW_LOGGING,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("mac"),
        help: Some("MAC"),
        hidden: false,
        act: None,
        kids: SHOW_MAC,
        edge: Edge::None,
        emits: false,
    },
    l("running-config", ActTag::Show(ShowKind::RunningConfig), Some("running config")),
    l("startup-config", ActTag::Show(ShowKind::StartupConfig), Some("startup config")),
    l("status", ActTag::Show(ShowKind::Status), Some("status")),
    l("switchport", ActTag::Show(ShowKind::Switchport), Some("switchport")),
    GNode {
        tok: Tok::Lit("vlan"),
        help: Some("VLANs"),
        hidden: false,
        act: Some(ActTag::Show(ShowKind::Vlan)),
        kids: &[l("brief", ActTag::Show(ShowKind::Vlan), Some("brief"))],
        edge: Edge::None,
        emits: false,
    },
    l("version", ActTag::Show(ShowKind::Version), Some("version")),
];

static PING_KIDS: &[GNode] = &[
    gh(
        Tok::Lit("-c"),
        None,
        &[GNode {
            tok: Tok::Range { lo: 1, hi: 100 },
            help: Some("<count>"),
            hidden: true,
            act: None,
            kids: &[GNode {
                tok: Tok::Ipv4,
                help: Some("<A.B.C.D>"),
                hidden: true,
                act: Some(ActTag::PingHiddenC),
                kids: EMPTY_KIDS,
                edge: Edge::None,
                emits: false,
            }],
            edge: Edge::None,
            emits: false,
        }],
        None,
    ),
    GNode {
        tok: Tok::Ipv4,
        help: Some("<A.B.C.D>"),
        hidden: false,
        act: Some(ActTag::Ping),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    },
];

static TRACEROUTE_KIDS: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<A.B.C.D>"),
    hidden: false,
    act: Some(ActTag::Traceroute),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static TERMINAL_HISTORY: &[GNode] = &[GNode {
    tok: Tok::Lit("size"),
    help: Some("history size"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Range { lo: 0, hi: 256 },
        help: Some("<0-256>"),
        hidden: false,
        act: Some(ActTag::HistorySize),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static TERMINAL_KIDS: &[GNode] = &[GNode {
    tok: Tok::Lit("history"),
    help: Some("history"),
    hidden: false,
    act: None,
    kids: TERMINAL_HISTORY,
    edge: Edge::None,
    emits: false,
}];

static CLEAR_IP_NAT: &[GNode] =
    &[l("translation", ActTag::Clear(ClearKind::NatTranslation), Some("translations"))];

static CLEAR_IP_OSPF: &[GNode] =
    &[l("process", ActTag::Clear(ClearKind::OspfProcess), Some("process"))];

static CLEAR_IP_ROUTE_STAR: &[GNode] =
    &[l("*", ActTag::Clear(ClearKind::IpRouteAll), Some("all routes"))];

static CLEAR_IP: &[GNode] = &[
    GNode {
        tok: Tok::Lit("nat"),
        help: Some("NAT"),
        hidden: false,
        act: None,
        kids: CLEAR_IP_NAT,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("ospf"),
        help: Some("OSPF"),
        hidden: false,
        act: None,
        kids: CLEAR_IP_OSPF,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("route"),
        help: Some("routes"),
        hidden: false,
        act: None,
        kids: CLEAR_IP_ROUTE_STAR,
        edge: Edge::None,
        emits: false,
    },
];

static CLEAR_MAC: &[GNode] = &[GNode {
    tok: Tok::Lit("address-table"),
    help: Some("MAC table"),
    hidden: false,
    act: None,
    kids: &[l("dynamic", ActTag::Clear(ClearKind::MacTable), Some("dynamic entries"))],
    edge: Edge::None,
    emits: false,
}];

static CLEAR_KIDS: &[GNode] = &[
    l("arp-cache", ActTag::Clear(ClearKind::Arp), Some("ARP cache")),
    CLEAR_COUNTERS_NODE,
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: CLEAR_IP,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("mac"),
        help: Some("MAC"),
        hidden: false,
        act: None,
        kids: CLEAR_MAC,
        edge: Edge::None,
        emits: false,
    },
];

static COPY_KIDS: &[GNode] = &[GNode {
    tok: Tok::Lit("running-config"),
    help: Some("running config"),
    hidden: false,
    act: None,
    kids: &[l("startup-config", ActTag::CopyRunStartAsk, Some("startup config"))],
    edge: Edge::None,
    emits: false,
}];

static WRITE_KIDS: &[GNode] = &[
    l("memory", ActTag::WriteMem, Some("write to nvram")),
    l("erase", ActTag::WriteEraseAsk, Some("erase nvram")),
];

static ERASE_KIDS: &[GNode] =
    &[l("startup-config", ActTag::EraseStartupAsk, Some("startup config"))];

static CONFIGURE_KIDS: &[GNode] =
    &[l("terminal", ActTag::Configure, Some("from terminal"))];

static DO_KIDS: &[GNode] = &[
    GNode {
        tok: Tok::Lit("copy"),
        help: Some("copy"),
        hidden: false,
        act: None,
        kids: COPY_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("write"),
        help: Some("write"),
        hidden: false,
        act: None,
        kids: WRITE_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("erase"),
        help: Some("erase"),
        hidden: false,
        act: None,
        kids: ERASE_KIDS,
        edge: Edge::None,
        emits: false,
    },
    l("reload", ActTag::ReloadAsk, Some("reload")),
    GNode {
        tok: Tok::Lit("clear"),
        help: Some("clear"),
        hidden: false,
        act: None,
        kids: CLEAR_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("ping"),
        help: Some("ping"),
        hidden: false,
        act: None,
        kids: PING_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("traceroute"),
        help: Some("traceroute"),
        hidden: false,
        act: None,
        kids: TRACEROUTE_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("show"),
        help: Some("show"),
        hidden: false,
        act: None,
        kids: SHOW_KIDS_ROUTER,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("terminal"),
        help: Some("terminal"),
        hidden: false,
        act: None,
        kids: TERMINAL_KIDS,
        edge: Edge::None,
        emits: false,
    },
];

// do-show needs the switch bank too — a second DO set for switches.
static DO_KIDS_SWITCH: &[GNode] = &[
    GNode {
        tok: Tok::Lit("copy"),
        help: Some("copy"),
        hidden: false,
        act: None,
        kids: COPY_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("write"),
        help: Some("write"),
        hidden: false,
        act: None,
        kids: WRITE_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("erase"),
        help: Some("erase"),
        hidden: false,
        act: None,
        kids: ERASE_KIDS,
        edge: Edge::None,
        emits: false,
    },
    l("reload", ActTag::ReloadAsk, Some("reload")),
    GNode {
        tok: Tok::Lit("clear"),
        help: Some("clear"),
        hidden: false,
        act: None,
        kids: CLEAR_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("ping"),
        help: Some("ping"),
        hidden: false,
        act: None,
        kids: PING_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("traceroute"),
        help: Some("traceroute"),
        hidden: false,
        act: None,
        kids: TRACEROUTE_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("show"),
        help: Some("show"),
        hidden: false,
        act: None,
        kids: SHOW_KIDS_SWITCH,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("terminal"),
        help: Some("terminal"),
        hidden: false,
        act: None,
        kids: TERMINAL_KIDS,
        edge: Edge::None,
        emits: false,
    },
];

static EXEC_ROOT_ROUTER: &[GNode] = &[
    l("enable", ActTag::Enable, Some("privileged exec")),
    l("exit", ActTag::Exit, Some("exit session")),
    l("logout", ActTag::Logout, Some("log out")),
    g(Tok::Lit("ping"), None, PING_KIDS, Some("ping")),
    g(Tok::Lit("traceroute"), None, TRACEROUTE_KIDS, Some("traceroute")),
    g(Tok::Lit("show"), None, SHOW_KIDS_ROUTER, Some("show")),
    g(Tok::Lit("terminal"), None, TERMINAL_KIDS, Some("terminal")),
    ge("do", Edge::Do, DO_KIDS, Some("run an exec command")),
];

static EXEC_ROOT_SWITCH: &[GNode] = &[
    l("enable", ActTag::Enable, Some("privileged exec")),
    l("exit", ActTag::Exit, Some("exit session")),
    l("logout", ActTag::Logout, Some("log out")),
    g(Tok::Lit("ping"), None, PING_KIDS, Some("ping")),
    g(Tok::Lit("traceroute"), None, TRACEROUTE_KIDS, Some("traceroute")),
    g(Tok::Lit("show"), None, SHOW_KIDS_SWITCH, Some("show")),
    g(Tok::Lit("terminal"), None, TERMINAL_KIDS, Some("terminal")),
    ge("do", Edge::Do, DO_KIDS_SWITCH, Some("run an exec command")),
];

static PRIVEXEC_ROOT_ROUTER: &[GNode] = &[
    l("enable", ActTag::Enable, Some("privileged exec")),
    l("exit", ActTag::Exit, Some("exit session")),
    l("logout", ActTag::Logout, Some("log out")),
    g(Tok::Lit("ping"), None, PING_KIDS, Some("ping")),
    g(Tok::Lit("traceroute"), None, TRACEROUTE_KIDS, Some("traceroute")),
    g(Tok::Lit("show"), None, SHOW_KIDS_ROUTER, Some("show")),
    g(Tok::Lit("terminal"), None, TERMINAL_KIDS, Some("terminal")),
    ge("do", Edge::Do, DO_KIDS, Some("run an exec command")),
    l("disable", ActTag::Disable, Some("user exec")),
    g(Tok::Lit("configure"), None, CONFIGURE_KIDS, Some("configure")),
    g(Tok::Lit("copy"), None, COPY_KIDS, Some("copy")),
    g(Tok::Lit("write"), None, WRITE_KIDS, Some("write")),
    g(Tok::Lit("erase"), None, ERASE_KIDS, Some("erase")),
    l("reload", ActTag::ReloadAsk, Some("reload")),
    g(Tok::Lit("clear"), None, CLEAR_KIDS, Some("clear")),
];

static PRIVEXEC_ROOT_SWITCH: &[GNode] = &[
    l("enable", ActTag::Enable, Some("privileged exec")),
    l("exit", ActTag::Exit, Some("exit session")),
    l("logout", ActTag::Logout, Some("log out")),
    g(Tok::Lit("ping"), None, PING_KIDS, Some("ping")),
    g(Tok::Lit("traceroute"), None, TRACEROUTE_KIDS, Some("traceroute")),
    g(Tok::Lit("show"), None, SHOW_KIDS_SWITCH, Some("show")),
    g(Tok::Lit("terminal"), None, TERMINAL_KIDS, Some("terminal")),
    ge("do", Edge::Do, DO_KIDS_SWITCH, Some("run an exec command")),
    l("disable", ActTag::Disable, Some("user exec")),
    g(Tok::Lit("configure"), None, CONFIGURE_KIDS, Some("configure")),
    g(Tok::Lit("copy"), None, COPY_KIDS, Some("copy")),
    g(Tok::Lit("write"), None, WRITE_KIDS, Some("write")),
    g(Tok::Lit("erase"), None, ERASE_KIDS, Some("erase")),
    l("reload", ActTag::ReloadAsk, Some("reload")),
    g(Tok::Lit("clear"), None, CLEAR_KIDS, Some("clear")),
];

static IP_ROUTE_TAIL: &[GNode] = &[
    // ip route <net> <mask> <nh> [<ad>]
    GNode {
        tok: Tok::Range { lo: 1, hi: 255 },
        help: Some("<ad>"),
        hidden: false,
        act: Some(ActTag::AddRoute),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    },
];

static IP_ROUTE_NH: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<nh>"),
    hidden: false,
    act: Some(ActTag::AddRoute),
    kids: IP_ROUTE_TAIL,
    edge: Edge::None,
    emits: false,
}];

static IP_ROUTE_MASK: &[GNode] = &[GNode {
    tok: Tok::Mask,
    help: Some("<mask>"),
    hidden: false,
    act: None,
    kids: IP_ROUTE_NH,
    edge: Edge::None,
    emits: false,
}];

static IP_ROUTE_NET: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<net>"),
    hidden: false,
    act: None,
    kids: IP_ROUTE_MASK,
    edge: Edge::None,
    emits: false,
}];

static IP_DHCP_EXCL_HI: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<hi>"),
    hidden: false,
    act: Some(ActTag::DhcpExcludedAdd),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static IP_DHCP_EXCL_LO: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<lo>"),
    hidden: false,
    act: Some(ActTag::DhcpExcludedAdd),
    kids: IP_DHCP_EXCL_HI,
    edge: Edge::None,
    emits: false,
}];

static IP_DHCP: &[GNode] = &[GNode {
    tok: Tok::Lit("excluded-address"),
    help: Some("excluded addresses"),
    hidden: false,
    act: None,
    kids: IP_DHCP_EXCL_LO,
    edge: Edge::None,
    emits: false,
}];

static IP_NAT_STATIC: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<il>"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Ipv4,
        help: Some("<ig>"),
        hidden: false,
        act: Some(ActTag::NatInsideStaticAdd),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static IP_NAT_SRC: &[GNode] = &[GNode {
    tok: Tok::Lit("static"),
    help: Some("static map"),
    hidden: false,
    act: None,
    kids: IP_NAT_STATIC,
    edge: Edge::None,
    emits: false,
}];

static IP_NAT: &[GNode] = &[GNode {
    tok: Tok::Lit("inside"),
    help: Some("inside"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Lit("source"),
        help: Some("source"),
        hidden: false,
        act: None,
        kids: IP_NAT_SRC,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static IP_KIDS: &[GNode] = &[
    l("domain-lookup", ActTag::Toggle(ToggleKind::DomainLookup), Some("DNS lookups")),
    GNode {
        tok: Tok::Lit("route"),
        help: Some("static route"),
        hidden: false,
        act: None,
        kids: IP_ROUTE_NET,
        edge: Edge::None,
        emits: false,
    },
    l("routing", ActTag::Toggle(ToggleKind::IpRouting), Some("IP routing")),
    l("proxy-arp", ActTag::Toggle(ToggleKind::ProxyArp), Some("proxy ARP")),
    l(
        "directed-broadcast",
        ActTag::Toggle(ToggleKind::DirectedBroadcast),
        Some("directed broadcast"),
    ),
    GNode {
        tok: Tok::Lit("dhcp"),
        help: Some("DHCP"),
        hidden: false,
        act: None,
        kids: IP_DHCP,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("nat"),
        help: Some("NAT"),
        hidden: false,
        act: None,
        kids: IP_NAT,
        edge: Edge::None,
        emits: false,
    },
];

/// <addr> seats: any | host <ip> | <ip> <wild>
static ACE_DST: &[GNode] = &[
    gd("any", Some(ActTag::AclAceAdd), EMPTY_KIDS, Some("any host")),
    gd(
        "host",
        None,
        &[GNode {
            tok: Tok::Ipv4,
            help: Some("<A.B.C.D>"),
            hidden: false,
            act: Some(ActTag::AclAceAdd),
            kids: EMPTY_KIDS,
            edge: Edge::None,
            emits: false,
        }],
        Some("a host"),
    ),
    GNode {
        tok: Tok::Ipv4,
        help: Some("<net>"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Mask,
            help: Some("<wild>"),
            hidden: false,
            act: Some(ActTag::AclAceAdd),
            kids: EMPTY_KIDS,
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    },
];

static ACE_SRC: &[GNode] = &[
    gd("any", None, ACE_DST, Some("any host")),
    gd(
        "host",
        None,
        &[GNode {
            tok: Tok::Ipv4,
            help: Some("<A.B.C.D>"),
            hidden: false,
            act: None,
            kids: ACE_DST,
            edge: Edge::None,
            emits: false,
        }],
        Some("a host"),
    ),
    GNode {
        tok: Tok::Ipv4,
        help: Some("<net>"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Mask,
            help: Some("<wild>"),
            hidden: false,
            act: None,
            kids: ACE_DST,
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    },
];

static ACE_PROTO: &[GNode] = &[
    gd("ip", None, ACE_SRC, Some("any IP")),
    gd("icmp", None, ACE_SRC, Some("ICMP")),
    gd("tcp", None, ACE_SRC, Some("TCP")),
    gd("udp", None, ACE_SRC, Some("UDP")),
];

static ACE_ACTION: &[GNode] = &[
    gd("permit", None, ACE_PROTO, Some("permit")),
    gd("deny", None, ACE_PROTO, Some("deny")),
];

static ACCESS_LIST_KIDS: &[GNode] = &[GNode {
    tok: Tok::Range { lo: 1, hi: 2699 },
    help: Some("<1-2699>"),
    hidden: false,
    act: None,
    kids: ACE_ACTION,
    edge: Edge::None,
    emits: false,
}];

// no access-list <n> [<ace…>] — the bare form drops the whole list; an
// ace tail removes that single entry.
static NO_ACE_DST: &[GNode] = &[
    gd("any", Some(ActTag::NegAclAce), EMPTY_KIDS, Some("any host")),
    gd(
        "host",
        None,
        &[GNode {
            tok: Tok::Ipv4,
            help: Some("<A.B.C.D>"),
            hidden: false,
            act: Some(ActTag::NegAclAce),
            kids: EMPTY_KIDS,
            edge: Edge::None,
            emits: false,
        }],
        Some("a host"),
    ),
    GNode {
        tok: Tok::Ipv4,
        help: Some("<net>"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Mask,
            help: Some("<wild>"),
            hidden: false,
            act: Some(ActTag::NegAclAce),
            kids: EMPTY_KIDS,
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    },
];

static NO_ACE_SRC: &[GNode] = &[
    gd("any", None, NO_ACE_DST, Some("any host")),
    gd(
        "host",
        None,
        &[GNode {
            tok: Tok::Ipv4,
            help: Some("<A.B.C.D>"),
            hidden: false,
            act: None,
            kids: NO_ACE_DST,
            edge: Edge::None,
            emits: false,
        }],
        Some("a host"),
    ),
    GNode {
        tok: Tok::Ipv4,
        help: Some("<net>"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Mask,
            help: Some("<wild>"),
            hidden: false,
            act: None,
            kids: NO_ACE_DST,
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    },
];

static NO_ACE_PROTO: &[GNode] = &[
    gd("ip", None, NO_ACE_SRC, Some("any IP")),
    gd("icmp", None, NO_ACE_SRC, Some("ICMP")),
    gd("tcp", None, NO_ACE_SRC, Some("TCP")),
    gd("udp", None, NO_ACE_SRC, Some("UDP")),
];

static NO_ACE_ACTION: &[GNode] = &[
    gd("permit", None, NO_ACE_PROTO, Some("permit")),
    gd("deny", None, NO_ACE_PROTO, Some("deny")),
];

static NO_IP_ROUTE_MASK: &[GNode] = &[GNode {
    tok: Tok::Mask,
    help: Some("<mask>"),
    hidden: false,
    act: Some(ActTag::NegRoute),
    kids: &[GNode {
        tok: Tok::Ipv4,
        help: Some("<nh>"),
        hidden: false,
        act: Some(ActTag::NegRoute),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static NO_IP_DHCP_EXCL: &[GNode] = &[GNode {
    tok: Tok::Lit("excluded-address"),
    help: Some("excluded addresses"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Ipv4,
        help: Some("<lo>"),
        hidden: false,
        act: Some(ActTag::NegDhcpExcluded),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static NO_IP_NAT_STATIC: &[GNode] = &[GNode {
    tok: Tok::Lit("static"),
    help: Some("static map"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Ipv4,
        help: Some("<il>"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Ipv4,
            help: Some("<ig>"),
            hidden: false,
            act: Some(ActTag::NegNatInsideStatic),
            kids: EMPTY_KIDS,
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static NO_IP_NAT: &[GNode] = &[GNode {
    tok: Tok::Lit("inside"),
    help: Some("inside"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Lit("source"),
        help: Some("source"),
        hidden: false,
        act: None,
        kids: NO_IP_NAT_STATIC,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static NO_IP_KIDS: &[GNode] = &[
    l("domain-lookup", ActTag::Toggle(ToggleKind::DomainLookup), Some("DNS lookups")),
    GNode {
        tok: Tok::Lit("route"),
        help: Some("static route"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Ipv4,
            help: Some("<net>"),
            hidden: false,
            act: None,
            kids: NO_IP_ROUTE_MASK,
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    },
    l("routing", ActTag::Toggle(ToggleKind::IpRouting), Some("IP routing")),
    l("proxy-arp", ActTag::Toggle(ToggleKind::ProxyArp), Some("proxy ARP")),
    l(
        "directed-broadcast",
        ActTag::Toggle(ToggleKind::DirectedBroadcast),
        Some("directed broadcast"),
    ),
    GNode {
        tok: Tok::Lit("dhcp"),
        help: Some("DHCP"),
        hidden: false,
        act: None,
        kids: NO_IP_DHCP_EXCL,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("nat"),
        help: Some("NAT"),
        hidden: false,
        act: None,
        kids: NO_IP_NAT,
        edge: Edge::None,
        emits: false,
    },
];

static NO_ROUTER_OSPF: &[GNode] = &[GNode {
    tok: Tok::Range { lo: 1, hi: 65535 },
    help: Some("<1-65535>"),
    hidden: false,
    act: Some(ActTag::NegOspfProcess),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static NO_ROUTER: &[GNode] = &[GNode {
    tok: Tok::Lit("ospf"),
    help: Some("OSPF"),
    hidden: false,
    act: None,
    kids: NO_ROUTER_OSPF,
    edge: Edge::None,
    emits: false,
}];

static NO_SERVICE: &[GNode] =
    &[l("dhcp", ActTag::ServiceDhcp, Some("DHCP service"))];

/// `no`/`default` kids under global Config — router form (no `vlan`).
static NO_CONFIG_ROUTER: &[GNode] = &[
    l("hostname", ActTag::NegHostname, Some("hostname")),
    NO_ACCESS_LIST_NODE,
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: NO_IP_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("router"),
        help: Some("routing process"),
        hidden: false,
        act: None,
        kids: NO_ROUTER,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("service"),
        help: Some("service"),
        hidden: false,
        act: None,
        kids: NO_SERVICE,
        edge: Edge::None,
        emits: false,
    },
];

/// switch form — adds `no vlan <id>`.
static NO_CONFIG_SWITCH: &[GNode] = &[
    l("hostname", ActTag::NegHostname, Some("hostname")),
    NO_ACCESS_LIST_NODE,
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: NO_IP_KIDS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("vlan"),
        help: Some("VLAN"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Range { lo: 1, hi: 4094 },
            help: Some("<1-4094>"),
            hidden: false,
            act: Some(ActTag::NegVlan),
            kids: EMPTY_KIDS,
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("service"),
        help: Some("service"),
        hidden: false,
        act: None,
        kids: NO_SERVICE,
        edge: Edge::None,
        emits: false,
    },
];

static CONFIG_HOSTNAME: &[GNode] = &[GNode {
    tok: Tok::Word,
    help: Some("WORD"),
    hidden: false,
    act: Some(ActTag::Hostname),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static CONFIG_IF: &[GNode] = &[GNode {
    tok: Tok::IfName,
    help: Some("<if>"),
    hidden: false,
    act: Some(ActTag::EnterIf),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static CONFIG_VLAN: &[GNode] = &[GNode {
    tok: Tok::Range { lo: 1, hi: 4094 },
    help: Some("<1-4094>"),
    hidden: false,
    act: Some(ActTag::EnterVlan),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static CONFIG_ROUTER_OSPF: &[GNode] = &[GNode {
    tok: Tok::Lit("ospf"),
    help: Some("OSPF"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Range { lo: 1, hi: 65535 },
        help: Some("<1-65535>"),
        hidden: false,
        act: Some(ActTag::EnterRouterOspf),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static CONFIG_ROOT_ROUTER: &[GNode] = &[
    g(Tok::Lit("hostname"), None, CONFIG_HOSTNAME, Some("hostname")),
    g(Tok::Lit("interface"), None, CONFIG_IF, Some("interface")),
    g(Tok::Lit("router"), None, CONFIG_ROUTER_OSPF, Some("routing process")),
    g(Tok::Lit("access-list"), None, ACCESS_LIST_KIDS, Some("ACL")),
    g(Tok::Lit("ip"), None, IP_KIDS, Some("IP")),
    g(Tok::Lit("service"), None, &[l("dhcp", ActTag::ServiceDhcp, Some("DHCP service"))], Some("service")),
    ge("no", Edge::Neg(crate::cli::types::NegKind::No), NO_CONFIG_ROUTER, Some("negate")),
    ge(
        "default",
        Edge::Neg(crate::cli::types::NegKind::Default),
        NO_CONFIG_ROUTER,
        Some("set to default"),
    ),
    ge("do", Edge::Do, DO_KIDS, Some("run an exec command")),
    l("end", ActTag::End, Some("end config")),
    l("exit", ActTag::Exit, Some("exit config")),
];

static CONFIG_ROOT_SWITCH: &[GNode] = &[
    g(Tok::Lit("hostname"), None, CONFIG_HOSTNAME, Some("hostname")),
    g(Tok::Lit("interface"), None, CONFIG_IF, Some("interface")),
    g(Tok::Lit("vlan"), None, CONFIG_VLAN, Some("VLAN")),
    g(Tok::Lit("access-list"), None, ACCESS_LIST_KIDS, Some("ACL")),
    g(Tok::Lit("ip"), None, IP_KIDS, Some("IP")),
    g(Tok::Lit("service"), None, &[l("dhcp", ActTag::ServiceDhcp, Some("DHCP service"))], Some("service")),
    ge("no", Edge::Neg(crate::cli::types::NegKind::No), NO_CONFIG_SWITCH, Some("negate")),
    ge(
        "default",
        Edge::Neg(crate::cli::types::NegKind::Default),
        NO_CONFIG_SWITCH,
        Some("set to default"),
    ),
    ge("do", Edge::Do, DO_KIDS_SWITCH, Some("run an exec command")),
    l("end", ActTag::End, Some("end config")),
    l("exit", ActTag::Exit, Some("exit config")),
];

static IF_DESC: &[GNode] = &[GNode {
    tok: Tok::Line,
    help: Some("LINE"),
    hidden: false,
    act: Some(ActTag::IfDescription),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static IF_IP_ADDR: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<A.B.C.D>"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Mask,
        help: Some("<mask>"),
        hidden: false,
        act: Some(ActTag::SetIp),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static IF_AG_DIR: &[GNode] = &[
    gd("in", Some(ActTag::IpAccessGroup), EMPTY_KIDS, Some("inbound")),
    gd("out", Some(ActTag::IpAccessGroup), EMPTY_KIDS, Some("outbound")),
];

static IF_NAT_DIR: &[GNode] = &[
    gd("inside", Some(ActTag::NatIfInside), EMPTY_KIDS, Some("inside")),
    gd("outside", Some(ActTag::NatIfOutside), EMPTY_KIDS, Some("outside")),
];

/// router config-if ip kids: address + access-group + nat.
static IF_IP_KIDS_ROUTER: &[GNode] = &[
    GNode {
        tok: Tok::Lit("address"),
        help: Some("address"),
        hidden: false,
        act: None,
        kids: IF_IP_ADDR,
        edge: Edge::None,
        emits: false,
    },
    IF_IP_AG_NODE,
    IF_NAT_NODE,
];

/// switch config-if ip kids: address + access-group (heads installed;
/// setters return KindGate).
static IF_IP_KIDS_SWITCH: &[GNode] = &[
    GNode {
        tok: Tok::Lit("address"),
        help: Some("address"),
        hidden: false,
        act: None,
        kids: IF_IP_ADDR,
        edge: Edge::None,
        emits: false,
    },
    IF_IP_AG_NODE,
];

static SW_MODE: &[GNode] = &[
    l("access", ActTag::SwModeAccess, Some("access mode")),
    l("trunk", ActTag::SwModeTrunk, Some("trunk mode")),
];

static SW_ACCESS: &[GNode] = &[GNode {
    tok: Tok::Lit("vlan"),
    help: Some("VLAN"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Range { lo: 1, hi: 4094 },
        help: Some("<1-4094>"),
        hidden: false,
        act: Some(ActTag::SwAccessVlan),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static SW_TRUNK_ALLOWED: &[GNode] = &[GNode {
    tok: Tok::Lit("vlan"),
    help: Some("VLAN list"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::VlanList,
        help: Some("<list>"),
        hidden: false,
        act: Some(ActTag::SwTrunkAllowed),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static SW_TRUNK: &[GNode] = &[GNode {
    tok: Tok::Lit("allowed"),
    help: Some("allowed VLANs"),
    hidden: false,
    act: None,
    kids: SW_TRUNK_ALLOWED,
    edge: Edge::None,
    emits: false,
}];

static SW_KIDS: &[GNode] = &[
    GNode {
        tok: Tok::Lit("mode"),
        help: Some("mode"),
        hidden: false,
        act: None,
        kids: SW_MODE,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("access"),
        help: Some("access"),
        hidden: false,
        act: None,
        kids: SW_ACCESS,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("trunk"),
        help: Some("trunk"),
        hidden: false,
        act: None,
        kids: SW_TRUNK,
        edge: Edge::None,
        emits: false,
    },
];

// config-if no-kids
static NO_IF_AG_DIR: &[GNode] = &[
    gd("in", Some(ActTag::NegIpAccessGroup), EMPTY_KIDS, Some("inbound")),
    gd("out", Some(ActTag::NegIpAccessGroup), EMPTY_KIDS, Some("outbound")),
];

static NO_IF_NAT_DIR: &[GNode] = &[
    gd("inside", Some(ActTag::NegNatIfSide), EMPTY_KIDS, Some("inside")),
    gd("outside", Some(ActTag::NegNatIfSide), EMPTY_KIDS, Some("outside")),
];

static NO_IF_IP_ROUTER: &[GNode] = &[
    l("address", ActTag::NegIfIp, Some("address")),
    NO_IF_AG_NODE,
    NO_IF_NAT_NODE,
];

static NO_IF_IP_SWITCH: &[GNode] = &[
    l("address", ActTag::NegIfIp, Some("address")),
    NO_IF_AG_NODE,
];

static NO_SW: &[GNode] = &[
    l("mode", ActTag::NegSwMode, Some("mode")),
    GNode {
        tok: Tok::Lit("access"),
        help: Some("access"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Lit("vlan"),
            help: Some("VLAN"),
            hidden: false,
            act: Some(ActTag::NegSwAccessVlan),
            kids: EMPTY_KIDS,
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("trunk"),
        help: Some("trunk"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Lit("allowed"),
            help: Some("allowed VLANs"),
            hidden: false,
            act: None,
            kids: &[GNode {
                tok: Tok::Lit("vlan"),
                help: Some("VLANs"),
                hidden: false,
                act: Some(ActTag::NegSwTrunkAllowed),
                kids: EMPTY_KIDS,
                edge: Edge::None,
                emits: false,
            }],
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    },
];

static NO_IF_KIDS_ROUTER: &[GNode] = &[
    l("description", ActTag::NegIfDescription, Some("description")),
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: NO_IF_IP_ROUTER,
        edge: Edge::None,
        emits: false,
    },
    l("shutdown", ActTag::IfAdmin, Some("shutdown")),
];

static NO_IF_KIDS_SWITCH: &[GNode] = &[
    l("description", ActTag::NegIfDescription, Some("description")),
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: NO_IF_IP_SWITCH,
        edge: Edge::None,
        emits: false,
    },
    l("shutdown", ActTag::IfAdmin, Some("shutdown")),
    GNode {
        tok: Tok::Lit("switchport"),
        help: Some("switchport"),
        hidden: false,
        act: None,
        kids: NO_SW,
        edge: Edge::None,
        emits: false,
    },
];

static CONFIG_IF_ROOT_ROUTER: &[GNode] = &[
    g(Tok::Lit("description"), None, IF_DESC, Some("description")),
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: IF_IP_KIDS_ROUTER,
        edge: Edge::None,
        emits: false,
    },
    l("shutdown", ActTag::IfAdmin, Some("shutdown")),
    ge("no", Edge::Neg(crate::cli::types::NegKind::No), NO_IF_KIDS_ROUTER, Some("negate")),
    ge(
        "default",
        Edge::Neg(crate::cli::types::NegKind::Default),
        NO_IF_KIDS_ROUTER,
        Some("set to default"),
    ),
    ge("do", Edge::Do, DO_KIDS, Some("run an exec command")),
    l("end", ActTag::End, Some("end config")),
    l("exit", ActTag::Exit, Some("exit config")),
];

static CONFIG_IF_ROOT_SWITCH: &[GNode] = &[
    g(Tok::Lit("description"), None, IF_DESC, Some("description")),
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: IF_IP_KIDS_SWITCH,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("switchport"),
        help: Some("switchport"),
        hidden: false,
        act: None,
        kids: SW_KIDS,
        edge: Edge::None,
        emits: false,
    },
    l("shutdown", ActTag::IfAdmin, Some("shutdown")),
    ge("no", Edge::Neg(crate::cli::types::NegKind::No), NO_IF_KIDS_SWITCH, Some("negate")),
    ge(
        "default",
        Edge::Neg(crate::cli::types::NegKind::Default),
        NO_IF_KIDS_SWITCH,
        Some("set to default"),
    ),
    ge("do", Edge::Do, DO_KIDS_SWITCH, Some("run an exec command")),
    l("end", ActTag::End, Some("end config")),
    l("exit", ActTag::Exit, Some("exit config")),
];

static VLAN_NAME: &[GNode] = &[GNode {
    tok: Tok::Word,
    help: Some("WORD"),
    hidden: false,
    act: Some(ActTag::VlanName),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static VLAN_STATE: &[GNode] = &[
    gd("active", Some(ActTag::VlanStateSet), EMPTY_KIDS, Some("active")),
    gd("suspend", Some(ActTag::VlanStateSet), EMPTY_KIDS, Some("suspend")),
];

static NO_VLAN_KIDS: &[GNode] = &[
    l("name", ActTag::NegVlanName, Some("name")),
    l("state", ActTag::VlanStateSet, Some("state")),
];

static CONFIG_VLAN_ROOT: &[GNode] = &[
    g(Tok::Lit("name"), None, VLAN_NAME, Some("name")),
    g(Tok::Lit("state"), None, VLAN_STATE, Some("state")),
    ge("no", Edge::Neg(crate::cli::types::NegKind::No), NO_VLAN_KIDS, Some("negate")),
    ge(
        "default",
        Edge::Neg(crate::cli::types::NegKind::Default),
        NO_VLAN_KIDS,
        Some("set to default"),
    ),
    ge("do", Edge::Do, DO_KIDS_SWITCH, Some("run an exec command")),
    l("end", ActTag::End, Some("end config")),
    l("exit", ActTag::Exit, Some("exit config")),
];

static OSPF_NET: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<net>"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Mask,
        help: Some("<wild>"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Lit("area"),
            help: Some("area"),
            hidden: false,
            act: None,
            kids: &[GNode {
                tok: Tok::Range { lo: 0, hi: 4294967295 },
                help: Some("<area>"),
                hidden: false,
                act: Some(ActTag::OspfNetAdd),
                kids: EMPTY_KIDS,
                edge: Edge::None,
                emits: false,
            }],
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static OSPF_RID: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<A.B.C.D>"),
    hidden: false,
    act: Some(ActTag::OspfRouterIdSet),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static NO_OSPF_NET: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<net>"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Mask,
        help: Some("<wild>"),
        hidden: false,
        act: None,
        kids: &[GNode {
            tok: Tok::Lit("area"),
            help: Some("area"),
            hidden: false,
            act: None,
            kids: &[GNode {
                tok: Tok::Range { lo: 0, hi: 4294967295 },
                help: Some("<area>"),
                hidden: false,
                act: Some(ActTag::NegOspfNet),
                kids: EMPTY_KIDS,
                edge: Edge::None,
                emits: false,
            }],
            edge: Edge::None,
            emits: false,
        }],
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static NO_ROUTERCFG_KIDS: &[GNode] = &[
    GNode {
        tok: Tok::Lit("network"),
        help: Some("network"),
        hidden: false,
        act: None,
        kids: NO_OSPF_NET,
        edge: Edge::None,
        emits: false,
    },
    l("router-id", ActTag::NegOspfRouterId, Some("router-id")),
];

static CONFIG_ROUTER_ROOT: &[GNode] = &[
    g(Tok::Lit("network"), None, OSPF_NET, Some("network")),
    g(Tok::Lit("router-id"), None, OSPF_RID, Some("router-id")),
    ge(
        "no",
        Edge::Neg(crate::cli::types::NegKind::No),
        NO_ROUTERCFG_KIDS,
        Some("negate"),
    ),
    ge(
        "default",
        Edge::Neg(crate::cli::types::NegKind::Default),
        NO_ROUTERCFG_KIDS,
        Some("set to default"),
    ),
    ge("do", Edge::Do, DO_KIDS, Some("run an exec command")),
    l("end", ActTag::End, Some("end config")),
    l("exit", ActTag::Exit, Some("exit config")),
];

static HOST_IP_PREFIX_GW: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<gw>"),
    hidden: false,
    act: Some(ActTag::HostIp),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static HOST_IP_MASK_GW: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<gw>"),
    hidden: false,
    act: Some(ActTag::HostIp),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static HOST_IP_MASK: &[GNode] = &[GNode {
    tok: Tok::Mask,
    help: Some("<mask>"),
    hidden: false,
    act: Some(ActTag::HostIp),
    kids: HOST_IP_MASK_GW,
    edge: Edge::None,
    emits: false,
}];

static HOST_IP_KIDS: &[GNode] = &[
    GNode {
        tok: Tok::Ipv4Prefix,
        help: Some("<A.B.C.D/len>"),
        hidden: false,
        act: Some(ActTag::HostIp),
        kids: HOST_IP_PREFIX_GW,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Ipv4,
        help: Some("<A.B.C.D>"),
        hidden: false,
        act: None,
        kids: HOST_IP_MASK,
        edge: Edge::None,
        emits: false,
    },
    l("dhcp", ActTag::HostDhcp, Some("DHCP")),
    g(
        Tok::Lit("dns"),
        Some(ActTag::HostDns),
        &[GNode {
            tok: Tok::Ipv4,
            help: Some("<server>"),
            hidden: false,
            act: Some(ActTag::HostDns),
            kids: EMPTY_KIDS,
            edge: Edge::None,
            emits: false,
        }],
        Some("resolver"),
    ),
];

static HOST_SET: &[GNode] = &[GNode {
    tok: Tok::Lit("pcname"),
    help: Some("pcname"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Word,
        help: Some("WORD"),
        hidden: false,
        act: Some(ActTag::Pcname),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static HOST_SHOW: &[GNode] = &[
    l("arp", ActTag::Show(ShowKind::Arp), Some("ARP table")),
    l("ip", ActTag::Show(ShowKind::HostIp), Some("IP configuration")),
];

const fn host_extra(lit: &'static str, k: HostExtraKind) -> GNode {
    GNode {
        tok: Tok::Lit(lit),
        help: Some(lit),
        hidden: false,
        act: Some(ActTag::HostExtra(k)),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }
}

// `nc [-u] <ip> <port> [payload]` — the flag rides argv as Str("-u").
static NC_PORT_KIDS: &[GNode] = &[GNode {
    tok: Tok::Range { lo: 1, hi: 65535 },
    help: Some("<port>"),
    hidden: false,
    act: Some(ActTag::HostNc),
    kids: &[GNode {
        tok: Tok::Line,
        help: Some("[payload]"),
        hidden: false,
        act: Some(ActTag::HostNc),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static NC_HOST: &[GNode] = &[GNode {
    tok: Tok::Ipv4,
    help: Some("<ip>"),
    hidden: false,
    act: None,
    kids: NC_PORT_KIDS,
    edge: Edge::None,
    emits: false,
}];

static NC_KIDS: &[GNode] = &[
    GNode {
        tok: Tok::Lit("-u"),
        help: Some("UDP"),
        hidden: false,
        act: None,
        kids: NC_HOST,
        edge: Edge::None,
        emits: true,
    },
    GNode {
        tok: Tok::Ipv4,
        help: Some("<ip>"),
        hidden: false,
        act: None,
        kids: NC_PORT_KIDS,
        edge: Edge::None,
        emits: false,
    },
];

// `curl <ip[:port]> [port]` — the target parses in bind (ip or ip:port).
static CURL_KIDS: &[GNode] = &[GNode {
    tok: Tok::Word,
    help: Some("<ip[:port]>"),
    hidden: false,
    act: Some(ActTag::HostCurl),
    kids: &[GNode {
        tok: Tok::Range { lo: 1, hi: 65535 },
        help: Some("<port>"),
        hidden: false,
        act: Some(ActTag::HostCurl),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static NSLOOKUP_KIDS: &[GNode] = &[GNode {
    tok: Tok::Word,
    help: Some("<name>"),
    hidden: false,
    act: Some(ActTag::HostNslookup),
    kids: EMPTY_KIDS,
    edge: Edge::None,
    emits: false,
}];

static SERVER_ONOFF: &[GNode] = &[
    gd("on", Some(ActTag::HostServerSvc), EMPTY_KIDS, Some("on")),
    gd("off", Some(ActTag::HostServerSvc), EMPTY_KIDS, Some("off")),
];

// `server dns a <name> <ip>` — `a` is structural (not emitted into argv).
static SERVER_DNS_A: &[GNode] = &[GNode {
    tok: Tok::Word,
    help: Some("<name>"),
    hidden: false,
    act: None,
    kids: &[GNode {
        tok: Tok::Ipv4,
        help: Some("<ip>"),
        hidden: false,
        act: Some(ActTag::HostDnsRecord),
        kids: EMPTY_KIDS,
        edge: Edge::None,
        emits: false,
    }],
    edge: Edge::None,
    emits: false,
}];

static SERVER_DNS_KIDS: &[GNode] = &[
    GNode {
        tok: Tok::Lit("a"),
        help: Some("A record"),
        hidden: false,
        act: None,
        kids: SERVER_DNS_A,
        edge: Edge::None,
        emits: false,
    },
    gd("on", Some(ActTag::HostServerSvc), EMPTY_KIDS, Some("on")),
    gd("off", Some(ActTag::HostServerSvc), EMPTY_KIDS, Some("off")),
];

static SERVER_KIDS: &[GNode] = &[
    gd("dhcp", None, SERVER_ONOFF, Some("DHCP server")),
    GNode {
        tok: Tok::Lit("dns"),
        help: Some("DNS server"),
        hidden: false,
        act: None,
        kids: SERVER_DNS_KIDS,
        edge: Edge::None,
        emits: true,
    },
    gd("http", None, SERVER_ONOFF, Some("HTTP server")),
];

pub static HOST_ROOT: &[GNode] = &[
    GNode {
        tok: Tok::Lit("ip"),
        help: Some("IP"),
        hidden: false,
        act: None,
        kids: HOST_IP_KIDS,
        edge: Edge::None,
        emits: false,
    },
    g(Tok::Lit("ping"), None, PING_KIDS, Some("ping")),
    g(Tok::Lit("trace"), None, TRACEROUTE_KIDS, Some("trace")),
    l("arp", ActTag::Show(ShowKind::Arp), Some("ARP table")),
    GNode {
        tok: Tok::Lit("set"),
        help: Some("set"),
        hidden: false,
        act: None,
        kids: HOST_SET,
        edge: Edge::None,
        emits: false,
    },
    GNode {
        tok: Tok::Lit("show"),
        help: Some("show"),
        hidden: false,
        act: None,
        kids: HOST_SHOW,
        edge: Edge::None,
        emits: false,
    },
    host_extra("traffic", HostExtraKind::Traffic),
    g(Tok::Lit("nc"), None, NC_KIDS, Some("nc")),
    g(Tok::Lit("curl"), None, CURL_KIDS, Some("curl")),
    g(Tok::Lit("server"), None, SERVER_KIDS, Some("server")),
    g(Tok::Lit("nslookup"), None, NSLOOKUP_KIDS, Some("nslookup")),
];

use crate::device::DeviceKind;
use super::types::ModeId;

/// (kind, mode) → root kids. Host is flat: every query lands on HOST_ROOT.
pub fn root(kind: DeviceKind, mode: ModeId) -> &'static [GNode] {
    match kind {
        DeviceKind::Host => HOST_ROOT,
        DeviceKind::Ext => EXEC_ROOT_ROUTER,
        DeviceKind::Router => match mode {
            ModeId::Exec => EXEC_ROOT_ROUTER,
            ModeId::PrivExec => PRIVEXEC_ROOT_ROUTER,
            ModeId::Config => CONFIG_ROOT_ROUTER,
            ModeId::ConfigIf => CONFIG_IF_ROOT_ROUTER,
            ModeId::ConfigVlan => EMPTY_KIDS,
            ModeId::ConfigRouter => CONFIG_ROUTER_ROOT,
        },
        DeviceKind::Switch => match mode {
            ModeId::Exec => EXEC_ROOT_SWITCH,
            ModeId::PrivExec => PRIVEXEC_ROOT_SWITCH,
            ModeId::Config => CONFIG_ROOT_SWITCH,
            ModeId::ConfigIf => CONFIG_IF_ROOT_SWITCH,
            ModeId::ConfigVlan => CONFIG_VLAN_ROOT,
            ModeId::ConfigRouter => EMPTY_KIDS,
        },
    }
}
