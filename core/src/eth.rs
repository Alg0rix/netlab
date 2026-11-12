//! Real Ethernet/ARP/IPv4/ICMP codec — the boundary between netlab's
//! simplified frames and emulated routers (Dynamips NIOs, v86 NICs) that
//! speak real wire format.

use crate::frame::{Frame, Payload};

fn u16be(b: &[u8], o: usize) -> u16 {
    ((b[o] as u16) << 8) | b[o + 1] as u16
}

fn u32be(b: &[u8], o: usize) -> u32 {
    ((b[o] as u32) << 24) | ((b[o + 1] as u32) << 16) | ((b[o + 2] as u32) << 8) | b[o + 3] as u32
}

pub(crate) fn mac_of(b: &[u8]) -> u64 {
    b.iter().take(6).fold(0u64, |m, &x| (m << 8) | x as u64)
}

pub(crate) fn mac_bytes(m: u64) -> [u8; 6] {
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
pub(crate) fn cksum(data: &[u8]) -> u16 {
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
    let mut etype = u16be(raw, 12);
    let mut hdr = 14usize;
    let mut vlan = 0u16;
    if (etype == 0x8100 || etype == 0x88a8) && raw.len() >= 18 {
        vlan = u16be(raw, 14) & 0x0FFF;
        etype = u16be(raw, 16);
        hdr = 18;
    }
    let body = &raw[hdr..];
    let mut f = Frame {
        src_mac,
        dst_mac,
        src_ip: [0, 0, 0, 0],
        dst_ip: [0, 0, 0, 0],
        ttl: 1,
        vlan,
        payload: Payload::Raw { ethertype: etype },
        raw: Some(raw.to_vec()),
        icmp_echo: None,
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
                let total = (u16be(body, 2) as usize).clamp(ihl, body.len());
                let icmp = &body[ihl..total];
                f.payload = match icmp[0] {
                    8 => Payload::IcmpEchoRequest,
                    0 => Payload::IcmpEchoReply,
                    11 => Payload::IcmpTimeExceeded,
                    _ => f.payload,
                };
                if matches!(f.payload, Payload::IcmpEchoRequest | Payload::IcmpEchoReply)
                    && icmp.len() >= 8
                {
                    f.icmp_echo = Some(icmp[4..].to_vec());
                }
            }
        }
        _ => {}
    }
    Some(f)
}

/// Encode a netlab `Frame` into raw Ethernet bytes. Frames carrying original
/// wire bytes pass through unchanged.
/// Force the 802.1Q tag on `frame` to `vid` (0 strips it), including `raw` bytes.
pub fn set_vlan(frame: &mut Frame, vid: u16) {
    frame.vlan = vid & 0x0FFF;
    if let Some(raw) = frame.raw.as_mut() {
        align_tag(raw, frame.vlan);
    }
}

fn align_tag(out: &mut Vec<u8>, vid: u16) {
    if out.len() < 14 {
        return;
    }
    let et = u16be(out, 12);
    let has = et == 0x8100 || et == 0x88a8;
    if vid == 0 {
        if has && out.len() >= 18 {
            let rest = out[16..].to_vec();
            out.truncate(12);
            out.extend_from_slice(&rest);
        }
        return;
    }
    if has && out.len() >= 16 {
        let t = (vid & 0x0FFF).to_be_bytes();
        out[12] = 0x81;
        out[13] = 0x00;
        out[14] = t[0];
        out[15] = t[1];
        return;
    }
    let rest = out[12..].to_vec();
    out.truncate(12);
    out.extend_from_slice(&0x8100u16.to_be_bytes());
    out.extend_from_slice(&(vid & 0x0FFF).to_be_bytes());
    out.extend_from_slice(&rest);
}

pub fn encode(f: &Frame) -> Option<Vec<u8>> {
    if let Some(raw) = &f.raw {
        // Original bytes, but with this hop's MACs, VLAN tag and TTL.
        let mut out = raw.clone();
        if out.len() >= 14 {
            out[0..6].copy_from_slice(&mac_bytes(f.dst_mac));
            out[6..12].copy_from_slice(&mac_bytes(f.src_mac));
            align_tag(&mut out, f.vlan);
            if let Some(off) = ipv4_off(&out) {
                let ihl = ((out[off] & 0x0F) as usize) * 4;
                if out.len() >= off + ihl && out[off + 8] != f.ttl {
                    out[off + 8] = f.ttl;
                    out[off + 10] = 0;
                    out[off + 11] = 0;
                    let c = cksum(&out[off..off + ihl]);
                    out[off + 10] = (c >> 8) as u8;
                    out[off + 11] = c as u8;
                }
            }
        }
        return Some(out);
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
            let op: u16 = if matches!(f.payload, Payload::ArpRequest { .. }) {
                1
            } else {
                2
            };
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
        Payload::IcmpEchoRequest | Payload::IcmpEchoReply | Payload::IcmpTimeExceeded => {
            out.extend_from_slice(&0x0800u16.to_be_bytes());
            let ty: u8 = match f.payload {
                Payload::IcmpEchoRequest => 8,
                Payload::IcmpTimeExceeded => 11,
                _ => 0,
            };
            let icmp = icmp_packet(ty, 0, f.icmp_echo.as_deref().unwrap_or(&[0, 0, 0, 1]));
            push_ipv4(&mut out, f.ttl, 1, 0, f.src_ip, f.dst_ip, icmp.len());
            out.extend_from_slice(&icmp);
        }
        Payload::Raw { .. } => return None,
    }
    if f.vlan != 0 {
        align_tag(&mut out, f.vlan);
    }
    Some(out)
}

