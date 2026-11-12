use netlab_core::{DeviceKind, Topology};

fn host_switch_host() -> (Topology, u32, u32) {
    let mut t = Topology::new();
    let pc1 = t.add_device(DeviceKind::Host, "PC1".into(), 0.0, 0.0);
    let sw = t.add_device(DeviceKind::Switch, "SW1".into(), 0.0, 0.0);
    let pc2 = t.add_device(DeviceKind::Host, "PC2".into(), 0.0, 0.0);
    t.add_link(pc1, sw);
    t.add_link(sw, pc2);
    t.set_ip(pc1, 0, "10.0.0.1", "255.255.255.0");
    t.set_ip(pc2, 0, "10.0.0.2", "255.255.255.0");
    (t, pc1, pc2)
}

#[test]
fn ping_through_switch_completes() {
    let (mut t, pc1, pc2) = host_switch_host();
    assert!(t.ping(pc1, "10.0.0.2"));
    for _ in 0..40 {
        t.step();
    }
    let msgs: Vec<String> = t.events.iter().map(|e| e.msg.clone()).collect();
    assert!(
        msgs.iter().any(|m| m == "!"),
        "no probe succeeded.\n{}",
        msgs.join("\n")
    );
    // the reply path ARP-resolved the initiator on the far host
    let learned = t
        .snapshot()
        .devices
        .into_iter()
        .find(|d| d.id == pc2)
        .unwrap()
        .arp_table
        .iter()
        .any(|a| a.ip == "10.0.0.1");
    assert!(learned, "the echo never reached PC2");
}

#[test]
fn arp_resolution_happens_before_icmp() {
    let (mut t, pc1, _pc2) = host_switch_host();
    t.ping(pc1, "10.0.0.2");
    for _ in 0..30 {
        t.step();
    }
    let learned = t
        .snapshot()
        .devices
        .into_iter()
        .find(|d| d.id == pc1)
        .unwrap()
        .arp_table
        .iter()
        .any(|a| a.ip == "10.0.0.2");
    assert!(learned, "the peer's address was never ARP-resolved");
}

#[test]
fn ping_routes_across_router() {
    let mut t = Topology::new();
    let pc1 = t.add_device(DeviceKind::Host, "PC1".into(), 0.0, 0.0);
    let r1 = t.add_device(DeviceKind::Router, "R1".into(), 0.0, 0.0);
    let pc2 = t.add_device(DeviceKind::Host, "PC2".into(), 0.0, 0.0);
    t.add_link(pc1, r1);
    t.add_link(r1, pc2);
    t.set_ip(pc1, 0, "10.0.0.1", "255.255.255.0");
    t.set_gateway(pc1, "10.0.0.254");
    t.set_ip(r1, 0, "10.0.0.254", "255.255.255.0");
    t.set_ip(r1, 1, "192.168.0.254", "255.255.255.0");
    t.set_admin(r1, 0, true);
    t.set_admin(r1, 1, true);
    t.set_ip(pc2, 0, "192.168.0.1", "255.255.255.0");
    t.set_gateway(pc2, "192.168.0.254");

    assert!(t.ping(pc1, "192.168.0.1"));
    for _ in 0..80 {
        t.step();
    }
    let msgs: Vec<String> = t.events.iter().map(|e| e.msg.clone()).collect();
    assert!(
        msgs.iter().any(|m| m == "!"),
        "routed ping failed.\n{}",
        msgs.join("\n")
    );
}
