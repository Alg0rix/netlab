use crate::frame::Frame;
use crate::DeviceId;

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

    fn bit_rate(self) -> u64 {
        match self {
            LinkSpeed::M10 => 10_000_000,
            LinkSpeed::M100 => 100_000_000,
            LinkSpeed::Wan => 1_544_000,
            LinkSpeed::G1 | LinkSpeed::G10 => 0,
        }
    }

    /// Serialization delay in ticks (1 tick = 1 ms in Realtime) for `len`
    /// wire bytes. Instant at 1G and up.
    pub fn serial_ticks(self, len: usize) -> u32 {
        let bps = self.bit_rate();
        if len == 0 || bps == 0 {
            return 0;
        }
        ((len as u64 * 8 * 1000) / bps).max(1) as u32
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
