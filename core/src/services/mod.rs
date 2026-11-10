//! DHCP, DNS, TCP/UDP/HTTP and source NAT for simulated hosts and routers.
//! Packets are real Ethernet/IPv4 frames so a Cisco IOS or RouterOS node on
//! the same link sees ordinary DHCP, DNS and TCP.

mod dhcp;
mod dns;
mod flows;
mod nat;

pub use dhcp::{DhcpClient, DhcpState};
pub use flows::Flow;
pub use nat::{NatEnt, StaticNat};

pub(crate) use dhcp::{dhcp_state, start_dhcp};
pub(crate) use dns::start_dns;
pub(crate) use flows::{start_tcp, start_udp};
pub(crate) use nat::{nat_egress, nat_ingress};

use crate::device::{Device, DeviceKind};
use crate::eth::{self, view_ip};
use crate::events::SimEvent;
use crate::frame::{Frame, Payload};

/// Per-tick service work: DHCP retransmits, flow deadlines.
/// OSPF is ticked by `Topology::step`, not here.
pub(crate) fn poll(dev: &mut Device, tick: u64, events: &mut Vec<SimEvent>) {
    dhcp::poll_client(dev, tick, events);
    flows::poll_flows(dev, tick, events);
}
/// Local DHCP / DNS / TCP / UDP handling. True = do not also route the frame.
pub fn local_l4(
    dev: &mut Device,
    frame: &Frame,
    in_port: usize,
    tick: u64,
    events: &mut Vec<SimEvent>,
) -> bool {
    let Some(raw) = frame.raw.as_deref() else {
        return false;
    };
    let Some(v) = view_ip(raw) else { return false };
    let bcast = frame.is_broadcast();
    let mine = dev.owns_ip(frame.dst_ip);
    if v.proto == 17 && (v.dport == 67 || v.dport == 68) {
        return dhcp::dispatch(dev, frame, raw, in_port, tick, events);
    }
    if bcast && !mine {
        return dev.kind != DeviceKind::Router;
    }
    if !mine {
        return false;
    }
    if v.proto == 17 && v.dport == 53 && dev.svc_dns {
        dns::on_dns(dev, frame, raw, v, in_port, tick, events);
        return true;
    }
    if v.proto == 17 {
        flows::on_udp(dev, frame, raw, v, in_port, tick, events);
        return true;
    }
    if v.proto == 6 {
        flows::on_tcp(dev, frame, raw, v, in_port, tick, events);
        return true;
    }
    if v.proto == 1 && v.icmp_type == 3 {
        flows::on_unreach(dev, frame, tick, events);
        return true;
    }
    false
}

pub(super) fn udp_frame(
    dev: &mut Device,
    smac: u64,
    dmac: u64,
    src: [u8; 4],
    dst: [u8; 4],
    sport: u16,
    dport: u16,
    payload: &[u8],
) -> Frame {
    let raw = eth::build_udp(smac, dmac, src, dst, 64, dev.ip_id, sport, dport, payload);
    dev.ip_id = dev.ip_id.wrapping_add(1);
    let mut f = Frame::raw_ip(src, dst, 64, raw);
    f.src_mac = smac;
    f.dst_mac = dmac;
    f
}

pub(crate) fn l4_tuple(frame: &Frame) -> (u8, u16, u16, u16) {
    if let Some(raw) = &frame.raw {
        if let Some(v) = view_ip(raw) {
            return (v.proto, v.sport, v.dport, v.icmp_id);
        }
    }
    match frame.payload {
        Payload::IcmpEchoRequest | Payload::IcmpEchoReply | Payload::IcmpTimeExceeded => {
            let id = frame
                .icmp_echo
                .as_ref()
                .filter(|e| e.len() >= 2)
                .map(|e| u16::from_be_bytes([e[0], e[1]]))
                .unwrap_or(0);
            (1, 0, 0, id)
        }
        _ => (0, 0, 0, 0),
    }
}

pub(super) fn alloc_port(dev: &mut Device) -> u16 {
    let p = 40_000 + (dev.ip_id % 20_000) as u16;
    dev.ip_id = dev.ip_id.wrapping_add(1);
    p
}

pub(super) fn next_id(dev: &mut Device) -> u32 {
    dev.ip_id = dev.ip_id.wrapping_add(1);
    dev.ip_id as u32
}

pub(crate) fn note(
    events: &mut Vec<SimEvent>,
    tick: u64,
    dev: &Device,
    kind: &'static str,
    msg: &str,
) {
    events.push(SimEvent::new(
        tick,
        Some(dev.id),
        None,
        kind,
        format!("{}: {msg}", dev.name),
    ));
}
