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
        msgs.iter()
            .any(|m| m.contains("lab.local") && m.contains("10.1.0.9")),
        "{}",
        msgs.join("\n")
    );

    assert!(t.http_get(pc, "10.1.0.1", 80));
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter().any(|m| m.contains("HTTP 200")),
        "{}",
        msgs.join("\n")
    );

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
        c.bytes.len() >= 30
            && c.bytes[26] == 203
            && c.bytes[27] == 0
            && c.bytes[28] == 113
            && c.bytes[29] == 1
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
    assert!(
        msgs.iter().any(|m| m.contains("impairment")),
        "{}",
        msgs.join("\n")
    );
    assert!(!msgs.iter().any(|m| m.contains("reply from")));

    t.set_link_impair(l, 4, 0, 0, 20, "WAN");
    t.ping(a, "10.0.0.2");
    let msgs = run(&mut t, 20);
    assert!(
        msgs.iter().any(|m| m.contains("exceeds MTU")),
        "{}",
        msgs.join("\n")
    );

    t.set_link_impair(l, 12, 2, 0, 0, "WAN");
    t.ping(a, "10.0.0.2");
    let msgs = run(&mut t, 80);
    assert!(
        msgs.iter().any(|m| m.contains("reply from 10.0.0.2")),
        "{}",
        msgs.join("\n")
    );
}

#[test]
fn crafted_frame_is_reinjected_on_a_link() {
    let mut t = Topology::new();
    let a = t.add_device(DeviceKind::Host, "A".into(), 0.0, 0.0);
    let b = t.add_device(DeviceKind::Host, "B".into(), 0.0, 0.0);
    let l = t.add_link(a, b).unwrap();
    t.set_ip(b, 0, "10.0.0.2", "255.255.255.0");
    // Ethernet + ARP request: who-has 10.0.0.2 tell 10.0.0.1, from a known mac
    let mut bytes = vec![
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02, 0, 0, 0, 0, 0x11, 0x08, 0x06,
    ];
    bytes.extend_from_slice(&[0, 1, 0x08, 0, 6, 4, 0, 1]);
    bytes.extend_from_slice(&[0x02, 0, 0, 0, 0, 0x11]);
    bytes.extend_from_slice(&[10, 0, 0, 1]);
    bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&[10, 0, 0, 2]);
    assert!(t.inject_link(l, a, &bytes));
    let msgs = run(&mut t, 20);
    assert!(
        msgs.iter()
            .any(|m| m.contains("10.0.0.2") && m.contains("is-at")),
        "{}",
        msgs.join("\n")
    );
}

#[test]
fn access_vlans_isolate_and_a_trunk_joins_them() {
    let mut t = Topology::new();
    let a = t.add_device(DeviceKind::Host, "A".into(), 0.0, 0.0);
    let b = t.add_device(DeviceKind::Host, "B".into(), 0.0, 0.0);
    let c = t.add_device(DeviceKind::Host, "C".into(), 0.0, 0.0);
    let sw1 = t.add_device(DeviceKind::Switch, "SW1".into(), 0.0, 0.0);
    let sw2 = t.add_device(DeviceKind::Switch, "SW2".into(), 0.0, 0.0);
    t.add_link_ports(a, 0, sw1, 0);
    t.add_link_ports(c, 0, sw1, 1);
    t.add_link_ports(sw1, 2, sw2, 0);
    t.add_link_ports(b, 0, sw2, 1);
    assert!(t.set_switchport(sw1, 0, false, 10, 1, ""));
    assert!(t.set_switchport(sw1, 1, false, 20, 1, ""));
    assert!(t.set_switchport(sw1, 2, true, 1, 1, ""));
    assert!(t.set_switchport(sw2, 0, true, 1, 1, ""));
    assert!(t.set_switchport(sw2, 1, false, 10, 1, ""));
    t.set_ip(a, 0, "10.0.0.1", "255.255.255.0");
    t.set_ip(b, 0, "10.0.0.2", "255.255.255.0");
    t.set_ip(c, 0, "10.0.0.3", "255.255.255.0");
    t.ping(a, "10.0.0.2");
    let msgs = run(&mut t, 80);
    assert!(
        msgs.iter().any(|m| m.contains("reply from 10.0.0.2")),
        "trunk\n{}",
        msgs.join("\n")
    );
    t.ping(a, "10.0.0.3");
    let msgs = run(&mut t, 30);
    assert!(
        msgs.iter().all(|m| !m.contains("reply from 10.0.0.3")),
        "isolated\n{}",
        msgs.join("\n")
    );
}

