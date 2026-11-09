use serde::{Deserialize, Serialize};

use crate::{ip_str, mac_str};

pub const BROADCAST_MAC: u64 = 0xFF_FF_FF_FF_FF_FF;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Payload {
    IcmpEchoRequest,
    IcmpEchoReply,
    ArpRequest { target_ip: [u8; 4] },
    ArpReply { target_ip: [u8; 4] },
    /// Real Ethernet payload we don't model (kept verbatim in `Frame::raw`).
    Raw { ethertype: u16 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub src_mac: u64,
    pub dst_mac: u64,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
    pub ttl: u8,
    pub payload: Payload,
    /// Original wire bytes when this came from an emulated (real-Ethernet) node.
    /// Preserved across forwards so checksums/ids survive; dropped when a
    /// device builds a fresh response.
    #[serde(default)]
    pub raw: Option<Vec<u8>>,
}

impl Frame {
    /// Short label for the canvas dot + event log.
    pub fn label(&self) -> String {
        match self.payload {
            Payload::IcmpEchoRequest => {
                format!("ICMP echo {} > {}", ip_str(self.src_ip), ip_str(self.dst_ip))
            }
            Payload::IcmpEchoReply => {
                format!("ICMP reply {} > {}", ip_str(self.src_ip), ip_str(self.dst_ip))
            }
            Payload::ArpRequest { target_ip } => {
                format!("ARP who-has {}", ip_str(target_ip))
            }
            Payload::ArpReply { target_ip } => {
                format!("ARP reply {} is-at", ip_str(target_ip))
            }
            Payload::Raw { ethertype } => {
                if let Some(r) = &self.raw {
                    format!("{} {} > {}", crate::eth::raw_label(r), mac_str(self.src_mac), mac_str(self.dst_mac))
                } else {
                    format!("frame ethertype 0x{:04x}", ethertype)
                }
            }
        }
    }

    /// Dot colour key for the UI.
    pub fn color_key(&self) -> &'static str {
        match self.payload {
            Payload::IcmpEchoRequest | Payload::IcmpEchoReply => "icmp",
            Payload::ArpRequest { .. } | Payload::ArpReply { .. } => "arp",
            Payload::Raw { .. } => "raw",
        }
    }
}
