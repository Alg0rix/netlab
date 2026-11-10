use serde::{Deserialize, Serialize};

use crate::{ip_str, mac_str};

pub const BROADCAST_MAC: u64 = 0xFF_FF_FF_FF_FF_FF;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Payload {
    IcmpEchoRequest,
    IcmpEchoReply,
    IcmpTimeExceeded,
    ArpRequest {
        target_ip: [u8; 4],
    },
    ArpReply {
        target_ip: [u8; 4],
    },
    /// Real Ethernet payload we don't model (kept verbatim in `Frame::raw`).
    Raw {
        ethertype: u16,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub src_mac: u64,
    pub dst_mac: u64,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
    pub ttl: u8,
    /// 802.1Q VID. 0 means the frame is untagged on the wire.
    #[serde(default)]
    pub vlan: u16,
    pub payload: Payload,
    /// Original wire bytes when this came from an emulated (real-Ethernet) node.
    /// Preserved across forwards so checksums/ids survive; dropped when a
    /// device builds a fresh response.
    #[serde(default)]
    pub raw: Option<Vec<u8>>,
    /// ICMP echo identifier, sequence and data (everything after the 4-byte
    /// ICMP header). A reply must echo these or real IP stacks drop it.
    #[serde(default)]
    pub icmp_echo: Option<Vec<u8>>,
}

impl Frame {
    /// An IPv4 frame with no wire bytes yet; `emit_ip` fills MACs.
    pub fn ip(src: [u8; 4], dst: [u8; 4], ttl: u8, payload: Payload) -> Self {
        Self {
            src_mac: 0,
            dst_mac: 0,
            src_ip: src,
            dst_ip: dst,
            ttl,
            vlan: 0,
            payload,
            raw: None,
            icmp_echo: None,
        }
    }

    /// An IPv4 frame whose L4 bytes already exist as wire format.
    pub fn raw_ip(src: [u8; 4], dst: [u8; 4], ttl: u8, raw: Vec<u8>) -> Self {
        Self {
            src_mac: 0,
            dst_mac: 0,
            src_ip: src,
            dst_ip: dst,
            ttl,
            vlan: 0,
            payload: Payload::Raw { ethertype: 0x0800 },
            raw: Some(raw),
            icmp_echo: None,
        }
    }

    /// A frame addressed to a local broadcast (L2 or limited L3).
    pub fn is_broadcast(&self) -> bool {
        self.dst_mac == BROADCAST_MAC || self.dst_ip == [255, 255, 255, 255]
    }

    /// Short label for the canvas dot + event log.
    pub fn label(&self) -> String {
        match self.payload {
            Payload::IcmpEchoRequest => {
                format!(
                    "ICMP echo {} > {}",
                    ip_str(self.src_ip),
                    ip_str(self.dst_ip)
                )
            }
            Payload::IcmpEchoReply => {
                format!(
                    "ICMP reply {} > {}",
                    ip_str(self.src_ip),
                    ip_str(self.dst_ip)
                )
            }
            Payload::IcmpTimeExceeded => {
                format!(
                    "ICMP time exceeded {} > {}",
                    ip_str(self.src_ip),
                    ip_str(self.dst_ip)
                )
            }
            Payload::ArpRequest { target_ip } => {
                format!("ARP who-has {}", ip_str(target_ip))
            }
            Payload::ArpReply { target_ip } => {
                format!("ARP reply {} is-at", ip_str(target_ip))
            }
            Payload::Raw { ethertype } => {
                if let Some(r) = &self.raw {
                    format!(
                        "{} {} > {}",
                        crate::eth::raw_label(r),
                        mac_str(self.src_mac),
                        mac_str(self.dst_mac)
                    )
                } else {
                    format!("frame ethertype 0x{:04x}", ethertype)
                }
            }
        }
    }

    /// Dot colour key for the UI.
    pub fn color_key(&self) -> &'static str {
        match self.payload {
            Payload::IcmpEchoRequest | Payload::IcmpEchoReply | Payload::IcmpTimeExceeded => "icmp",
            Payload::ArpRequest { .. } | Payload::ArpReply { .. } => "arp",
            Payload::Raw { .. } => self
                .raw
                .as_deref()
                .map(crate::eth::l4_color)
                .unwrap_or("raw"),
        }
    }
}
