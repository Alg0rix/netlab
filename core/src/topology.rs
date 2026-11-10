use std::collections::HashMap;

use serde::Serialize;

use crate::device::{Device, DeviceKind, Route};
use crate::events::SimEvent;
use crate::frame::{Frame, Payload};
use crate::link::{Endpoint, InFlight, Link};
use crate::{ip_str, mac_str, parse_ip, DeviceId, LinkId};

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
}

#[derive(Serialize)]
pub struct Snapshot {
    pub tick: u64,
    pub devices: Vec<DeviceSnap>,
    pub links: Vec<LinkSnap>,
}

#[derive(Serialize)]
pub struct DeviceSnap {
    pub id: DeviceId,
    pub name: String,
    pub kind: &'static str,
    pub x: f64,
    pub y: f64,
    pub ports: Vec<PortSnap>,
    pub routes: Vec<RouteSnap>,
    pub powered: bool,
    /// (ip, mac)
    pub arp: Vec<(String, String)>,
    /// (mac, port) — switches
    pub macs: Vec<(String, usize)>,
}

#[derive(Serialize)]
pub struct RouteSnap {
    pub prefix: String,
    pub mask: String,
    pub next_hop: String,
    pub out_port: usize,
}

#[derive(Serialize)]
pub struct PortSnap {
    pub ip: Option<String>,
    pub mask: Option<String>,
    pub mac: String,
    pub link: Option<LinkId>,
}

#[derive(Serialize)]
pub struct LinkSnap {
    pub id: LinkId,
    pub a: DeviceId,
    pub b: DeviceId,
    pub a_port: usize,
    pub b_port: usize,
    pub up: bool,
    pub capture: bool,
    pub delay: u32,
    pub frames: Vec<FrameSnap>,
}

