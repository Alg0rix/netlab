//! Shared CLI types — the public surface the corpus pins.
//! Serde tags on Op/Intent are fixed: `t` and `kind` respectively.

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

use crate::device::VlanState;

pub type PortId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeId {
    Exec,
    PrivExec,
    Config,
    ConfigIf,
    ConfigVlan,
    ConfigRouter,
}

#[derive(Debug, Default)]
pub struct ModeCtx {
    pub if_port: Option<PortId>,
    pub vlan_id: Option<u16>,
    pub ospf_pid: Option<u16>,
}

#[derive(Debug)]
pub enum Pending {
    YesNo {
        prompt: String,
        default: Option<bool>,
        on_yes: Box<PStep>,
        on_no: Option<Box<PStep>>,
    },
    Field {
        prompt: String,
        default: Option<String>,
        /// Receives the resolved field value (typed text, or the
        /// default on a blank answer) and yields the continuation.
        on_submit: fn(&str) -> PStep,
    },
}

/// Dialog continuation. `Seq` runs several steps under one answer —
/// the reload Save?-yes leg uses it to emit the write before arming the
/// proceed confirm. `Out` emits pre-built ops without touching state.
#[derive(Debug)]
pub enum PStep {
    Act(Act),
    Ask(Pending),
    Out(Vec<Op>),
    Seq(Vec<PStep>),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub start: usize,
}

/// Grammar node token kinds. `Names` is reserved — uninstalled in v1.
#[derive(Debug, Clone, Copy)]
pub enum Tok {
    Lit(&'static str),
    Word,
    Line,
    Range { lo: u32, hi: u32 },
    Ipv4,
    Mask,
    Ipv4Prefix,
    Mac,
    IfName,
    /// Cisco vlan list: "10,20,30-40"
    VlanList,
    Names(NameSet),
}

#[derive(Debug, Clone, Copy)]
pub enum NameSet {
    AclName,
    DhcpPool,
}

/// Edge semantics recorded by the matcher rather than emitted as argv.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    None,
    Neg(NegKind),
    Do,
}

#[derive(Debug)]
pub struct GNode {
    pub tok: Tok,
    pub help: Option<&'static str>,
    pub hidden: bool,
    pub act: Option<ActTag>,
    pub kids: &'static [GNode],
    /// no/default/do edges — record matcher state, never argv.
    pub edge: Edge,
    /// Discriminating Lit: emits Arg::Str(canonical lit). See spec item 5.
    pub emits: bool,
}

pub const EMPTY_KIDS: &[GNode] = &[];

#[derive(Debug, Clone)]
pub enum Arg {
    Str(String),
    Num(u32),
    Ip(Ipv4Addr),
    Mask(Ipv4Addr),
    Prefix(Ipv4Addr, u8),
    Mac([u8; 6]),
    Port(PortId),
    Name(String),
    Vlans(Vec<u16>),
}

