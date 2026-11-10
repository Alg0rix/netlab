//! Minimal OSPFv2 for simulated routers: hellos on 224.0.0.5, router LSAs of
//! connected prefixes, flood to neighbors, Dijkstra, install as proto 1.
//! Multicast is link-local and is never forwarded.

use std::collections::HashMap;

use crate::device::{Device, DeviceKind, Route};
use crate::eth;
use crate::events::SimEvent;
use crate::frame::{Frame, Payload};
use crate::ip_str;
use crate::net_of;

const ALLSPF: [u8; 4] = [224, 0, 0, 5];
const OSPF_MAC: u64 = 0x0100_5e00_0005;
const HELLO_EVERY: u64 = 10;
const DEAD: u64 = 40;

#[derive(Debug, Clone)]
pub struct OspfNet {
    pub prefix: [u8; 4],
    pub wild: [u8; 4],
    pub area: u32,
}

#[derive(Debug, Clone)]
pub struct Nbr {
    pub rid: [u8; 4],
    pub ip: [u8; 4],
    pub port: usize,
    pub area: u32,
    pub last: u64,
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
    pub router_id: [u8; 4],
    pub nets: Vec<OspfNet>,
    /// Per-port area from `ip ospf area`. None = not set on that port.
    pub if_area: Vec<Option<u32>>,
    pub nbrs: Vec<Nbr>,
    pub lsas: Vec<Lsa>,
    pub next_hello: u64,
    pub seq: u32,
}

impl Default for Ospf {
    fn default() -> Self {
        Self {
            on: false,
            router_id: [0; 4],
            nets: Vec::new(),
            if_area: Vec::new(),
            nbrs: Vec::new(),
            lsas: Vec::new(),
            next_hello: 0,
            seq: 0x8000_0001,
        }
    }
}

pub fn tick(dev: &mut Device, now: u64, events: &mut Vec<SimEvent>) {
    if !dev.ospf.on || !dev.powered || dev.kind != DeviceKind::Router {
        return;
    }
    ensure_rid(dev);
    let before = dev.ospf.nbrs.len();
    dev.ospf.nbrs.retain(|n| now.saturating_sub(n.last) <= DEAD);
    if dev.ospf.nbrs.len() != before {
        originate(dev, now, events);
    }
    if now < dev.ospf.next_hello {
        return;
    }
    dev.ospf.next_hello = now + HELLO_EVERY;
    let ports: Vec<(usize, u32, [u8; 4], [u8; 4])> = (0..dev.ports.len())
        .filter_map(|i| {
            let area = area_of(dev, i)?;
            let (ip, mask) = dev.ports[i].ip?;
            Some((i, area, ip, mask))
        })
        .collect();
    for (port, area, ip, mask) in ports {
        let nbrs: Vec<[u8; 4]> = dev.ospf.nbrs.iter().filter(|n| n.port == port).map(|n| n.rid).collect();
        send(dev, port, ip, hello(dev.ospf.router_id, area, mask, &nbrs));
    }
}

