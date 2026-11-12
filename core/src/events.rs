use serde::Serialize;

use crate::DeviceId;
use crate::LinkId;

/// Frozen event-kind vocabulary — the only channel labels the adapter sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EvKind {
    /// C1/C2 interface state pairs.
    Link,
    /// C3–C5 ping probe glyphs, headers and summaries.
    Ping,
    /// C6–C8 traceroute header and hop rows.
    Trace,
    /// C9 DHCP assignment/renewal notes.
    Dhcp,
    /// C10 %-style notices (OSPF ADJCHG, service notes).
    Syslog,
}

/// Canvas-side flow signal — one per link enqueue. Drained through
/// `NetLab::drain_flow`, never rendered to a console: a realtime transit is
/// shorter than the host's snapshot cadence, so `l.frames` sampling alone
/// misses most hops.
#[derive(Debug, Clone, Serialize)]
pub struct FlowEvent {
    pub tick: u64,
    pub link: LinkId,
    pub from: DeviceId,
    pub proto: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimEvent {
    pub tick: u64,
    pub device: DeviceId,
    pub link: Option<LinkId>,
    pub kind: EvKind,
    pub msg: String,
}

impl SimEvent {
    pub fn new(
        tick: u64,
        device: DeviceId,
        link: Option<LinkId>,
        kind: EvKind,
        msg: String,
    ) -> Self {
        Self {
            tick,
            device,
            link,
            kind,
            msg,
        }
    }
}
