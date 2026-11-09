// Lab controls the UI depends on: unplugging cables, stopping nodes,
// wiring specific interfaces, deleting nodes.

use netlab_core::{DeviceKind, Topology};

fn lab() -> (Topology, u32, u32, u32, u32) {
    let mut t = Topology::new();
    let pc1 = t.add_device(DeviceKind::Host, "PC1".into(), 0.0, 0.0);
    let sw = t.add_device(DeviceKind::Switch, "SW1".into(), 0.0, 0.0);
    let pc2 = t.add_device(DeviceKind::Host, "PC2".into(), 0.0, 0.0);
    t.add_link_ports(pc1, 0, sw, 3).unwrap();
    let l2 = t.add_link_ports(sw, 5, pc2, 0).unwrap();
    t.set_ip(pc1, 0, "10.0.0.1", "255.255.255.0");
    t.set_ip(pc2, 0, "10.0.0.2", "255.255.255.0");
    (t, pc1, sw, pc2, l2)
}

fn run(t: &mut Topology) -> Vec<String> {
    for _ in 0..60 {
        t.step();
    }
    t.drain_events().into_iter().map(|e| e.msg).collect()
}

fn replied(msgs: &[String]) -> bool {
    msgs.iter().any(|m| m.contains("reply from 10.0.0.2"))
}

#[test]
fn ping_works_over_chosen_ports() {
    let (mut t, pc1, _, _, _) = lab();
    assert!(t.ping(pc1, "10.0.0.2"));
    assert!(replied(&run(&mut t)));
}

#[test]
fn busy_or_missing_port_is_refused() {
    let (mut t, _, sw, pc2, _) = lab();
    let pc3 = t.add_device(DeviceKind::Host, "PC3".into(), 0.0, 0.0);
    assert!(t.add_link_ports(sw, 3, pc3, 0).is_none(), "switch port 3 already used");
    assert!(t.add_link_ports(pc2, 0, pc3, 0).is_none(), "pc2 eth0 already used");
    assert!(t.add_link_ports(sw, 99, pc3, 0).is_none(), "no such port");
}

#[test]
fn unplugged_cable_drops_traffic_and_replug_restores_it() {
    let (mut t, pc1, _, _, l2) = lab();
    t.set_link_up(l2, false);
    t.ping(pc1, "10.0.0.2");
    let msgs = run(&mut t);
    assert!(!replied(&msgs));
    assert!(msgs.iter().any(|m| m.contains("unplugged")), "drop must be explained:\n{}", msgs.join("\n"));

    t.set_link_up(l2, true);
    t.ping(pc1, "10.0.0.2");
    assert!(replied(&run(&mut t)));
}

#[test]
fn stopped_node_neither_sends_nor_answers() {
    let (mut t, pc1, _, pc2, _) = lab();
    t.set_power(pc2, false);
    t.ping(pc1, "10.0.0.2");
    let msgs = run(&mut t);
    assert!(!replied(&msgs));
    assert!(msgs.iter().any(|m| m.contains("PC2 is stopped")));

    t.set_power(pc1, false);
    assert!(!t.ping(pc1, "10.0.0.2"), "a stopped node cannot originate a ping");
}

#[test]
fn deleting_a_node_frees_the_peer_port() {
    let (mut t, _, sw, pc2, _) = lab();
    t.remove_device(pc2);
    let pc3 = t.add_device(DeviceKind::Host, "PC3".into(), 0.0, 0.0);
    assert!(t.add_link_ports(sw, 5, pc3, 0).is_some(), "SW1 port 5 should be free again");
}