fn l4_name(proto: u8) -> &'static str {
    match proto {
        6 => "TCP",
        17 => "UDP",
        47 => "GRE",
        88 => "EIGRP",
        89 => "OSPF",
        112 => "VRRP",
        _ => "IP",
    }
}

/// Human-readable label for a raw Ethernet frame (event log).
pub fn raw_label(raw: &[u8]) -> String {
    if let Some(off) = ipv4_off(raw) {
        if raw.len() > off + 9 {
            return l4_name(raw[off + 9]).into();
        }
    }
    if raw.len() >= 14 {
        match u16be(raw, 12) {
            0x0800 => "IP".into(),
            0x0806 => "ARP".into(),
            0x8100 => {
                if raw.len() >= 18 {
                    let vid = u16be(raw, 14) & 0x0FFF;
                    format!("802.1Q VLAN {}", vid)
                } else {
                    "802.1Q".into()
                }
            }
            0x88a8 => "802.1ad QinQ".into(),
            0x88cc => "LLDP".into(),
            0x8809 => "LACP".into(),
            0x86dd => "IPv6".into(),
            t if t <= 1500 => {
                if raw[0..6] == [0x01, 0x80, 0xc2, 0x00, 0x00, 0x00]
                    || (raw.len() >= 17 && raw[14] == 0x42 && raw[15] == 0x42)
                {
                    "STP BPDU".into()
                } else if raw[0..6] == [0x01, 0x00, 0x0c, 0xcc, 0xcc, 0xcd] {
                    "PVST+ BPDU".into()
                } else if raw[0..6] == [0x01, 0x00, 0x0c, 0xcc, 0xcc, 0xcc] {
                    "CDP/VTP".into()
                } else {
                    "802.3 LLC".into()
                }
            }
            t => format!("ethertype 0x{:04x}", t),
        }
    } else {
        "frame".into()
    }
}

/// Where the IPv4 header starts, skipping one 802.1Q / 802.1ad tag.
pub(crate) fn ipv4_off(frame: &[u8]) -> Option<usize> {
    if frame.len() < 34 {
        return None;
    }
    let et = u16be(frame, 12);
    if et == 0x0800 && frame[14] >> 4 == 4 {
        return Some(14);
    }
    if (et == 0x8100 || et == 0x88a8)
        && frame.len() >= 38
        && u16be(frame, 16) == 0x0800
        && frame[18] >> 4 == 4
    {
        return Some(18);
    }
    None
}

#[derive(Clone, Copy)]
pub(crate) struct IpView {
    pub off: usize,
    pub ihl: usize,
    pub proto: u8,
    pub sport: u16,
    pub dport: u16,
    pub icmp_type: u8,
    pub icmp_id: u16,
    pub payload: usize,
}

pub(crate) fn view_ip(frame: &[u8]) -> Option<IpView> {
    let off = ipv4_off(frame)?;
    let ihl = ((frame[off] & 0x0f) as usize) * 4;
    if ihl < 20 || frame.len() < off + ihl {
        return None;
    }
    let proto = frame[off + 9];
    let l4 = off + ihl;
    let mut sport = 0u16;
    let mut dport = 0u16;
    let mut icmp_type = 0u8;
    let mut icmp_id = 0u16;
    let mut payload = l4;
    if (proto == 6 || proto == 17) && frame.len() >= l4 + 4 {
        sport = u16be(frame, l4);
        dport = u16be(frame, l4 + 2);
        payload = l4
            + if proto == 17 {
                8
            } else {
                20.min(frame.len() - l4)
            };
    } else if proto == 1 && frame.len() >= l4 + 8 {
        icmp_type = frame[l4];
        icmp_id = u16be(frame, l4 + 4);
        payload = l4 + 8;
    }
    Some(IpView {
        off,
        ihl,
        proto,
        sport,
        dport,
        icmp_type,
        icmp_id,
        payload,
    })
}

