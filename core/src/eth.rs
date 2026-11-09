//! Real Ethernet/ARP/IPv4/ICMP codec — the boundary between netlab's
//! simplified frames and emulated routers (Dynamips NIOs, v86 NICs) that
//! speak real wire format.

use crate::frame::{Frame, Payload, BROADCAST_MAC};

fn u16be(b: &[u8], o: usize) -> u16 {
    ((b[o] as u16) << 8) | b[o + 1] as u16
}

fn u32be(b: &[u8], o: usize) -> u32 {
    ((b[o] as u32) << 24) | ((b[o + 1] as u32) << 16) | ((b[o + 2] as u32) << 8) | b[o + 3] as u32
}

fn mac_of(b: &[u8]) -> u64 {
    b.iter().take(6).fold(0u64, |m, &x| (m << 8) | x as u64)
}

fn mac_bytes(m: u64) -> [u8; 6] {
    [
        (m >> 40) as u8,
        (m >> 32) as u8,
        (m >> 24) as u8,
        (m >> 16) as u8,
        (m >> 8) as u8,
        m as u8,
    ]
}

/// Internet checksum (16-bit ones' complement).
fn cksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | data[i + 1] as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Decode a raw Ethernet frame into a netlab `Frame`, keeping the original
/// bytes so forwarding preserves checksums/ids. `None` if too short or not
/// ARP/IPv4.
pub fn decode(raw: &[u8]) -> Option<Frame> {
    if raw.len() < 14 {
        return None;
    }
    let dst_mac = mac_of(&raw[0..6]);
    let src_mac = mac_of(&raw[6..12]);
    let etype = u16be(raw, 12);
    let body = &raw[14..];
    let mut f = Frame {
        src_mac,
        dst_mac,
        src_ip: [0, 0, 0, 0],
        dst_ip: [0, 0, 0, 0],
        ttl: 1,
        payload: Payload::Raw { ethertype: etype },
        raw: Some(raw.to_vec()),
    };
    match etype {
        0x0806 => {
            // ARP: htype(2) ptype(2) hlen plen op(2) sha(6) spa(4) tha(6) tpa(4)
            if body.len() < 28 || u16be(body, 0) != 1 || u16be(body, 2) != 0x0800 {
                return Some(f); // valid ethernet, odd ARP — keep raw
            }
            let op = u16be(body, 6);
            let spa = u32be(body, 14).to_be_bytes();
            let tpa = u32be(body, 24).to_be_bytes();
            f.src_ip = spa;
            f.dst_ip = tpa;
            f.payload = match op {
                1 => Payload::ArpRequest { target_ip: tpa },
                2 => Payload::ArpReply { target_ip: tpa },
                _ => return Some(f),
            };
        }
        0x0800 => {
            // IPv4 header
            if body.len() < 20 || body[0] >> 4 != 4 {
                return Some(f);
            }
            let ihl = ((body[0] & 0x0F) as usize) * 4;
            if body.len() < ihl {
                return Some(f);
            }
            f.ttl = body[8];
            let proto = body[9];
            f.src_ip = [body[12], body[13], body[14], body[15]];
            f.dst_ip = [body[16], body[17], body[18], body[19]];
            if proto == 1 && body.len() >= ihl + 4 {
                let icmp = &body[ihl..];
                f.payload = match icmp[0] {
                    8 => Payload::IcmpEchoRequest,
                    0 => Payload::IcmpEchoReply,
                    _ => f.payload,
                };
            }
        }
        _ => {}
    }
    Some(f)
}

/// Encode a netlab `Frame` into raw Ethernet bytes. Frames carrying original
/// wire bytes pass through unchanged.
pub fn encode(f: &Frame) -> Option<Vec<u8>> {
    if let Some(raw) = &f.raw {
        return Some(raw.clone());
    }
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&mac_bytes(f.dst_mac));
    out.extend_from_slice(&mac_bytes(f.src_mac));
    match f.payload {
        Payload::ArpRequest { target_ip } | Payload::ArpReply { target_ip } => {
            out.extend_from_slice(&0x0806u16.to_be_bytes());
            out.extend_from_slice(&1u16.to_be_bytes()); // htype eth
            out.extend_from_slice(&0x0800u16.to_be_bytes()); // ptype ipv4
            out.push(6);
            out.push(4);
            let op: u16 = if matches!(f.payload, Payload::ArpRequest { .. }) { 1 } else { 2 };
            out.extend_from_slice(&op.to_be_bytes());
            out.extend_from_slice(&mac_bytes(f.src_mac));
            out.extend_from_slice(&f.src_ip); // spa = answering IP
            match f.payload {
                Payload::ArpRequest { .. } => out.extend_from_slice(&[0u8; 6]), // tha
                _ => out.extend_from_slice(&mac_bytes(f.dst_mac)),
            }
            // tpa: request -> who-has target; reply -> the original asker
            match f.payload {
                Payload::ArpRequest { .. } => out.extend_from_slice(&target_ip),
                _ => out.extend_from_slice(&f.dst_ip),
            }
        }
        Payload::IcmpEchoRequest | Payload::IcmpEchoReply => {
            out.extend_from_slice(&0x0800u16.to_be_bytes());
            // ICMP: type code cksum id seq
            let ty: u8 = if matches!(f.payload, Payload::IcmpEchoRequest) { 8 } else { 0 };
            let mut icmp = vec![ty, 0, 0, 0, 0, 0, 0, 1];
            let c = cksum(&icmp);
            icmp[2] = (c >> 8) as u8;
            icmp[3] = c as u8;
            // IPv4 header
            let total = 20 + icmp.len();
            out.push(0x45);
            out.push(0);
            out.extend_from_slice(&(total as u16).to_be_bytes());
            out.extend_from_slice(&0u16.to_be_bytes()); // id
            out.extend_from_slice(&0x4000u16.to_be_bytes()); // DF
            out.push(f.ttl);
            out.push(1);
            out.extend_from_slice(&[0, 0]); // cksum placeholder
            out.extend_from_slice(&f.src_ip);
            out.extend_from_slice(&f.dst_ip);
            let hlen = out.len() - 14;
            let c = cksum(&out[14..14 + hlen]);
            let cpos = out.len() - 10;
            out[cpos] = (c >> 8) as u8;
            out[cpos + 1] = c as u8;
            out.extend_from_slice(&icmp);
        }
        Payload::Raw { .. } => return None,
    }
    Some(out)
}

/// Human-readable label for a raw Ethernet frame (event log).
pub fn raw_label(raw: &[u8]) -> String {
    if raw.len() >= 14 {
        match u16be(raw, 12) {
            0x0800 => "IP".into(),
            0x0806 => "ARP".into(),
            0x8100 => "802.1Q".into(),
            0x86dd => "IPv6".into(),
            t => format!("ethertype 0x{:04x}", t),
        }
    } else {
        "frame".into()
    }
}

pub const _: () = assert!(BROADCAST_MAC == 0xFF_FF_FF_FF_FF_FF);
