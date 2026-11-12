//! Minimal OSPFv2 for simulated routers: hellos on 224.0.0.5, router LSAs of
//! connected prefixes, flood to neighbors, Dijkstra, install as proto 1.
//! Multicast is link-local and is never forwarded.

use std::collections::HashMap;

use crate::device::{Device, DeviceKind, Route, RouteProto};
use crate::eth;
use crate::events::{EvKind, SimEvent};
use crate::frame::Frame;
use crate::ip_str;
use crate::net_of;

const ALLSPF: [u8; 4] = [224, 0, 0, 5];
const OSPF_MAC: u64 = 0x0100_5e00_0005;
const HELLO_EVERY: u64 = 10;
const DEAD: u64 = 40;
const OSPF_AD: u8 = 110;

#[derive(Debug, Clone)]
pub struct OspfNet {
    pub prefix: [u8; 4],
    pub wild: [u8; 4],
    pub area: u32,
}

/// router_id value plus provenance — explicit config beats derivation.
#[derive(Debug, Clone, Copy)]
pub struct OspfRouterId {
    pub value: [u8; 4],
    pub explicit: bool,
}

/// Neighbor FSM seat. Serde names: "O" | "FULL" | "-".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NbrState {
    O,
    Full,
    Down,
}

impl NbrState {
    pub fn as_str(self) -> &'static str {
        match self {
            NbrState::O => "O",
            NbrState::Full => "FULL",
            NbrState::Down => "-",
        }
    }

    fn word(self) -> &'static str {
        match self {
            NbrState::O => "O",
            NbrState::Full => "Full",
            NbrState::Down => "Down",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Nbr {
    pub rid: [u8; 4],
    pub ip: [u8; 4],
    pub port: usize,
    pub area: u32,
    pub state: NbrState,
    pub first_hello: u64,
    pub last_hello: u64,
    /// tick the seat fell to Down — drives the purge sweep
    down_tick: u64,
}

#[derive(Debug, Clone)]
pub struct Lsa {
    pub rid: [u8; 4],
    pub seq: u32,
    pub stubs: Vec<([u8; 4], [u8; 4])>,
    pub links: Vec<[u8; 4]>,
}

#[derive(Debug, Clone)]
pub struct Ospf {
    pub on: bool,
    pub pid: u16,
    pub router_id: OspfRouterId,
    pub nets: Vec<OspfNet>,
    /// Per-port area from `ip ospf area`. None = not set on that port.
    pub if_area: Vec<Option<u32>>,
    pub nbrs: Vec<Nbr>,
    pub lsas: Vec<Lsa>,
    pub next_hello: u64,
    pub seq: u32,
}

impl Ospf {
    /// `clear ip ospf process` — runtime state flushed, config survives.
    pub fn reset(&mut self) {
        self.nbrs.clear();
        self.lsas.clear();
        self.next_hello = 0;
    }
}

impl Default for Ospf {
    fn default() -> Self {
        Self {
            on: false,
            pid: 0,
            router_id: OspfRouterId {
                value: [0; 4],
                explicit: false,
            },
            nets: Vec::new(),
            if_area: Vec::new(),
            nbrs: Vec::new(),
            lsas: Vec::new(),
            next_hello: 0,
            seq: 0x8000_0001,
        }
    }
}

/// Derived winner: highest IPv4 among admin_up ∧ link-up ∧ addressed ports.
pub fn derive_router_id(dev: &Device) -> OspfRouterId {
    OspfRouterId {
        value: dev
            .ports
            .iter()
            .filter(|p| p.admin_up && p.link_up)
            .filter_map(|p| p.ip.map(|(a, _)| a))
            .max()
            .unwrap_or([0; 4]),
        explicit: false,
    }
}

fn adjchg(
    dev: &Device,
    tick: u64,
    events: &mut Vec<SimEvent>,
    nbr: &Nbr,
    from: NbrState,
    to: NbrState,
) {
    events.push(SimEvent::new(
        tick,
        dev.id,
        dev.ports.get(nbr.port).and_then(|p| p.link),
        EvKind::Syslog,
        format!(
            "%OSPF-5-ADJCHG: Process {}, Nbr {} on {} from {} to {}",
            dev.ospf.pid,
            ip_str(nbr.rid),
            Device::render_port_name(dev.kind, nbr.port),
            from.word(),
            to.word()
        ),
    ));
}

/// One FSM move plus its ADJCHG event (transitions only — never same-seat).
fn transition(
    dev: &mut Device,
    idx: usize,
    to: NbrState,
    tick: u64,
    events: &mut Vec<SimEvent>,
) {
    let (from, port, rid) = {
        let n = &dev.ospf.nbrs[idx];
        (n.state, n.port, n.rid)
    };
    if from == to {
        return;
    }
    {
        let n = &mut dev.ospf.nbrs[idx];
        n.state = to;
        if to == NbrState::Down {
            n.down_tick = tick;
        }
        if to == NbrState::O {
            n.first_hello = tick;
            n.last_hello = tick;
        }
    }
    let snap = Nbr {
        rid,
        ip: dev.ospf.nbrs[idx].ip,
        port,
        area: dev.ospf.nbrs[idx].area,
        state: to,
        first_hello: dev.ospf.nbrs[idx].first_hello,
        last_hello: dev.ospf.nbrs[idx].last_hello,
        down_tick: dev.ospf.nbrs[idx].down_tick,
    };
    adjchg(dev, tick, events, &snap, from, to);
}

/// link-down / admin-down on a port: every neighbor on it falls to Down.
pub fn port_down(dev: &mut Device, port: usize, tick: u64, events: &mut Vec<SimEvent>) {
    if !dev.ospf.on {
        return;
    }
    let mut moved = false;
    for i in 0..dev.ospf.nbrs.len() {
        if dev.ospf.nbrs[i].port == port && dev.ospf.nbrs[i].state != NbrState::Down {
            transition(dev, i, NbrState::Down, tick, events);
            moved = true;
        }
    }
    if moved {
        originate(dev, tick);
    }
}

/// `no network` sweep: neighbors whose port/area lost coverage fall to Down.
pub fn nbrs_off_networks(dev: &mut Device, tick: u64, events: &mut Vec<SimEvent>) {
    if !dev.ospf.on {
        return;
    }
    let mut moved = false;
    for i in 0..dev.ospf.nbrs.len() {
        let (port, area, state) = {
            let n = &dev.ospf.nbrs[i];
            (n.port, n.area, n.state)
        };
        if state != NbrState::Down && area_of(dev, port) != Some(area) {
            transition(dev, i, NbrState::Down, tick, events);
            moved = true;
        }
    }
    if moved {
        originate(dev, tick);
    }
}

pub fn tick(dev: &mut Device, now: u64, events: &mut Vec<SimEvent>) {
    if !dev.ospf.on || !dev.powered || dev.kind != DeviceKind::Router {
        return;
    }
    let mut aged = false;
    let mut i = 0;
    while i < dev.ospf.nbrs.len() {
        let (state, last, down_tick) = {
            let n = &dev.ospf.nbrs[i];
            (n.state, n.last_hello, n.down_tick)
        };
        match state {
            NbrState::O | NbrState::Full if now.saturating_sub(last) >= DEAD => {
                transition(dev, i, NbrState::Down, now, events);
                aged = true;
                i += 1;
            }
            NbrState::Down if now.saturating_sub(down_tick) >= DEAD => {
                dev.ospf.nbrs.remove(i);
            }
            _ => i += 1,
        }
    }
    if aged {
        originate(dev, now);
    }
    if now < dev.ospf.next_hello {
        return;
    }
    dev.ospf.next_hello = now + HELLO_EVERY;
    let ports: Vec<(usize, u32, [u8; 4], [u8; 4])> = (0..dev.ports.len())
        .filter(|i| dev.ports[*i].admin_up && dev.ports[*i].link_up)
        .filter_map(|i| {
            let area = area_of(dev, i)?;
            let (ip, mask) = dev.ports[i].ip?;
            Some((i, area, ip, mask))
        })
        .collect();
    for (port, area, ip, mask) in ports {
        let nbrs: Vec<[u8; 4]> = dev
            .ospf
            .nbrs
            .iter()
            .filter(|n| n.port == port && n.state != NbrState::Down)
            .map(|n| n.rid)
            .collect();
        send(
            dev,
            port,
            ip,
            hello(dev.ospf.router_id.value, area, mask, &nbrs),
        );
    }
}

/// True when the frame was OSPF and must not be routed.
pub fn ingest(
    dev: &mut Device,
    frame: &Frame,
    in_port: usize,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> bool {
    if frame.dst_ip != ALLSPF && !is_ospf(frame) {
        return false;
    }
    if !is_ospf(frame) {
        return frame.dst_ip == ALLSPF;
    }
    if !dev.ospf.on || dev.kind != DeviceKind::Router || !dev.powered {
        return true;
    }
    let Some(raw) = frame.raw.as_deref() else {
        return true;
    };
    let Some(v) = eth::view_ip(raw) else {
        return true;
    };
    let body = &raw[v.payload.min(raw.len())..];
    if body.len() < 24 || body[0] != 2 {
        return true;
    }
    if !dev.ports.get(in_port).map(|p| p.admin_up && p.link_up).unwrap_or(false) {
        return true;
    }
    let Some(area) = area_of(dev, in_port) else {
        return true;
    };
    let pkt_area = u32::from_be_bytes(body[12..16].try_into().unwrap());
    if pkt_area != area {
        return true;
    }
    let Some((my_ip, mask)) = dev.ports[in_port].ip else {
        return true;
    };
    if net_of(frame.src_ip, mask) != net_of(my_ip, mask) {
        return true;
    }
    let rid = [body[4], body[5], body[6], body[7]];
    if rid == dev.ospf.router_id.value || rid == [0; 4] {
        return true;
    }
    match body[1] {
        1 => {
            // RFC 2328 §10.5: the hello's mask must equal the port's.
            if body.len() < 28 || body[24..28] != mask {
                return true;
            }
            let mut grew = false;
            match dev
                .ospf
                .nbrs
                .iter()
                .position(|n| n.rid == rid && n.port == in_port)
            {
                Some(i) => {
                    let state = dev.ospf.nbrs[i].state;
                    match state {
                        NbrState::Down => {
                            transition(dev, i, NbrState::O, tick, events);
                            grew = true;
                        }
                        NbrState::O => {
                            dev.ospf.nbrs[i].last_hello = tick;
                            dev.ospf.nbrs[i].ip = frame.src_ip;
                            dev.ospf.nbrs[i].area = area;
                            if tick.saturating_sub(dev.ospf.nbrs[i].first_hello) >= HELLO_EVERY {
                                transition(dev, i, NbrState::Full, tick, events);
                            }
                        }
                        NbrState::Full => {
                            dev.ospf.nbrs[i].last_hello = tick;
                            dev.ospf.nbrs[i].ip = frame.src_ip;
                            dev.ospf.nbrs[i].area = area;
                        }
                    }
                }
                None => {
                    dev.ospf.nbrs.push(Nbr {
                        rid,
                        ip: frame.src_ip,
                        port: in_port,
                        area,
                        state: NbrState::O,
                        first_hello: tick,
                        last_hello: tick,
                        down_tick: 0,
                    });
                    let n = dev.ospf.nbrs.last().unwrap().clone();
                    adjchg(dev, tick, events, &n, NbrState::Down, NbrState::O);
                    grew = true;
                }
            }
            if grew {
                originate(dev, tick);
            }
        }
        4 => {
            if parse_lsu(dev, &body[24..]) {
                install(dev, tick);
                let payload = body.to_vec();
                let ports: Vec<(usize, [u8; 4])> = (0..dev.ports.len())
                    .filter(|p| {
                        *p != in_port
                            && area_of(dev, *p).is_some()
                            && dev.ports[*p].admin_up
                            && dev.ports[*p].link_up
                    })
                    .filter_map(|p| dev.ports[p].ip.map(|(a, _)| (p, a)))
                    .collect();
                for (p, ip) in ports {
                    send(dev, p, ip, payload.clone());
                }
            }
        }
        _ => {}
    }
    true
}

fn is_ospf(frame: &Frame) -> bool {
    frame
        .raw
        .as_deref()
        .and_then(eth::view_ip)
        .map(|v| v.proto == 89)
        .unwrap_or(false)
}

pub fn area_of(dev: &Device, port: usize) -> Option<u32> {
    if let Some(Some(a)) = dev.ospf.if_area.get(port) {
        return Some(*a);
    }
    let (ip, _) = dev.ports.get(port)?.ip?;
    dev.ospf
        .nets
        .iter()
        .find(|n| crate::device::wild_match(ip, n.prefix, n.wild))
        .map(|n| n.area)
}

fn originate(dev: &mut Device, tick: u64) {
    let rid = dev.ospf.router_id.value;
    if rid == [0; 4] {
        return;
    }
    dev.ospf.seq = dev.ospf.seq.wrapping_add(1);
    let stubs: Vec<([u8; 4], [u8; 4])> = dev
        .ports
        .iter()
        .filter(|p| p.admin_up && p.link_up)
        .filter_map(|p| p.ip.map(|(a, m)| (net_of(a, m), m)))
        .collect();
    let links: Vec<[u8; 4]> = dev
        .ospf
        .nbrs
        .iter()
        .filter(|n| n.state != NbrState::Down)
        .map(|n| n.rid)
        .collect();
    dev.ospf.lsas.retain(|l| l.rid != rid);
    dev.ospf.lsas.push(Lsa {
        rid,
        seq: dev.ospf.seq,
        stubs: stubs.clone(),
        links: links.clone(),
    });
    install(dev, tick);
    let pkt = lsu(rid, dev.ospf.seq, &stubs, &links);
    let ports: Vec<(usize, [u8; 4], u32)> = (0..dev.ports.len())
        .filter(|i| dev.ports[*i].admin_up && dev.ports[*i].link_up)
        .filter_map(|i| {
            let area = area_of(dev, i)?;
            let (ip, _) = dev.ports[i].ip?;
            Some((i, ip, area))
        })
        .collect();
    for (port, ip, area) in ports {
        let mut body = pkt.clone();
        body[12..16].copy_from_slice(&area.to_be_bytes());
        send(dev, port, ip, body);
    }
}

fn install(dev: &mut Device, tick: u64) {
    dev.routes.retain(|r| r.proto != RouteProto::Ospf);
    let me = dev.ospf.router_id.value;
    let mut dist: HashMap<[u8; 4], u32> = HashMap::new();
    let mut hop: HashMap<[u8; 4], (usize, [u8; 4])> = HashMap::new();
    dist.insert(me, 0);
    for n in &dev.ospf.nbrs {
        if n.state == NbrState::Down {
            continue;
        }
        dist.insert(n.rid, 1);
        hop.insert(n.rid, (n.port, n.ip));
    }
    let lsas = dev.ospf.lsas.clone();
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 64 {
            break;
        }
        let mut grew = false;
        for lsa in &lsas {
            let Some(d0) = dist.get(&lsa.rid).copied() else {
                continue;
            };
            for nbr in &lsa.links {
                let nd = d0 + 1;
                if dist.get(nbr).copied().unwrap_or(u32::MAX) > nd {
                    dist.insert(*nbr, nd);
                    if let Some(via) = hop.get(&lsa.rid).copied().or_else(|| hop.get(nbr).copied())
                    {
                        hop.insert(*nbr, via);
                    }
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
    for lsa in &lsas {
        if lsa.rid == me {
            continue;
        }
        let Some(&(port, nh)) = hop.get(&lsa.rid) else {
            continue;
        };
        if port >= dev.ports.len() {
            continue;
        }
        let metric = dist.get(&lsa.rid).copied().unwrap_or(0);
        for &(prefix, mask) in &lsa.stubs {
            dev.routes.push(Route {
                prefix,
                mask,
                next_hop: nh,
                out_port: port,
                connected: false,
                proto: RouteProto::Ospf,
                ad: OSPF_AD,
                metric,
                learned_tick: tick,
            });
        }
    }
}

fn hello(rid: [u8; 4], area: u32, mask: [u8; 4], nbrs: &[[u8; 4]]) -> Vec<u8> {
    let mut b = header(1, rid, area, 24 + 20 + nbrs.len() * 4);
    b.extend_from_slice(&mask);
    b.extend_from_slice(&(HELLO_EVERY as u16).to_be_bytes());
    b.push(0x02);
    b.push(1);
    b.extend_from_slice(&(DEAD as u32).to_be_bytes());
    b.extend_from_slice(&[0; 8]); // DR, BDR
    for n in nbrs {
        b.extend_from_slice(n);
    }
    b
}

fn lsu(rid: [u8; 4], seq: u32, stubs: &[([u8; 4], [u8; 4])], links: &[[u8; 4]]) -> Vec<u8> {
    let mut b = header(
        4,
        rid,
        0,
        24 + 4 + 4 + 4 + 2 + stubs.len() * 8 + links.len() * 4,
    );
    b.extend_from_slice(&1u32.to_be_bytes());
    b.extend_from_slice(&rid);
    b.extend_from_slice(&seq.to_be_bytes());
    b.push(stubs.len() as u8);
    b.push(links.len() as u8);
    for (p, m) in stubs {
        b.extend_from_slice(p);
        b.extend_from_slice(m);
    }
    for n in links {
        b.extend_from_slice(n);
    }
    b
}

fn header(ty: u8, rid: [u8; 4], area: u32, len: usize) -> Vec<u8> {
    let mut b = vec![0u8; 24];
    b[0] = 2;
    b[1] = ty;
    b[2..4].copy_from_slice(&(len as u16).to_be_bytes());
    b[4..8].copy_from_slice(&rid);
    b[12..16].copy_from_slice(&area.to_be_bytes());
    b
}

fn parse_lsu(dev: &mut Device, body: &[u8]) -> bool {
    if body.len() < 14 {
        return false;
    }
    let rid = [body[4], body[5], body[6], body[7]];
    let seq = u32::from_be_bytes(body[8..12].try_into().unwrap());
    if rid == dev.ospf.router_id.value {
        return false;
    }
    if dev.ospf.lsas.iter().any(|l| l.rid == rid && l.seq >= seq) {
        return false;
    }
    let ns = body[12] as usize;
    let nl = body[13] as usize;
    let mut i = 14;
    let mut stubs = Vec::new();
    for _ in 0..ns {
        if i + 8 > body.len() {
            return false;
        }
        let p = body[i..i + 4].try_into().unwrap();
        let m = body[i + 4..i + 8].try_into().unwrap();
        stubs.push((p, m));
        i += 8;
    }
    let mut links = Vec::new();
    for _ in 0..nl {
        if i + 4 > body.len() {
            return false;
        }
        links.push(body[i..i + 4].try_into().unwrap());
        i += 4;
    }
    dev.ospf.lsas.retain(|l| l.rid != rid);
    dev.ospf.lsas.push(Lsa {
        rid,
        seq,
        stubs,
        links,
    });
    true
}

fn send(dev: &mut Device, port: usize, src: [u8; 4], payload: Vec<u8>) {
    let mac = dev.ports[port].mac;
    let raw = eth::build_ip(
        eth::IpSpec {
            src_mac: mac,
            dst_mac: OSPF_MAC,
            src,
            dst: ALLSPF,
            ttl: 1,
            id: dev.ip_id,
        },
        89,
        &payload,
    );
    dev.ip_id = dev.ip_id.wrapping_add(1);
    let mut f = Frame::raw_ip(src, ALLSPF, 1, raw);
    f.src_mac = mac;
    f.dst_mac = OSPF_MAC;
    dev.outbox.push((port, f));
}
