use crate::DeviceId;
use crate::Frame;

pub type LinkId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub device: DeviceId,
    pub port: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSpeed {
    M10,
    M100,
    G1,
    G10,
    Wan,
}

impl LinkSpeed {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkSpeed::M10 => "10M",
            LinkSpeed::M100 => "100M",
            LinkSpeed::G1 => "1G",
            LinkSpeed::G10 => "10G",
            LinkSpeed::Wan => "WAN",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "10M" | "10" => LinkSpeed::M10,
            "100M" | "100" => LinkSpeed::M100,
            "10G" => LinkSpeed::G10,
            "WAN" | "SERIAL" | "T1" => LinkSpeed::Wan,
            _ => LinkSpeed::G1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InFlight {
    pub frame: Frame,
    /// 0..budget; a frame with progress == budget is delivered this tick.
    pub progress: u32,
    /// Per-frame delay after jitter and serialization.
    pub budget: u32,
    pub to: Endpoint,
}

#[derive(Debug)]
pub struct Link {
    pub id: LinkId,
    pub a: Endpoint,
    pub b: Endpoint,
    /// ticks a frame needs to cross the wire (1 tick = 1 ms in Realtime)
    pub delay: u32,
    /// extra delay, uniform in ±jitter ticks
    pub jitter: u32,
    /// percent of frames dropped, 0–100
    pub loss: u8,
    /// drop frames longer than this many bytes; 0 = no limit
    pub mtu: u16,
    pub speed: LinkSpeed,
    /// false = cable unplugged: frames put on it are dropped
    pub up: bool,
    /// copy every frame put on this link into the capture buffer
    pub capture: bool,
    pub in_flight: Vec<InFlight>,
}

impl Link {
    pub fn other(&self, ep: Endpoint) -> Endpoint {
        if ep == self.a {
            self.b
        } else {
            self.a
        }
    }
}
