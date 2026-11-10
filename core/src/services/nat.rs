//! Source NAT (masquerade) and static 1:1 / port-forward maps.

use crate::device::{Device, DeviceKind, NatRole};
use crate::eth;
use crate::frame::Frame;

use super::{alloc_port, l4_tuple};
#[derive(Clone, Debug)]
pub struct NatEnt {
    pub proto: u8,
    pub inside_ip: [u8; 4],
    pub inside_port: u16,
    pub outside_port: u16,
    pub peer_ip: [u8; 4],
    pub peer_port: u16,
}

/// proto 0 is a 1:1 map of every protocol. proto 6 or 17 is one static port forward.
/// The outside address is always the single outside interface.
#[derive(Clone, Debug)]
pub struct StaticNat {
    pub proto: u8,
    pub inside_ip: [u8; 4],
    pub inside_port: u16,
    pub outside_port: u16,
}

/// True when the frame was translated toward an inside host and should be forwarded.
pub fn nat_ingress(dev: &mut Device, frame: &mut Frame, in_port: usize) -> bool {
    if dev.kind != DeviceKind::Router || in_port >= dev.ports.len() {
        return false;
    }
    if dev.ports[in_port].nat != NatRole::Outside {
        return false;
    }
    let Some((outside_ip, _)) = dev.ports[in_port].ip else {
        return false;
    };
    if frame.dst_ip != outside_ip {
        return false;
    }
    let (proto, _sport, dport, icmp_id) = l4_tuple(frame);
    if proto == 6 || proto == 17 {
        if let Some(map) = dev
            .static_nat
            .iter()
            .find(|s| s.proto == proto && s.outside_port == dport)
            .cloned()
        {
            frame.dst_ip = map.inside_ip;
            if let Some(raw) = frame.raw.as_mut() {
                eth::sync_ipv4(
                    raw,
                    frame.src_ip,
                    frame.dst_ip,
                    frame.ttl,
                    None,
                    Some(map.inside_port),
                    None,
                );
            }
            return true;
        }
    }
    if proto != 0 {
        let key_port = if proto == 1 { icmp_id } else { dport };
        let peer_port = if proto == 1 { 0 } else { _sport };
        if let Some(ent) = dev
            .nat
            .iter()
            .find(|e| {
                e.proto == proto
                    && e.outside_port == key_port
                    && e.peer_ip == frame.src_ip
                    && e.peer_port == peer_port
            })
            .cloned()
        {
            frame.dst_ip = ent.inside_ip;
            if let Some(raw) = frame.raw.as_mut() {
                if proto == 1 {
                    eth::sync_ipv4(
                        raw,
                        frame.src_ip,
                        frame.dst_ip,
                        frame.ttl,
                        None,
                        None,
                        Some(ent.inside_port),
                    );
                } else {
                    eth::sync_ipv4(
                        raw,
                        frame.src_ip,
                        frame.dst_ip,
                        frame.ttl,
                        None,
                        Some(ent.inside_port),
                        None,
                    );
                }
            } else if proto == 1 {
                let id = ent.inside_port.to_be_bytes();
                let mut echo = frame.icmp_echo.clone().unwrap_or_else(|| vec![0, 0, 0, 1]);
                if echo.len() >= 2 {
                    echo[0] = id[0];
                    echo[1] = id[1];
                }
                frame.icmp_echo = Some(echo);
            }
            return true;
        }
    }
    if let Some(map) = dev.static_nat.iter().find(|s| s.proto == 0).cloned() {
        frame.dst_ip = map.inside_ip;
        if let Some(raw) = frame.raw.as_mut() {
            eth::sync_ipv4(raw, frame.src_ip, frame.dst_ip, frame.ttl, None, None, None);
        }
        return true;
    }
    false
}