/// True when the frame was OSPF and must not be routed.
pub fn ingest(dev: &mut Device, frame: &Frame, in_port: usize, tick: u64, events: &mut Vec<SimEvent>) -> bool {
    if frame.dst_ip != ALLSPF && !is_ospf(frame) {
        return false;
    }
    if !is_ospf(frame) {
        return frame.dst_ip == ALLSPF;
    }
    if !dev.ospf.on || dev.kind != DeviceKind::Router {
        return true;
    }
    let Some(raw) = frame.raw.as_deref() else { return true };
    let Some(v) = eth::view_ip(raw) else { return true };
    let body = &raw[v.payload.min(raw.len())..];
    if body.len() < 24 || body[0] != 2 {
        return true;
    }
    let Some(area) = area_of(dev, in_port) else { return true };
    let pkt_area = u32::from_be_bytes(body[12..16].try_into().unwrap());
    if pkt_area != area {
        return true;
    }
    let Some((my_ip, mask)) = dev.ports[in_port].ip else { return true };
    if net_of(frame.src_ip, mask) != net_of(my_ip, mask) {
        return true;
    }
    let rid = [body[4], body[5], body[6], body[7]];
    if rid == dev.ospf.router_id || rid == [0; 4] {
        return true;
    }
    match body[1] {
        1 => {
            let new = !dev.ospf.nbrs.iter().any(|n| n.rid == rid && n.port == in_port);
            if let Some(n) = dev.ospf.nbrs.iter_mut().find(|n| n.rid == rid && n.port == in_port) {
                n.last = tick;
                n.ip = frame.src_ip;
                n.area = area;
            } else {
                dev.ospf.nbrs.push(Nbr { rid, ip: frame.src_ip, port: in_port, area, last: tick });
                crate::services::note(events, tick, dev, "ospf", &format!("OSPF neighbor {} up on {}", ip_str(frame.src_ip), in_port));
            }
            if new {
                originate(dev, tick, events);
            }
        }
        4 => {
            if parse_lsu(dev, &body[24..]) {
                install(dev);
                // reflood out other OSPF ports
                let payload = body.to_vec();
                let ports: Vec<(usize, [u8; 4])> = (0..dev.ports.len())
                    .filter(|p| *p != in_port && area_of(dev, *p).is_some())
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
    frame.raw.as_deref().and_then(eth::view_ip).map(|v| v.proto == 89).unwrap_or(false)
}

pub fn area_of(dev: &Device, port: usize) -> Option<u32> {
    if let Some(Some(a)) = dev.ospf.if_area.get(port) {
        return Some(*a);
    }
    let (ip, _) = dev.ports.get(port)?.ip?;
    dev.ospf.nets.iter().find(|n| crate::device::wild_match(ip, n.prefix, n.wild)).map(|n| n.area)
}

fn ensure_rid(dev: &mut Device) {
    if dev.ospf.router_id != [0; 4] {
        return;
    }
    dev.ospf.router_id = dev.ports.iter().filter_map(|p| p.ip.map(|(a, _)| a)).max().unwrap_or([0; 4]);
}

fn originate(dev: &mut Device, tick: u64, events: &mut Vec<SimEvent>) {
    ensure_rid(dev);
    let rid = dev.ospf.router_id;
    if rid == [0; 4] {
        return;
    }
    dev.ospf.seq = dev.ospf.seq.wrapping_add(1);
    let stubs: Vec<([u8; 4], [u8; 4])> = dev
        .ports
        .iter()
        .filter_map(|p| p.ip.map(|(a, m)| (net_of(a, m), m)))
        .collect();
    let links: Vec<[u8; 4]> = dev.ospf.nbrs.iter().map(|n| n.rid).collect();
    dev.ospf.lsas.retain(|l| l.rid != rid);
    dev.ospf.lsas.push(Lsa { rid, seq: dev.ospf.seq, stubs: stubs.clone(), links: links.clone() });
    install(dev);
    let pkt = lsu(rid, dev.ospf.seq, &stubs, &links);
    let ports: Vec<(usize, [u8; 4], u32)> = (0..dev.ports.len())
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
    let _ = (tick, events);
}

fn install(dev: &mut Device) {
    dev.routes.retain(|r| r.proto != 1);
    let me = dev.ospf.router_id;
    let mut dist: HashMap<[u8; 4], u32> = HashMap::new();
    let mut hop: HashMap<[u8; 4], (usize, [u8; 4])> = HashMap::new();
    dist.insert(me, 0);
    for n in &dev.ospf.nbrs {
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
            let Some(d0) = dist.get(&lsa.rid).copied() else { continue };
            for nbr in &lsa.links {
                let nd = d0 + 1;
                if dist.get(nbr).copied().unwrap_or(u32::MAX) > nd {
                    dist.insert(*nbr, nd);
                    if let Some(via) = hop.get(&lsa.rid).copied().or_else(|| hop.get(nbr).copied()) {
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
        let Some(&(port, nh)) = hop.get(&lsa.rid) else { continue };
        if port >= dev.ports.len() {
            continue;
        }
        for &(prefix, mask) in &lsa.stubs {
            dev.routes.push(Route {
                prefix,
                mask,
                next_hop: nh,
                out_port: port,
                connected: false,
                proto: 1,
            });
        }
    }
}

fn hello(rid: [u8; 4], area: u32, mask: [u8; 4], nbrs: &[[u8; 4]]) -> Vec<u8> {
    let mut b = header(1, rid, area, 24 + 20 + nbrs.len() * 4);
    b.extend_from_slice(&mask);
    b.extend_from_slice(&10u16.to_be_bytes());
    b.push(0x02);
    b.push(1);
    b.extend_from_slice(&40u32.to_be_bytes());
    b.extend_from_slice(&[0; 8]); // DR, BDR
    for n in nbrs {
        b.extend_from_slice(n);
    }
    b
}

fn lsu(rid: [u8; 4], seq: u32, stubs: &[([u8; 4], [u8; 4])], links: &[[u8; 4]]) -> Vec<u8> {
    let mut b = header(4, rid, 0, 24 + 4 + 4 + 4 + 2 + stubs.len() * 8 + links.len() * 4);
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
    if rid == dev.ospf.router_id {
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
    dev.ospf.lsas.push(Lsa { rid, seq, stubs, links });
    true
}

fn send(dev: &mut Device, port: usize, src: [u8; 4], payload: Vec<u8>) {
    let mac = dev.ports[port].mac;
    let raw = eth::build_ip(mac, OSPF_MAC, src, ALLSPF, 1, 89, dev.ip_id, &payload);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.outbox.push((
        port,
        Frame {
            src_mac: mac,
            dst_mac: OSPF_MAC,
            src_ip: src,
            dst_ip: ALLSPF,
            ttl: 1,
            vlan: 0,
            payload: Payload::Raw { ethertype: 0x0800 },
            raw: Some(raw),
            icmp_echo: None,
        },
    ));
}
