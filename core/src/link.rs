use crate::DeviceId;
use crate::Frame;

pub type LinkId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub device: DeviceId,
    pub port: usize,
}

#[derive(Debug, Clone)]
pub struct InFlight {
    pub frame: Frame,
    /// 0..delay; a frame with progress == delay is delivered this tick.
    pub progress: u32,
    pub to: Endpoint,
}

#[derive(Debug)]
pub struct Link {
    pub id: LinkId,
    pub a: Endpoint,
    pub b: Endpoint,
    /// ticks a frame needs to cross the wire
    pub delay: u32,
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