fn pseudo_sum(src: [u8; 4], dst: [u8; 4], proto: u8, len: usize) -> u32 {
    let mut s = 0u32;
    s += ((src[0] as u32) << 8) | src[1] as u32;
    s += ((src[2] as u32) << 8) | src[3] as u32;
    s += ((dst[0] as u32) << 8) | dst[1] as u32;
    s += ((dst[2] as u32) << 8) | dst[3] as u32;
    s += proto as u32;
    s + len as u32
}

fn udp_sum(c: u16) -> u16 {
    if c == 0 {
        0xffff
    } else {
        c
    }
}

fn finish(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn sum_words(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | data[i + 1] as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    sum
}

/// Write addresses and TTL into an Ethernet/IPv4 frame and recompute checksums.
/// Optional L4 port / ICMP id overrides are applied first.
pub(crate) fn sync_ipv4(
    frame: &mut [u8],
    src: [u8; 4],
    dst: [u8; 4],
    ttl: u8,
    sport: Option<u16>,
    dport: Option<u16>,
    icmp_id: Option<u16>,
) {
    let Some(v) = view_ip(frame) else { return };
    let off = v.off;
    frame[off + 8] = ttl;
    frame[off + 12..off + 16].copy_from_slice(&src);
    frame[off + 16..off + 20].copy_from_slice(&dst);
    frame[off + 10] = 0;
    frame[off + 11] = 0;
    let c = cksum(&frame[off..off + v.ihl]);
    frame[off + 10] = (c >> 8) as u8;
    frame[off + 11] = c as u8;

    let l4 = off + v.ihl;
    let total = u16be(frame, off + 2) as usize;
    let seg_end = (off + total).min(frame.len()).max(l4);
    if v.proto == 17 && frame.len() >= l4 + 8 {
        if let Some(p) = sport {
            frame[l4..l4 + 2].copy_from_slice(&p.to_be_bytes());
        }
        if let Some(p) = dport {
            frame[l4 + 2..l4 + 4].copy_from_slice(&p.to_be_bytes());
        }
        let ulen = u16be(frame, l4 + 4) as usize;
        let end = (l4 + ulen).min(frame.len());
        frame[l4 + 6] = 0;
        frame[l4 + 7] = 0;
        let sum = pseudo_sum(src, dst, 17, end - l4) + sum_words(&frame[l4..end]);
        let c = udp_sum(finish(sum));
        frame[l4 + 6] = (c >> 8) as u8;
        frame[l4 + 7] = c as u8;
    } else if v.proto == 6 && frame.len() >= l4 + 20 {
        if let Some(p) = sport {
            frame[l4..l4 + 2].copy_from_slice(&p.to_be_bytes());
        }
        if let Some(p) = dport {
            frame[l4 + 2..l4 + 4].copy_from_slice(&p.to_be_bytes());
        }
        if seg_end >= l4 + 18 {
            frame[l4 + 16] = 0;
            frame[l4 + 17] = 0;
            let sum = pseudo_sum(src, dst, 6, seg_end - l4) + sum_words(&frame[l4..seg_end]);
            let c = finish(sum);
            frame[l4 + 16] = (c >> 8) as u8;
            frame[l4 + 17] = c as u8;
        }
    } else if v.proto == 1 && frame.len() >= l4 + 8 {
        if let Some(id) = icmp_id {
            frame[l4 + 4..l4 + 6].copy_from_slice(&id.to_be_bytes());
        }
        frame[l4 + 2] = 0;
        frame[l4 + 3] = 0;
        let c = cksum(&frame[l4..seg_end]);
        frame[l4 + 2] = (c >> 8) as u8;
        frame[l4 + 3] = c as u8;
    }
}

fn push_ipv4(
    out: &mut Vec<u8>,
    ttl: u8,
    proto: u8,
    id: u16,
    src: [u8; 4],
    dst: [u8; 4],
    payload_len: usize,
) {
    let total = 20 + payload_len;
    out.push(0x45);
    out.push(0);
    out.extend_from_slice(&(total as u16).to_be_bytes());
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&0x4000u16.to_be_bytes());
    out.push(ttl);
    out.push(proto);
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&src);
    out.extend_from_slice(&dst);
    let start = out.len() - 20;
    let c = cksum(&out[start..]);
    out[start + 10] = (c >> 8) as u8;
    out[start + 11] = c as u8;
}

