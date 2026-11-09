// Emulated guests (Cisco IOS, MikroTik RouterOS) speak real Ethernet. Their
// pings must get replies that a real IP stack accepts: valid checksums and the
// request's identifier, sequence and payload echoed back.

use netlab_core::{DeviceKind, Topology};

fn csum(data: &[u8]) -> u16 {
    let mut s: u32 = 0;
    for c in data.chunks(2) {
        s += ((c[0] as u32) << 8) | *c.get(1).unwrap_or(&0) as u32;
    }
    while s >> 16 != 0 {
        s = (s & 0xffff) + (s >> 16);
    }
    !(s as u16)
}

const GUEST_MAC: [u8; 6] = [0x00, 0x22, 0x15, 0xa9, 0x90, 0x08];
const GUEST_IP: [u8; 4] = [192, 168, 20, 50];
const HOST_IP: [u8; 4] = [192, 168, 20, 13];

fn arp_request() -> Vec<u8> {
    let mut f = vec![0xff; 6];
    f.extend_from_slice(&GUEST_MAC);
    f.extend_from_slice(&[0x08, 0x06, 0, 1, 0x08, 0, 6, 4, 0, 1]);
    f.extend_from_slice(&GUEST_MAC);
    f.extend_from_slice(&GUEST_IP);
    f.extend_from_slice(&[0; 6]);
    f.extend_from_slice(&HOST_IP);
    f
}

fn echo_request(host_mac: &[u8], id: u16, seq: u16, data: &[u8]) -> Vec<u8> {
    let mut icmp = vec![8, 0, 0, 0];
    icmp.extend_from_slice(&id.to_be_bytes());
    icmp.extend_from_slice(&seq.to_be_bytes());
    icmp.extend_from_slice(data);
    let c = csum(&icmp);
    icmp[2..4].copy_from_slice(&c.to_be_bytes());
    let mut ip = vec![0x45, 0, 0, 0, 0x12, 0x34, 0, 0, 64, 1, 0, 0];
    ip[2..4].copy_from_slice(&((20 + icmp.len()) as u16).to_be_bytes());
    ip.extend_from_slice(&GUEST_IP);
    ip.extend_from_slice(&HOST_IP);
    let c = csum(&ip);
    ip[10..12].copy_from_slice(&c.to_be_bytes());
    let mut f = host_mac.to_vec();
    f.extend_from_slice(&GUEST_MAC);
    f.extend_from_slice(&[0x08, 0x00]);
    f.extend_from_slice(&ip);
    f.extend_from_slice(&icmp);
    f
}

fn run_until_ext(t: &mut Topology, max: usize) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    for _ in 0..max {
        t.step();
        for (_, _, b) in t.drain_ext() {
            got.push(b);
        }
    }
    got
}

#[test]
fn guest_ping_gets_a_reply_a_real_stack_accepts() {
    let mut t = Topology::new();
    let guest = t.add_device(DeviceKind::Ext, "MT1".into(), 0.0, 0.0);
    let sw = t.add_device(DeviceKind::Switch, "SW".into(), 0.0, 0.0);
    let host = t.add_device(DeviceKind::Host, "PC".into(), 0.0, 0.0);
    t.add_link_ports(guest, 0, sw, 0).unwrap();
    t.add_link_ports(sw, 1, host, 0).unwrap();
    t.set_ip(host, 0, "192.168.20.13", "255.255.255.0");

    // ARP first, as the guest would
    t.ext_inject(guest, 0, &arp_request());
    let arp = run_until_ext(&mut t, 40);
    let reply = arp.iter().find(|f| f.len() >= 42 && f[12..14] == [0x08, 0x06] && f[21] == 2)
        .expect("guest gets an ARP reply");
    let host_mac = reply[22..28].to_vec();
    assert_eq!(&reply[0..6], &GUEST_MAC, "ARP reply addressed to the guest");

    let payload: Vec<u8> = (0u8..28).collect();
    t.ext_inject(guest, 0, &echo_request(&host_mac, 0xbeef, 7, &payload));
    let out = run_until_ext(&mut t, 40);
    let r = out.iter().find(|f| f.len() >= 42 && f[12..14] == [0x08, 0x00] && f[23] == 1)
        .expect("guest gets an ICMP packet back");

    assert_eq!(&r[0..6], &GUEST_MAC, "addressed to the guest's MAC");
    assert_eq!(&r[6..12], &host_mac[..], "from the host's MAC");
    let ihl = ((r[14] & 0x0f) as usize) * 4;
    let ip = &r[14..14 + ihl];
    assert_eq!(csum(ip), 0, "IPv4 header checksum valid");
    assert_eq!(&ip[12..16], &HOST_IP);
    assert_eq!(&ip[16..20], &GUEST_IP);
    let total = u16::from_be_bytes([ip[2], ip[3]]) as usize;
    let icmp = &r[14 + ihl..14 + total];
    assert_eq!(icmp[0], 0, "echo reply");
    assert_eq!(csum(icmp), 0, "ICMP checksum valid");
    assert_eq!(&icmp[4..6], &0xbeefu16.to_be_bytes(), "identifier echoed");
    assert_eq!(&icmp[6..8], &7u16.to_be_bytes(), "sequence echoed");
    assert_eq!(&icmp[8..], &payload[..], "payload echoed");
}