#[test]
fn extended_acl_denies_then_permits_icmp() {
    let mut t = Topology::new();
    let a = t.add_device(DeviceKind::Host, "A".into(), 0.0, 0.0);
    let r = t.add_device(DeviceKind::Router, "R".into(), 0.0, 0.0);
    let b = t.add_device(DeviceKind::Host, "B".into(), 0.0, 0.0);
    t.add_link_ports(a, 0, r, 0);
    t.add_link_ports(r, 1, b, 0);
    t.set_ip(a, 0, "10.1.0.2", "255.255.255.0");
    t.set_gateway(a, "10.1.0.1");
    t.set_ip(r, 0, "10.1.0.1", "255.255.255.0");
    t.set_ip(r, 1, "10.2.0.1", "255.255.255.0");
    t.set_ip(b, 0, "10.2.0.2", "255.255.255.0");
    t.set_gateway(b, "10.2.0.1");
    let any = "0.0.0.0";
    let wild = "255.255.255.255";
    assert!(t.acl_add(r, 100, false, 1, any, wild, any, wild, 0, 0));
    assert!(t.set_access_group(r, 0, 100, "in"));
    t.ping(a, "10.2.0.2");
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter().any(|m| m.contains("ACL denied")),
        "deny\n{}",
        msgs.join("\n")
    );
    assert!(msgs.iter().all(|m| !m.contains("reply from 10.2.0.2")));
    t.acl_clear(r, 100);
    assert!(t.acl_add(r, 100, true, 1, any, wild, any, wild, 0, 0));
    assert!(t.set_access_group(r, 0, 100, "in"));
    t.ping(a, "10.2.0.2");
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter().any(|m| m.contains("reply from 10.2.0.2")),
        "permit\n{}",
        msgs.join("\n")
    );
}

#[test]
fn dhcp_relay_leases_across_a_router() {
    let mut t = Topology::new();
    let pc = t.add_device(DeviceKind::Host, "PC".into(), 0.0, 0.0);
    let r = t.add_device(DeviceKind::Router, "R".into(), 0.0, 0.0);
    let srv = t.add_device(DeviceKind::Host, "SRV".into(), 0.0, 0.0);
    t.add_link_ports(pc, 0, r, 0);
    t.add_link_ports(r, 1, srv, 0);
    t.set_ip(r, 0, "10.1.0.1", "255.255.255.0");
    t.set_ip(r, 1, "10.2.0.1", "255.255.255.0");
    t.set_ip(srv, 0, "10.2.0.10", "255.255.255.0");
    t.set_gateway(srv, "10.2.0.1");
    t.set_services(srv, true, false, false);
    assert!(t.set_helper(r, 0, "10.2.0.10"));
    assert!(t.dhcp_client(pc, 0));
    let msgs = run(&mut t, 200);
    assert_eq!(
        port_ip(&t, pc).as_deref(),
        Some("10.1.0.10"),
        "{}",
        msgs.join("\n")
    );
}

#[test]
fn ospf_installs_a_remote_prefix_and_static_wins() {
    let mut t = Topology::new();
    let r1 = t.add_device(DeviceKind::Router, "R1".into(), 0.0, 0.0);
    let r2 = t.add_device(DeviceKind::Router, "R2".into(), 0.0, 0.0);
    t.add_link_ports(r1, 1, r2, 0);
    t.set_ip(r1, 0, "10.1.0.1", "255.255.255.0");
    t.set_ip(r1, 1, "10.0.0.1", "255.255.255.252");
    t.set_ip(r2, 0, "10.0.0.2", "255.255.255.252");
    t.set_ip(r2, 1, "10.2.0.1", "255.255.255.0");
    assert!(t.ospf_network(r1, "10.0.0.0", "0.255.255.255", 0));
    assert!(t.ospf_network(r2, "10.0.0.0", "0.255.255.255", 0));
    let msgs = run(&mut t, 30);
    let has = t
        .snapshot()
        .devices
        .into_iter()
        .find(|d| d.id == r2)
        .unwrap()
        .routes
        .iter()
        .any(|r| r.prefix == "10.1.0.0" && r.proto == "O");
    assert!(has, "no OSPF route\n{}", msgs.join("\n"));
    t.ping(r2, "10.1.0.1");
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter().any(|m| m.contains("reply from 10.1.0.1")),
        "ospf ping\n{}",
        msgs.join("\n")
    );
    t.add_route(r2, "10.1.0.0", "255.255.255.0", "10.0.0.9", 0);
    t.ping(r2, "10.1.0.1");
    let msgs = run(&mut t, 30);
    assert!(
        msgs.iter().all(|m| !m.contains("reply from 10.1.0.1")),
        "static should beat OSPF\n{}",
        msgs.join("\n")
    );
}

