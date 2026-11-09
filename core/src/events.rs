use serde::Serialize;

use crate::DeviceId;
use crate::LinkId;

#[derive(Debug, Clone, Serialize)]
pub struct SimEvent {
    pub tick: u64,
    pub device: Option<DeviceId>,
    pub link: Option<LinkId>,
    /// "tx" | "learn" | "flood" | "forward" | "arp_req" | "arp_rep"
    /// | "echo_req" | "echo_rep" | "deliver" | "drop"
    pub kind: &'static str,
    pub msg: String,
}

impl SimEvent {
    pub fn new(
        tick: u64,
        device: Option<DeviceId>,
        link: Option<LinkId>,
        kind: &'static str,
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
