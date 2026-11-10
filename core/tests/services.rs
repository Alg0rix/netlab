use netlab_core::{DeviceKind, Topology};

fn run(t: &mut Topology, n: usize) -> Vec<String> {
    let mut msgs = Vec::new();
    for _ in 0..n {
        t.step();
        msgs.extend(t.drain_events().into_iter().map(|e| e.msg));
    }
    msgs
}

fn port_ip(t: &Topology, id: u32) -> Option<String> {
    t.snapshot()
        .devices
        .into_iter()
        .find(|d| d.id == id)
        .and_then(|d| d.ports.into_iter().find_map(|p| p.ip))
}

#[test]
fn dhcp_dns_and_http_between_hosts() {
    let mut t = Topology::new();
    let pc = t.add_device(DeviceKind::Host, "PC1".into(), 0.0, 0.0);
    let sw = t.add_device(DeviceKind::Switch, "SW1".into(), 0.0, 0.0);
    let srv = t.add_device(DeviceKind::Host, "SRV".into(), 0.0, 0.0);
    t.add_link(pc, sw);
    t.add_link(sw, srv);
    t.set_ip(srv, 0, "10.1.0.1", "255.255.255.0");
    t.set_services(srv, true, true, true);
    assert!(t.add_dns_record(srv, "lab.local", "10.1.0.9"));
    assert!(t.dhcp_client(pc, 0));
    let msgs = run(&mut t, 80);
    let leased = port_ip(&t, pc).expect(&format!("no lease\n{}", msgs.join("\n")));
    assert_eq!(leased, "10.1.0.10", "{}", msgs.join("\n"));

    assert!(t.dns_query(pc, "lab.local"));
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter().any(|m| m.contains("lab.local") && m.contains("10.1.0.9")),
        "{}",
        msgs.join("\n")
    );

    assert!(t.http_get(pc, "10.1.0.1", 80));
    let msgs = run(&mut t, 40);
    assert!(msgs.iter().any(|m| m.contains("HTTP 200")), "{}", msgs.join("\n"));

    assert!(t.probe_tcp(pc, "10.1.0.1", 22));
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter().any(|m| m.contains("TCP 10.1.0.1:22 closed")),
        "{}",
        msgs.join("\n")
    );
}

#[test]
fn nat_masquerade_lets_an_inside_host_ping_out() {
    let mut t = Topology::new();
    let inside = t.add_device(DeviceKind::Host, "PC1".into(), 0.0, 0.0);
    let r = t.add_device(DeviceKind::Router, "R1".into(), 0.0, 0.0);
    let outside = t.add_device(DeviceKind::Host, "WAN".into(), 0.0, 0.0);
    t.add_link_ports(inside, 0, r, 0);
    let wan = t.add_link_ports(r, 1, outside, 0).unwrap();
    t.set_ip(inside, 0, "10.0.0.2", "255.255.255.0");
    t.set_gateway(inside, "10.0.0.1");
    t.set_ip(r, 0, "10.0.0.1", "255.255.255.0");
    t.set_ip(r, 1, "203.0.113.1", "255.255.255.0");
    t.set_ip(outside, 0, "203.0.113.2", "255.255.255.0");
    t.set_gateway(outside, "203.0.113.1");
    assert!(t.set_nat(r, 0, "inside"));
    assert!(t.set_nat(r, 1, "outside"));
    t.set_capture(wan, true);

    assert!(t.ping(inside, "203.0.113.2"));
    let msgs = run(&mut t, 80);
    assert!(
        msgs.iter().any(|m| m.contains("reply from 203.0.113.2")),
        "nat ping failed\n{}",
        msgs.join("\n")
    );
    let caps = t.drain_captures();
    let saw_masq = caps.iter().any(|c| {
        c.bytes.len() >= 30 && c.bytes[26] == 203 && c.bytes[27] == 0 && c.bytes[28] == 113 && c.bytes[29] == 1
    });
    assert!(saw_masq, "outside link never saw the NAT address");
}

#[test]
fn loss_and_mtu_drop_and_a_slow_link_still_delivers() {
    let mut t = Topology::new();
    let a = t.add_device(DeviceKind::Host, "A".into(), 0.0, 0.0);
    let b = t.add_device(DeviceKind::Host, "B".into(), 0.0, 0.0);
    let l = t.add_link(a, b).unwrap();
    t.set_ip(a, 0, "10.0.0.1", "255.255.255.0");
    t.set_ip(b, 0, "10.0.0.2", "255.255.255.0");

    t.set_link_impair(l, 4, 0, 100, 0, "1G");
    t.ping(a, "10.0.0.2");
    let msgs = run(&mut t, 30);
    assert!(msgs.iter().any(|m| m.contains("impairment")), "{}", msgs.join("\n"));
    assert!(!msgs.iter().any(|m| m.contains("reply from")));

    t.set_link_impair(l, 4, 0, 0, 20, "WAN");
    t.ping(a, "10.0.0.2");
    let msgs = run(&mut t, 20);
    assert!(msgs.iter().any(|m| m.contains("exceeds MTU")), "{}", msgs.join("\n"));

    t.set_link_impair(l, 12, 2, 0, 0, "WAN");
    t.ping(a, "10.0.0.2");
    let msgs = run(&mut t, 80);
    assert!(msgs.iter().any(|m| m.contains("reply from 10.0.0.2")), "{}", msgs.join("\n"));
}

#[test]
fn crafted_frame_is_reinjected_on_a_link() {
    let mut t = Topology::new();
    let a = t.add_device(DeviceKind::Host, "A".into(), 0.0, 0.0);
    let b = t.add_device(DeviceKind::Host, "B".into(), 0.0, 0.0);
    let l = t.add_link(a, b).unwrap();
    t.set_ip(b, 0, "10.0.0.2", "255.255.255.0");
    // Ethernet + ARP request: who-has 10.0.0.2 tell 10.0.0.1, from a known mac
    let mut bytes = vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02, 0, 0, 0, 0, 0x11, 0x08, 0x06];
    bytes.extend_from_slice(&[0, 1, 0x08, 0, 6, 4, 0, 1]);
    bytes.extend_from_slice(&[0x02, 0, 0, 0, 0, 0x11]);
    bytes.extend_from_slice(&[10, 0, 0, 1]);
    bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&[10, 0, 0, 2]);
    assert!(t.inject_link(l, a, &bytes));
    let msgs = run(&mut t, 20);
    assert!(
        msgs.iter().any(|m| m.contains("10.0.0.2") && m.contains("is-at")),
        "{}",
        msgs.join("\n")
    );
}