fn nat_lab() -> (Topology, u32, u32, u32, u32) {
    let mut t = Topology::new();
    let pc = t.add_device(DeviceKind::Host, "PC".into(), 0.0, 0.0);
    let web = t.add_device(DeviceKind::Host, "Web".into(), 0.0, 0.0);
    let sw = t.add_device(DeviceKind::Switch, "SW".into(), 0.0, 0.0);
    let r = t.add_device(DeviceKind::Router, "R1".into(), 0.0, 0.0);
    let wan = t.add_device(DeviceKind::Host, "WAN".into(), 0.0, 0.0);
    t.add_link_ports(pc, 0, sw, 0);
    t.add_link_ports(web, 0, sw, 1);
    t.add_link_ports(sw, 2, r, 0);
    t.add_link_ports(r, 1, wan, 0);
    t.set_ip(pc, 0, "10.0.0.2", "255.255.255.0");
    t.set_gateway(pc, "10.0.0.1");
    t.set_ip(web, 0, "10.0.0.10", "255.255.255.0");
    t.set_gateway(web, "10.0.0.1");
    t.set_services(web, false, false, true);
    t.set_ip(r, 0, "10.0.0.1", "255.255.255.0");
    t.set_ip(r, 1, "203.0.113.1", "255.255.255.0");
    t.set_ip(wan, 0, "203.0.113.2", "255.255.255.0");
    t.set_gateway(wan, "203.0.113.1");
    assert!(t.set_nat(r, 0, "inside"));
    assert!(t.set_nat(r, 1, "outside"));
    (t, pc, web, r, wan)
}

#[test]
fn static_nat_makes_the_inside_host_answer_for_the_outside_address() {
    let (mut t, pc, _web, r, wan) = nat_lab();
    assert!(t.nat_static(r, "10.0.0.2", 0, 0, 0));
    t.ping(wan, "203.0.113.1");
    let msgs = run(&mut t, 80);
    assert!(
        msgs.iter().any(|m| m.contains("PC: reply to 203.0.113.2")),
        "inside should answer\n{}",
        msgs.join("\n")
    );
    assert!(
        msgs.iter().all(|m| !m.contains("R1: reply to")),
        "router should not answer\n{}",
        msgs.join("\n")
    );
    assert!(
        msgs.iter().any(|m| m.contains("reply from 203.0.113.1")),
        "wan should see the outside address\n{}",
        msgs.join("\n")
    );
    t.set_power(r, false);
    let kept = t
        .snapshot()
        .devices
        .into_iter()
        .find(|d| d.id == r)
        .unwrap()
        .static_nat;
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].inside, "10.0.0.2");
    let _ = pc;
}

#[test]
fn port_forward_delivers_http_to_the_inside_server() {
    let (mut t, _pc, _web, r, wan) = nat_lab();
    assert!(t.nat_static(r, "10.0.0.10", 6, 80, 80));
    assert!(t.http_get(wan, "203.0.113.1", 80));
    let msgs = run(&mut t, 120);
    assert!(
        msgs.iter().any(|m| m.contains("HTTP 200")),
        "port forward\n{}",
        msgs.join("\n")
    );
    t.ping(wan, "203.0.113.1");
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter().any(|m| m.contains("R1: reply to")),
        "port forward must leave ICMP on the router\n{}",
        msgs.join("\n")
    );
}

#[test]
fn traceroute_names_each_hop_then_the_destination() {
    let mut t = Topology::new();
    let pc = t.add_device(DeviceKind::Host, "PC".into(), 0.0, 0.0);
    let r1 = t.add_device(DeviceKind::Router, "R1".into(), 0.0, 0.0);
    let r2 = t.add_device(DeviceKind::Router, "R2".into(), 0.0, 0.0);
    let pc2 = t.add_device(DeviceKind::Host, "PC2".into(), 0.0, 0.0);
    t.add_link_ports(pc, 0, r1, 0);
    t.add_link_ports(r1, 1, r2, 0);
    t.add_link_ports(r2, 1, pc2, 0);
    t.set_ip(pc, 0, "10.1.0.2", "255.255.255.0");
    t.set_gateway(pc, "10.1.0.1");
    t.set_ip(r1, 0, "10.1.0.1", "255.255.255.0");
    t.set_ip(r1, 1, "10.0.0.1", "255.255.255.252");
    t.set_ip(r2, 0, "10.0.0.2", "255.255.255.252");
    t.set_ip(r2, 1, "10.2.0.1", "255.255.255.0");
    t.set_ip(pc2, 0, "10.2.0.2", "255.255.255.0");
    t.set_gateway(pc2, "10.2.0.1");
    t.add_route(r1, "10.2.0.0", "255.255.255.0", "10.0.0.2", 1);
    t.add_route(r2, "10.1.0.0", "255.255.255.0", "10.0.0.1", 0);

    assert!(t.ping_ttl(pc, "10.2.0.2", 1));
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter()
            .any(|m| m.contains("time exceeded from 10.1.0.1")),
        "ttl 1\n{}",
        msgs.join("\n")
    );

    assert!(t.ping_ttl(pc, "10.2.0.2", 2));
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter()
            .any(|m| m.contains("time exceeded from 10.0.0.2")),
        "ttl 2\n{}",
        msgs.join("\n")
    );

    assert!(t.ping_ttl(pc, "10.2.0.2", 3));
    let msgs = run(&mut t, 40);
    assert!(
        msgs.iter().any(|m| m.contains("reply from 10.2.0.2")),
        "ttl 3\n{}",
        msgs.join("\n")
    );
}