/// Masquerade packets that are about to leave via an outside interface.
pub fn nat_egress(dev: &mut Device, frame: &mut Frame, in_port: Option<usize>) {
    if dev.kind != DeviceKind::Router {
        return;
    }
    let Some(route) = dev.lookup(frame.dst_ip) else {
        return;
    };
    let out = route.out_port;
    if dev.ports.get(out).map(|p| p.nat) != Some(NatRole::Outside) {
        return;
    }
    let explicit_inside = dev.ports.iter().any(|p| p.nat == NatRole::Inside);
    let from_inside = match in_port {
        Some(p) => {
            let role = dev.ports.get(p).map(|p| p.nat).unwrap_or(NatRole::Off);
            role == NatRole::Inside || (!explicit_inside && role != NatRole::Outside)
        }
        None => false,
    };
    if !from_inside {
        return;
    }
    let Some((outside_ip, _)) = dev.ports[out].ip else {
        return;
    };
    let (proto, sport, dport, icmp_id) = l4_tuple(frame);
    if dev
        .static_nat
        .iter()
        .any(|s| s.proto == 0 && s.inside_ip == frame.src_ip)
    {
        frame.src_ip = outside_ip;
        if let Some(raw) = frame.raw.as_mut() {
            eth::sync_ipv4(raw, frame.src_ip, frame.dst_ip, frame.ttl, None, None, None);
        }
        return;
    }
    if proto == 6 || proto == 17 {
        if let Some(map) = dev
            .static_nat
            .iter()
            .find(|s| s.proto == proto && s.inside_ip == frame.src_ip && s.inside_port == sport)
            .cloned()
        {
            frame.src_ip = outside_ip;
            if let Some(raw) = frame.raw.as_mut() {
                eth::sync_ipv4(
                    raw,
                    frame.src_ip,
                    frame.dst_ip,
                    frame.ttl,
                    Some(map.outside_port),
                    None,
                    None,
                );
            }
            return;
        }
    }
    if proto != 1 && proto != 6 && proto != 17 {
        return;
    }
    let inside_port = if proto == 1 { icmp_id } else { sport };
    let peer_port = if proto == 1 { 0 } else { dport };
    let mut outside_port = inside_port;
    let taken = |dev: &Device, outside_port: u16| {
        dev.static_nat
            .iter()
            .any(|s| s.proto == proto && s.outside_port == outside_port)
            || dev.nat.iter().any(|e| {
                e.proto == proto
                    && e.outside_port == outside_port
                    && e.peer_ip == frame.dst_ip
                    && e.peer_port == peer_port
                    && e.inside_ip != frame.src_ip
            })
    };
    if taken(dev, outside_port) {
        outside_port = alloc_port(dev);
        if taken(dev, outside_port) {
            outside_port = outside_port.wrapping_add(1);
        }
    }
    dev.nat.retain(|e| {
        !(e.proto == proto
            && e.inside_ip == frame.src_ip
            && e.inside_port == inside_port
            && e.peer_ip == frame.dst_ip
            && e.peer_port == peer_port)
    });
    dev.nat.push(NatEnt {
        proto,
        inside_ip: frame.src_ip,
        inside_port,
        outside_port,
        peer_ip: frame.dst_ip,
        peer_port,
    });
    if dev.nat.len() > 256 {
        dev.nat.drain(0..64);
    }
    frame.src_ip = outside_ip;
    if let Some(raw) = frame.raw.as_mut() {
        if proto == 1 {
            eth::sync_ipv4(
                raw,
                frame.src_ip,
                frame.dst_ip,
                frame.ttl,
                None,
                None,
                Some(outside_port),
            );
        } else {
            eth::sync_ipv4(
                raw,
                frame.src_ip,
                frame.dst_ip,
                frame.ttl,
                Some(outside_port),
                None,
                None,
            );
        }
    } else if proto == 1 && outside_port != inside_port {
        let id = outside_port.to_be_bytes();
        let mut echo = frame.icmp_echo.clone().unwrap_or_else(|| vec![0, 0, 0, 1]);
        if echo.len() >= 2 {
            echo[0] = id[0];
            echo[1] = id[1];
        }
        frame.icmp_echo = Some(echo);
    }
}