#[derive(Serialize)]
pub struct FrameSnap {
    /// 0.0 at `a`, 1.0 at `b`
    pub t: f32,
    /// +1 travelling a→b, -1 travelling b→a
    pub dir: i8,
    pub color: &'static str,
    pub label: String,
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
        }
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
            DeviceKind::Ext => 8,
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
    pub fn add_link_ports(&mut self, a: DeviceId, pa: usize, b: DeviceId, pb: usize) -> Option<LinkId> {
        let free = |d: DeviceId, p: usize| {
            self.devices.get(&d).and_then(|d| d.ports.get(p)).map(|p| p.link.is_none()) == Some(true)
        };
        if !free(a, pa) || !free(b, pb) || (a == b && pa == pb) {
            return None;
        }
        let id = self.next_link;
        self.next_link += 1;
        let link = Link {
            id,
            a: Endpoint { device: a, port: pa },
            b: Endpoint { device: b, port: pb },
            delay: LINK_DELAY,
            up: true,
            capture: false,
            in_flight: Vec::new(),
        };
        self.devices.get_mut(&a)?.ports[pa].link = Some(id);
        self.devices.get_mut(&b)?.ports[pb].link = Some(id);
        self.links.insert(id, link);
        Some(id)
    }

    pub fn remove_link(&mut self, id: LinkId) {
        if let Some(l) = self.links.remove(&id) {
            for ep in [l.a, l.b] {
                if let Some(d) = self.devices.get_mut(&ep.device) {
                    d.ports[ep.port].link = None;
                }
            }
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
        if let Some(l) = self.links.get_mut(&id) {
            l.up = up;
            if !up {
                l.in_flight.clear();
            }
        }
    }

    pub fn set_power(&mut self, id: DeviceId, on: bool) {
        if let Some(d) = self.devices.get_mut(&id) {
            d.powered = on;
            if !on {
                d.outbox.clear();
                d.pending.clear();
                d.arp_cache.clear();
                d.mac_table.clear();
            }
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

    pub fn clear_ip(&mut self, dev: DeviceId, port: usize) {
        if let Some(p) = self.devices.get_mut(&dev).and_then(|d| d.ports.get_mut(port)) {
            p.ip = None;
        }
    }

    /// Remove the n-th static route (index into the snapshot's `routes`).
    pub fn remove_route(&mut self, dev: DeviceId, idx: usize) {
        if let Some(d) = self.devices.get_mut(&dev) {
            if idx < d.routes.len() {
                d.routes.remove(idx);
            }
        }
    }

    pub fn set_ip(&mut self, dev: DeviceId, port: usize, ip: &str, mask: &str) -> bool {
        let (Some(ip), Some(mask)) = (parse_ip(ip), parse_ip(mask)) else {
            return false;
        };
        if let Some(d) = self.devices.get_mut(&dev) {
            if let Some(p) = d.ports.get_mut(port) {
                p.ip = Some((ip, mask));
                return true;
            }
        }
        false
    }

    /// Convenience: default route via `gw` on the matching connected port (or port 0).
    pub fn set_gateway(&mut self, dev: DeviceId, gw: &str) -> bool {
        let Some(gw_ip) = parse_ip(gw) else { return false };
        if let Some(d) = self.devices.get_mut(&dev) {
            let out_port = d
                .ports
                .iter()
                .enumerate()
                .find_map(|(i, p)| {
                    p.ip.and_then(|(addr, mask)| {
                        (crate::net_of(addr, mask) == crate::net_of(gw_ip, mask)).then_some(i)
                    })
                })
                .unwrap_or(0);
            d.routes.retain(|r| r.prefix != [0, 0, 0, 0]);
            d.routes.push(Route {
                prefix: [0, 0, 0, 0],
                mask: [0, 0, 0, 0],
                next_hop: gw_ip,
                out_port,
                connected: false,
            });
            return true;
        }
        false
    }

    pub fn add_route(
        &mut self,
        dev: DeviceId,
        prefix: &str,
        mask: &str,
        next_hop: &str,
        out_port: usize,
    ) -> bool {
        let (Some(prefix), Some(mask), Some(next_hop)) =
            (parse_ip(prefix), parse_ip(mask), parse_ip(next_hop))
        else {
            return false;
        };
        if let Some(d) = self.devices.get_mut(&dev) {
            d.routes.push(Route {
                prefix,
                mask,
                next_hop,
                out_port,
                connected: false,
            });
            return true;
        }
        false
    }

    /// Start an ICMP echo from `src` (a host or router) to `dst_ip`.
    pub fn ping(&mut self, src: DeviceId, dst_ip: &str) -> bool {
        let Some(dst) = parse_ip(dst_ip) else { return false };
        let Some(d) = self.devices.get_mut(&src) else { return false };
        if !d.powered {
            return false;
        }
        let frame = Frame {
            src_mac: 0,
            dst_mac: 0,
            src_ip: [0, 0, 0, 0],
            dst_ip: dst,
            ttl: 64,
            payload: Payload::IcmpEchoRequest,
                raw: None,
                icmp_echo: None,
        };
        d.emit_ip(frame, self.tick, &mut self.events);
        true
    }

    pub fn step(&mut self) {
        self.tick += 1;

        // advance and collect arrivals
        let mut arrivals: Vec<(Endpoint, Frame, LinkId)> = Vec::new();
        for link in self.links.values_mut() {
            for f in &mut link.in_flight {
                f.progress += 1;
            }
            let mut rest = Vec::new();
            for f in link.in_flight.drain(..) {
                if f.progress >= link.delay {
                    arrivals.push((f.to, f.frame, link.id));
                } else {
                    rest.push(f);
                }
            }
            link.in_flight = rest;
        }

        // deliver (devices may push to outbox)
        for (to, frame, link_id) in arrivals {
            if let Some(d) = self.devices.get_mut(&to.device) {
                if !d.powered {
                    self.events.push(SimEvent::new(
                        self.tick,
                        Some(d.id),
                        Some(link_id),
                        "drop",
                        format!("{} is stopped \u{2014} {} lost", d.name, frame.label()),
                    ));
                } else if to.port < d.ports.len() {
                    d.receive(frame, to.port, self.tick, &mut self.events);
                }
            }
        }

        // put outbox frames onto the wire
        let mut tx: Vec<(LinkId, Endpoint, Frame)> = Vec::new();
        for d in self.devices.values_mut() {
            for (port, frame) in d.outbox.drain(..) {
                match d.ports.get(port).and_then(|p| p.link) {
                    Some(l) => tx.push((l, Endpoint { device: d.id, port }, frame)),
                    None => self.events.push(SimEvent::new(
                        self.tick,
                        Some(d.id),
                        None,
                        "drop",
                        format!("{}: port {} not connected", d.name, port),
                    )),
                }
            }
        }
        for (link_id, from, frame) in tx {
            if let Some(link) = self.links.get_mut(&link_id) {
                if !link.up {
                    let name = |id| self.devices.get(&id).map(|d: &Device| d.name.clone()).unwrap_or_default();
                    let to = link.other(from);
                    let msg = format!(
                        "cable {} \u{2194} {} is unplugged \u{2014} {} lost",
                        name(from.device),
                        name(to.device),
                        frame.label()
                    );
                    self.events.push(SimEvent::new(self.tick, Some(from.device), Some(link_id), "drop", msg));
                    continue;
                }
                let to = link.other(from);
                if link.capture {
                    let bytes = frame.raw.clone().or_else(|| crate::eth::encode(&frame));
                    if let Some(bytes) = bytes {
                        self.captures.push(Capture { link: link_id, tick: self.tick, from: from.device, bytes });
                    }
                }
                link.in_flight.push(InFlight {
                    frame,
                    progress: 0,
                    to,
                });
            }
        }
    }

    /// Inject real wire bytes from an emulated node into ext device `dev`
    /// port `port`; decoded frame is queued onto that port's link.
    pub fn ext_inject(&mut self, dev: DeviceId, port: usize, bytes: &[u8]) {
        if let Some(d) = self.devices.get_mut(&dev) {
            d.ext_inject(port, bytes, self.tick, &mut self.events);
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

    pub fn snapshot(&self) -> Snapshot {
        let devices = self
            .devices
            .values()
            .map(|d| DeviceSnap {
                id: d.id,
                name: d.name.clone(),
                kind: match d.kind {
                    DeviceKind::Host => "host",
                    DeviceKind::Switch => "switch",
                    DeviceKind::Router => "router",
                    DeviceKind::Ext => "ext",
                },
                x: d.x,
                y: d.y,
                ports: d
                    .ports
                    .iter()
                    .map(|p| PortSnap {
                        ip: p.ip.map(|(a, _)| ip_str(a)),
                        mask: p.ip.map(|(_, m)| ip_str(m)),
                        mac: mac_str(p.mac),
                        link: p.link,
                    })
                    .collect(),
                routes: d
                    .routes
                    .iter()
                    .map(|r| RouteSnap {
                        prefix: ip_str(r.prefix),
                        mask: ip_str(r.mask),
                        next_hop: ip_str(r.next_hop),
                        out_port: r.out_port,
                    })
                    .collect(),
                powered: d.powered,
                arp: d.arp_cache.iter().map(|(ip, mac)| (ip_str(*ip), mac_str(*mac))).collect(),
                macs: d.mac_table.iter().map(|(mac, p)| (mac_str(*mac), *p)).collect(),
            })
            .collect();
        let links = self
            .links
            .values()
            .map(|l| LinkSnap {
                id: l.id,
                a: l.a.device,
                b: l.b.device,
                a_port: l.a.port,
                b_port: l.b.port,
                up: l.up,
                capture: l.capture,
                delay: l.delay,
                frames: l
                    .in_flight
                    .iter()
                    .map(|f| FrameSnap {
                        t: if f.to == l.b {
                            f.progress as f32 / l.delay as f32
                        } else {
                            1.0 - f.progress as f32 / l.delay as f32
                        },
                        dir: if f.to == l.b { 1 } else { -1 },
                        color: f.frame.color_key(),
                        label: f.frame.label(),
                    })
                    .collect(),
            })
            .collect();
        Snapshot { tick: self.tick, devices, links }
    }
}