/// ICMP message: type, code, checksum over `body` (id/seq/data).
pub(crate) fn icmp_packet(ty: u8, code: u8, body: &[u8]) -> Vec<u8> {
    let mut icmp = vec![ty, code, 0, 0];
    icmp.extend_from_slice(body);
    let c = cksum(&icmp);
    icmp[2] = (c >> 8) as u8;
    icmp[3] = c as u8;
    icmp
}

/// The header slots every wire builder shares: the Ethernet pair, the
/// IPv4 addresses, ttl and the identification field.
#[derive(Clone, Copy)]
pub(crate) struct IpSpec {
    pub src_mac: u64,
    pub dst_mac: u64,
    pub src: [u8; 4],
    pub dst: [u8; 4],
    pub ttl: u8,
    pub id: u16,
}

/// TCP header fields beyond `IpSpec` — the port pair and the
/// sequence/acknowledgement/flags word.
#[derive(Clone, Copy)]
pub(crate) struct TcpHdr {
    pub sport: u16,
    pub dport: u16,
    pub seq: u32,
    pub ack: u32,
    pub flags: u8,
}

/// Ethernet/IPv4 frame carrying an ICMP message.
pub(crate) fn build_icmp(s: IpSpec, ty: u8, code: u8, body: &[u8]) -> Vec<u8> {
    build_ip(s, 1, &icmp_packet(ty, code, body))
}

pub(crate) fn build_ip(s: IpSpec, proto: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(14 + 20 + payload.len());
    out.extend_from_slice(&mac_bytes(s.dst_mac));
    out.extend_from_slice(&mac_bytes(s.src_mac));
    out.extend_from_slice(&0x0800u16.to_be_bytes());
    push_ipv4(&mut out, s.ttl, proto, s.id, s.src, s.dst, payload.len());
    out.extend_from_slice(payload);
    out
}

pub(crate) fn build_udp(s: IpSpec, sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(14 + 20 + 8 + payload.len());
    out.extend_from_slice(&mac_bytes(s.dst_mac));
    out.extend_from_slice(&mac_bytes(s.src_mac));
    out.extend_from_slice(&0x0800u16.to_be_bytes());
    push_ipv4(&mut out, s.ttl, 17, s.id, s.src, s.dst, 8 + payload.len());
    out.extend_from_slice(&sport.to_be_bytes());
    out.extend_from_slice(&dport.to_be_bytes());
    out.extend_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(payload);
    let l4 = 14 + 20;
    let sum = pseudo_sum(s.src, s.dst, 17, 8 + payload.len()) + sum_words(&out[l4..]);
    let c = udp_sum(finish(sum));
    out[l4 + 6] = (c >> 8) as u8;
    out[l4 + 7] = c as u8;
    out
}

pub(crate) fn build_tcp(s: IpSpec, h: TcpHdr, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(14 + 20 + 20 + payload.len());
    out.extend_from_slice(&mac_bytes(s.dst_mac));
    out.extend_from_slice(&mac_bytes(s.src_mac));
    out.extend_from_slice(&0x0800u16.to_be_bytes());
    push_ipv4(&mut out, s.ttl, 6, s.id, s.src, s.dst, 20 + payload.len());
    out.extend_from_slice(&h.sport.to_be_bytes());
    out.extend_from_slice(&h.dport.to_be_bytes());
    out.extend_from_slice(&h.seq.to_be_bytes());
    out.extend_from_slice(&h.ack.to_be_bytes());
    out.push(0x50); // data offset 5
    out.push(h.flags);
    out.extend_from_slice(&65535u16.to_be_bytes());
    out.extend_from_slice(&[0, 0]); // checksum
    out.extend_from_slice(&0u16.to_be_bytes()); // urgent
    out.extend_from_slice(payload);
    let l4 = 34;
    let sum = pseudo_sum(s.src, s.dst, 6, 20 + payload.len()) + sum_words(&out[l4..]);
    let c = finish(sum);
    out[l4 + 16] = (c >> 8) as u8;
    out[l4 + 17] = c as u8;
    out
}

/// Canvas colour for a raw Ethernet frame.
pub fn l4_color(raw: &[u8]) -> &'static str {
    if let Some(v) = view_ip(raw) {
        if v.proto == 17 && (v.sport == 67 || v.dport == 67 || v.sport == 68 || v.dport == 68) {
            return "dhcp";
        }
        if v.proto == 17 && (v.sport == 53 || v.dport == 53) {
            return "dns";
        }
        return match v.proto {
            1 => "icmp",
            6 => "tcp",
            17 => "udp",
            89 => "ospf",
            _ => "raw",
        };
    }
    if raw.len() >= 14 {
        match u16be(raw, 12) {
            0x0806 => return "arp",
            0x8100 | 0x88a8 => return "vlan",
            t if t <= 1500 => return "stp",
            _ => {}
        }
    }
    "raw"
}