#[derive(Debug)]
pub enum Verdict {
    Ok { act: Act, argv: Vec<Arg> },
    Incomplete,
    Ambiguous,
    NoMatch { tok_index: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegKind {
    No,
    Default,
}

#[derive(Debug)]
pub enum Act {
    Mutate(Mutation),
    No(NoTarget),
    Default(NoTarget),
    Session(SessionAct),
    HostExtra(HostExtraKind),
    Show(ShowKind),
    Clear(ClearKind),
    Mode(ModeOp),
    Do(Box<Act>),
    Begin(Pending),
    Intent(Intent),
    Translate { word: String },
}

/// Nullary tag — grammar-table leaf identity, no captured data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActTag {
    Enable,
    Disable,
    Configure,
    End,
    Exit,
    Logout,
    EnterIf,
    EnterVlan,
    EnterRouterOspf,
    HistorySize,
    Hostname,
    Pcname,
    IfAdmin,
    SetIp,
    HostIp,
    HostDhcp,
    IfDescription,
    SwModeAccess,
    SwModeTrunk,
    SwAccessVlan,
    SwTrunkAllowed,
    AddRoute,
    VlanName,
    VlanStateSet,
    AclAceAdd,
    IpAccessGroup,
    OspfNetAdd,
    OspfRouterIdSet,
    NatInsideStaticAdd,
    NatIfInside,
    NatIfOutside,
    ServiceDhcp,
    DhcpExcludedAdd,
    Toggle(crate::config::ToggleKind),
    NegHostname,
    NegIfDescription,
    NegIfIp,
    NegRoute,
    NegVlan,
    NegVlanName,
    NegAcl,
    NegAclAce,
    NegIpAccessGroup,
    NegOspfNet,
    NegOspfRouterId,
    NegOspfProcess,
    NegNatInsideStatic,
    NegNatIfSide,
    NegDhcpExcluded,
    NegSwAccessVlan,
    NegSwTrunkAllowed,
    NegSwMode,
    Show(ShowKind),
    Clear(ClearKind),
    HostExtra(HostExtraKind),
    HostDns,
    HostNslookup,
    HostCurl,
    HostNc,
    HostServerSvc,
    HostDnsRecord,
    WriteMem,
    ReloadAsk,
    CopyRunStartAsk,
    WriteEraseAsk,
    EraseStartupAsk,
    Ping,
    Traceroute,
    PingHiddenC,
}

#[derive(Debug)]
pub enum Mutation {
    Hostname(String),
    Pcname(String),
    SetAdmin { port: PortId, up: bool },
    SetIp { port: PortId, ip: [u8; 4], mask: [u8; 4] },
    HostIp { ip: [u8; 4], mask: [u8; 4], gw: Option<[u8; 4]> },
    HostDhcp,
    /// `ip dns` (None → show) / `ip dns <server>` (Some → set).
    HostDns { ip: Option<[u8; 4]> },
    HostNslookup { name: String },
    HostCurl { ip: [u8; 4], port: u16 },
    HostNc { udp: bool, ip: [u8; 4], port: u16, payload: String },
    HostService { svc: HostSvc, on: bool },
    HostDnsRecord { name: String, ip: [u8; 4] },
    IfDescription { port: PortId, text: String },
    Switchport { port: PortId, spec: SwSpec },
    AddRoute { net: [u8; 4], mask: [u8; 4], nh: [u8; 4], ad: Option<u8> },
    Vlan { id: u16, name: Option<String>, state: Option<VlanState> },
    AclAce { list: u16, ace: AceSpec },
    IpAccessGroup { port: PortId, list: u16, dir_in: bool },
    OspfNet { pid: u16, net: [u8; 4], wild: [u8; 4], area: u32 },
    OspfRouterId { pid: u16, rid: [u8; 4] },
    NatInsideStatic { inside_local: [u8; 4], inside_global: [u8; 4] },
    NatIfRole { port: PortId, side: crate::device::NatRole },
    DhcpService(bool),
    DhcpExcluded { lo: [u8; 4], hi: Option<[u8; 4]> },
    Toggle(crate::config::ToggleKind, bool),
}

#[derive(Debug)]
pub enum SwSpec {
    Mode(bool /* trunk */),
    AccessVlan(u16),
    TrunkAllowed(Vec<u16>),
}

#[derive(Debug)]
pub struct AceSpec {
    pub permit: bool,
    pub proto: u8,
    pub src: AddrSpec,
    pub dst: AddrSpec,
}

#[derive(Debug)]
pub enum AddrSpec {
    Any,
    Host([u8; 4]),
    Net { ip: [u8; 4], wild: [u8; 4] },
}

#[derive(Debug)]
pub enum NoTarget {
    Hostname,
    IfDescription { port: PortId },
    IfIp { port: PortId },
    Route { net: [u8; 4], mask: [u8; 4], nh: Option<[u8; 4]> },
    Vlan { id: u16 },
    VlanName { id: u16 },
    Acl { list: u16 },
    AclAce { list: u16, ace: AceSpec },
    IpAccessGroup { port: PortId, list: u16, dir_in: bool },
    OspfNet { pid: u16, net: [u8; 4], wild: [u8; 4], area: u32 },
    OspfRouterId { pid: u16 },
    OspfProcess { pid: u16 },
    NatInsideStatic { inside_local: [u8; 4] },
    NatIfSide { port: PortId, side: crate::device::NatRole },
    DhcpExcluded { lo: [u8; 4] },
    SwAccessVlan { port: PortId },
    SwTrunkAllowed { port: PortId },
    SwMode { port: PortId },
    /// no state → Active; no shutdown → up; no <toggle> → off
    VlanState { id: u16 },
    IfAdmin { port: PortId },
    ServiceDhcp,
    Toggle(crate::config::ToggleKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAct {
    HistorySize(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostSvc {
    Dhcp,
    Dns,
    Http,
}

impl HostSvc {
    pub fn label(self) -> &'static str {
        match self {
            HostSvc::Dhcp => "dhcp",
            HostSvc::Dns => "dns",
            HostSvc::Http => "http",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostExtraKind {
    Traffic,
}

impl HostExtraKind {
    pub fn tool(self) -> &'static str {
        match self {
            HostExtraKind::Traffic => "traffic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeOp {
    Enable,
    Disable,
    Configure,
    End,
    Exit,
    EnterIf(PortId),
    EnterVlan(u16),
    EnterRouterOspf(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowKind {
    IpIntBrief,
    IpRoute,
    Arp,
    MacAddrTable,
    AccessLists,
    Ospf,
    OspfNeighbor,
    OspfInterface,
    NatTranslations,
    Dhcp,
    Protocols,
    Interfaces,
    Switchport,
    Vlan,
    Status,
    History,
    Version,
    StartupConfig,
    RunningConfig,
    LoggingBuffered,
    HostIp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearKind {
    Arp,
    MacTable,
    NatTranslation,
    OspfProcess,
    IpRouteAll,
    Counters { port: Option<PortId> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t")]
pub enum Op {
    #[serde(rename = "text")]
    Text { cls: Option<OpCls>, text: String },
    #[serde(rename = "clear")]
    Clear,
    #[serde(rename = "bell")]
    Bell,
    #[serde(rename = "echo")]
    Echo { text: String },
    #[serde(rename = "intent")]
    Intent(Intent),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OpCls {
    Dim,
    Err,
    Ok,
    Warn,
    Echo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Intent {
    #[serde(rename = "ping")]
    Ping { target: Ipv4Addr },
    #[serde(rename = "trace")]
    Trace { target: Ipv4Addr },
    #[serde(rename = "rename")]
    Rename { name: String },
    #[serde(rename = "nvram-write")]
    NvramWrite { text: String },
    #[serde(rename = "nvram-erase")]
    NvramErase,
    #[serde(rename = "reload")]
    Reload,
}

impl Op {
    pub fn text(text: impl Into<String>) -> Op {
        Op::Text { cls: None, text: text.into() }
    }
    pub fn err(text: impl Into<String>) -> Op {
        Op::Text { cls: Some(OpCls::Err), text: text.into() }
    }
    pub fn dim(text: impl Into<String>) -> Op {
        Op::Text { cls: Some(OpCls::Dim), text: text.into() }
    }
    pub fn ok(text: impl Into<String>) -> Op {
        Op::Text { cls: Some(OpCls::Ok), text: text.into() }
    }
}

#[derive(Debug)]
pub struct CliResult {
    pub prompt: String,
    pub out: Vec<Op>,
    pub mutated: bool,
    pub pending: Option<PendingView>,
}

#[derive(Debug, Clone)]
pub struct PendingView {
    /// Verbatim, including the trailing space.
    pub prompt: String,
}

#[derive(Debug)]
pub struct HelpRow {
    pub word: String,
    pub help: Option<String>,
}

#[derive(Debug)]
pub enum HelpResult {
    Rows { rows: Vec<HelpRow>, prompt: String },
    Inline { inline: Vec<String>, prompt: String },
    Err { index: usize, prompt: String },
}

#[derive(Debug)]
pub enum CompleteResult {
    Line { line: String },
    List { list: Vec<String> },
    Err { index: usize },
}

#[derive(Debug)]
pub struct ScriptResult {
    pub out: Vec<Op>,
    pub mutated: bool,
    pub errors: Vec<ScriptError>,
}

#[derive(Debug)]
pub struct ScriptError {
    pub lineno: u32,
    pub msg: String,
}
