use std::collections::HashMap;

use serde::Serialize;

use crate::device::{Device, DeviceKind};
use crate::events::SimEvent;
use crate::frame::Frame;
use crate::link::{Endpoint, InFlight, Link, LinkSpeed};
use crate::{DeviceId, LinkId};

mod actions;
mod config;
mod snapshot;

pub use config::AceArgs;
pub use snapshot::Snapshot;

const LINK_DELAY: u32 = 4;
const BASE_MAC: u64 = 0x02_00_00_00_00_00;
#[derive(Serialize)]
pub struct Capture {
    pub link: LinkId,
    pub tick: u64,
    pub from: DeviceId,
    pub bytes: Vec<u8>,
}

pub struct Topology {
    pub captures: Vec<Capture>,
    pub devices: HashMap<DeviceId, Device>,
    pub links: HashMap<LinkId, Link>,
    next_device: DeviceId,
    next_link: LinkId,
    next_mac: u64,
    pub tick: u64,
    pub events: Vec<SimEvent>,
    rng: u32,
}

impl Default for Topology {
    fn default() -> Self {
        Self::new()
    }
}

impl Topology {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            links: HashMap::new(),
            captures: Vec::new(),
            next_device: 1,
            next_link: 1,
            next_mac: BASE_MAC,
            tick: 0,
            events: Vec::new(),
            rng: 0xA5A5_1234,
        }
    }

    fn rng(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        if x == 0 {
            x = 0xA5A5_1234;
        }
        self.rng = x;
        x
    }

    fn alloc_mac(&mut self) -> u64 {
        self.next_mac += 1;
        self.next_mac
    }

    pub fn add_device(&mut self, kind: DeviceKind, name: String, x: f64, y: f64) -> DeviceId {
        let id = self.next_device;
        self.next_device += 1;
        let n_ports = match kind {
            DeviceKind::Host => 8,
            DeviceKind::Switch => 16,
            DeviceKind::Router => 8,
            DeviceKind::Ext => 16,
        };
        let macs = (0..n_ports).map(|_| self.alloc_mac()).collect();
        self.devices
            .insert(id, Device::new(id, name, kind, x, y, macs));
        id
    }

    pub fn remove_device(&mut self, id: DeviceId) {
        self.devices.remove(&id);
        let dead: Vec<LinkId> = self
            .links
            .values()
            .filter(|l| l.a.device == id || l.b.device == id)
            .map(|l| l.id)
            .collect();
        for l in dead {
            self.remove_link(l);
        }
    }

    pub fn move_device(&mut self, id: DeviceId, x: f64, y: f64) {
        if let Some(d) = self.devices.get_mut(&id) {
            d.x = x;
            d.y = y;
        }
    }

    /// Connect two devices on their first free ports.
    pub fn add_link(&mut self, a: DeviceId, b: DeviceId) -> Option<LinkId> {
        let pa = self.devices.get(&a)?.free_port()?;
        let pb = self.devices.get(&b)?.free_port()?;
        self.add_link_ports(a, pa, b, pb)
    }

    /// Connect specific ports. Fails if either port is missing or in use,
    /// or if both ends are the same port.
    pub fn add_link_ports(
        &mut self,
        a: DeviceId,
        pa: usize,
        b: DeviceId,
        pb: usize,
    ) -> Option<LinkId> {
        let free = |d: DeviceId, p: usize| {
            self.devices
                .get(&d)
                .and_then(|d| d.ports.get(p))
                .map(|p| p.link.is_none())
                == Some(true)
        };
        if !free(a, pa) || !free(b, pb) || (a == b && pa == pb) {
            return None;
        }
        let id = self.next_link;
        self.next_link += 1;
        let link = Link {
            id,
            a: Endpoint {
                device: a,
                port: pa,
            },
            b: Endpoint {
                device: b,
                port: pb,
            },
            delay: LINK_DELAY,
            jitter: 0,
            loss: 0,
            mtu: 0,
            speed: LinkSpeed::G1,
            up: true,
            capture: false,
            in_flight: Vec::new(),
        };
        self.devices.get_mut(&a)?.ports[pa].link = Some(id);
        self.devices.get_mut(&b)?.ports[pb].link = Some(id);
        self.links.insert(id, link);
        self.refresh_link_state(a, pa);
        self.refresh_link_state(b, pb);
        Some(id)
    }

    pub fn remove_link(&mut self, id: LinkId) {
        if let Some(l) = self.links.remove(&id) {
            for ep in [l.a, l.b] {
                if let Some(d) = self.devices.get_mut(&ep.device) {
                    d.ports[ep.port].link = None;
                }
                self.refresh_link_state(ep.device, ep.port);
            }
        }
    }

    /// Recompute carrier on one port from its link, then run the
    /// effective-state machinery (C1/C2 pair, OSPF port-down, connected
    /// route synthesis) when admin×carrier flips.
    fn refresh_link_state(&mut self, dev_id: DeviceId, port: usize) {
        let carrier = self
            .devices
            .get(&dev_id)
            .and_then(|d| d.ports.get(port))
            .and_then(|p| p.link)
            .and_then(|l| self.links.get(&l))
            .map(|l| l.up)
            .unwrap_or(false);
        let tick = self.tick;
        if let Some(d) = self.devices.get_mut(&dev_id) {
            let Some(p) = d.ports.get_mut(port) else {
                return;
            };
            let was = p.admin_up && p.link_up;
            p.link_up = carrier;
            let now = p.admin_up && p.link_up;
            crate::config::effective_changed(d, port, was, now, tick, &mut self.events);
        }
    }

    pub fn rename_device(&mut self, id: DeviceId, name: String) {
        if let Some(d) = self.devices.get_mut(&id) {
            d.name = name;
        }
    }

    /// Plug / unplug a cable without removing it. Unplugging drops whatever
    /// is in flight on it.
    pub fn set_link_up(&mut self, id: LinkId, up: bool) {
        let eps = if let Some(l) = self.links.get_mut(&id) {
            l.up = up;
            if !up {
                l.in_flight.clear();
            }
            Some([l.a, l.b])
        } else {
            None
        };
        if let Some([a, b]) = eps {
            self.refresh_link_state(a.device, a.port);
            self.refresh_link_state(b.device, b.port);
        }
    }

    pub fn set_capture(&mut self, id: LinkId, on: bool) {
        if let Some(l) = self.links.get_mut(&id) {
            l.capture = on;
        }
    }

    pub fn drain_captures(&mut self) -> Vec<Capture> {
        std::mem::take(&mut self.captures)
    }

    pub fn step(&mut self) {
        self.tick += 1;
        let tick = self.tick;
        let ids: Vec<DeviceId> = self.devices.keys().copied().collect();
        for id in ids {
            if let Some(d) = self.devices.get_mut(&id) {
                crate::services::poll(d, tick, &mut self.events);
                crate::ospf::tick(d, tick, &mut self.events);
                d.poll_probe(tick, &mut self.events);
            }
        }

        let mut arrivals: Vec<(Endpoint, Frame, LinkId)> = Vec::new();
        for link in self.links.values_mut() {
            for f in &mut link.in_flight {
                f.progress += 1;
            }
            let mut rest = Vec::new();
            for f in link.in_flight.drain(..) {
                if f.progress >= f.budget {
                    arrivals.push((f.to, f.frame, link.id));
                } else {
                    rest.push(f);
                }
            }
            link.in_flight = rest;
        }

        // deliver
        for (to, frame, _link_id) in arrivals {
            if let Some(d) = self.devices.get_mut(&to.device) {
                if d.powered && to.port < d.ports.len() {
                    d.receive(frame, to.port, self.tick, &mut self.events);
                }
            }
        }

        // put outbox frames onto the wire
        let mut tx: Vec<(LinkId, Endpoint, Frame)> = Vec::new();
        for d in self.devices.values_mut() {
            for (port, frame) in d.outbox.drain(..) {
                let Some(p) = d.ports.get_mut(port) else {
                    continue;
                };
                if !(p.admin_up && p.link_up) {
                    continue;
                }
                p.stats.tx += 1;
                if let Some(l) = p.link {
                    tx.push((l, Endpoint { device: d.id, port }, frame));
                }
            }
        }
        for (link_id, from, frame) in tx {
            self.enqueue(link_id, from, frame);
        }
    }

    /// Put `frame` on `link_id` travelling away from `from`, applying loss, MTU and jitter.
    fn enqueue(&mut self, link_id: LinkId, from: Endpoint, frame: Frame) {
        let Some(link) = self.links.get(&link_id) else {
            return;
        };
        let up = link.up;
        let mtu = link.mtu;
        let loss = link.loss;
        let jitter = link.jitter;
        let delay = link.delay.max(1);
        let speed = link.speed;
        let capture = link.capture;
        let to = link.other(from);
        if !up {
            return;
        }
        let bytes = frame.raw.clone().or_else(|| crate::eth::encode(&frame));
        let len = bytes.as_ref().map(|b| b.len()).unwrap_or(0);
        if mtu > 0 && len as u16 > mtu {
            return;
        }
        if loss > 0 && (self.rng() % 100) < loss as u32 {
            return;
        }
        let swing = if jitter == 0 {
            0
        } else {
            (self.rng() % (jitter * 2 + 1)) as i32 - jitter as i32
        };
        let budget = (delay as i32 + swing).max(1) as u32 + speed.serial_ticks(len);
        if capture {
            if let Some(bytes) = bytes.clone() {
                self.captures.push(Capture {
                    link: link_id,
                    tick: self.tick,
                    from: from.device,
                    bytes,
                });
            }
        }
        if let Some(link) = self.links.get_mut(&link_id) {
            link.in_flight.push(InFlight {
                frame,
                progress: 0,
                budget,
                to,
            });
        }
    }

    /// Re-inject a captured (or crafted) Ethernet frame onto a link, from `from` toward the peer.
    pub fn inject_link(&mut self, link: LinkId, from: DeviceId, bytes: &[u8]) -> bool {
        if bytes.len() < 14 {
            return false;
        }
        let Some(l) = self.links.get(&link) else {
            return false;
        };
        let from_ep = if l.a.device == from {
            l.a
        } else if l.b.device == from {
            l.b
        } else {
            return false;
        };
        let Some(frame) = crate::eth::decode(bytes) else { return false };
        self.enqueue(link, from_ep, frame);
        true
    }

    pub fn set_link_impair(
        &mut self,
        id: LinkId,
        delay: u32,
        jitter: u32,
        loss: u8,
        mtu: u16,
        speed: &str,
    ) -> bool {
        let Some(l) = self.links.get_mut(&id) else {
            return false;
        };
        l.delay = delay.max(1);
        l.jitter = jitter;
        l.loss = loss.min(100);
        l.mtu = mtu;
        l.speed = LinkSpeed::parse(speed);
        true
    }

    /// Inject real wire bytes from an emulated node into ext device `dev`
    /// port `port`; decoded frame is queued onto that port's link.
    pub fn ext_inject(&mut self, dev: DeviceId, port: usize, bytes: &[u8]) {
        if let Some(d) = self.devices.get_mut(&dev) {
            d.ext_inject(port, bytes);
        }
    }

    /// Drain encoded wire bytes destined for emulated nodes.
    /// Returns (device_id, port, bytes).
    pub fn drain_ext(&mut self) -> Vec<(DeviceId, usize, Vec<u8>)> {
        let mut out = Vec::new();
        for d in self.devices.values_mut() {
            let id = d.id;
            for (port, bytes) in d.ext_out.drain(..) {
                out.push((id, port, bytes));
            }
        }
        out
    }

    pub fn drain_events(&mut self) -> Vec<SimEvent> {
        std::mem::take(&mut self.events)
    }
}
