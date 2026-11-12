pub mod cli;
pub mod config;
pub mod device;
mod eth;
mod events;
mod frame;
mod link;
mod ospf;
mod services;
mod topology;

pub use cli::{Cli, CliResult, Intent, Op};
pub use device::{Device, DeviceId, DeviceKind};
pub use events::SimEvent;
pub use link::LinkId;
pub use topology::{AceArgs, Capture, Snapshot, Topology};

pub(crate) fn ip_str(ip: [u8; 4]) -> String {
    format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

pub(crate) fn mac_str(mac: u64) -> String {
    let b = mac.to_be_bytes();
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        b[2], b[3], b[4], b[5], b[6], b[7]
    )
}

pub(crate) fn parse_ip(s: &str) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let parts: Vec<&str> = s.trim().split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    for (i, p) in parts.iter().enumerate() {
        out[i] = p.parse().ok()?;
    }
    Some(out)
}

pub(crate) fn mask_bits(mask: [u8; 4]) -> u32 {
    mask.iter().map(|o| o.count_ones()).sum()
}

pub(crate) fn net_of(ip: [u8; 4], mask: [u8; 4]) -> [u8; 4] {
    [
        ip[0] & mask[0],
        ip[1] & mask[1],
        ip[2] & mask[2],
        ip[3] & mask[3],
    ]
}
