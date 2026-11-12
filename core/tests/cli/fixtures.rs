//! Six named Topology rigs. Seeds land through `crate::config` setters
//! plus link wiring at build time — never through cli_script and never
//! into learned tables. Seed-time events are drained so the asserted
//! window opens clean.

use std::collections::HashMap;

use netlab_core::{config, Device, DeviceId, DeviceKind, Topology};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopoId {
    SingleRouter,
    SingleSwitch,
    TwoRouterLink,
    SwitchHosts,
    OspfPair,
    NatDhcp,
}

pub fn topo_id(name: &str) -> Option<TopoId> {
    Some(match name {
        "single-router" => TopoId::SingleRouter,
        "single-switch" => TopoId::SingleSwitch,
        "two-router-link" => TopoId::TwoRouterLink,
        "switch-hosts" => TopoId::SwitchHosts,
        "ospf-pair" => TopoId::OspfPair,
        "nat-dhcp" => TopoId::NatDhcp,
        _ => return None,
    })
}

const MASK_24: [u8; 4] = [255, 255, 255, 0];

fn dev(m: &mut HashMap<DeviceId, Device>, id: DeviceId) -> &mut Device {
    m.get_mut(&id).expect("fixture device")
}

fn router(topo: &mut Topology, name: &str, x: f64) -> DeviceId {
    topo.add_device(DeviceKind::Router, name.to_string(), x, 0.0)
}

fn seed_r2(topo: &mut Topology, r2: DeviceId) {
    let tick = topo.tick;
    let d = dev(&mut topo.devices, r2);
    config::set_ip(d, 0, [10, 0, 0, 2], MASK_24, tick).expect("seed r2 ip");
    config::set_admin(d, 0, true, tick, &mut topo.events).expect("seed r2 up");
}

fn seed_ospf_side(topo: &mut Topology, r: DeviceId, last: u8) {
    let tick = topo.tick;
    let d = dev(&mut topo.devices, r);
    config::set_ip(d, 0, [10, 0, 0, last], MASK_24, tick).expect("seed ip");
    config::set_admin(d, 0, true, tick, &mut topo.events).expect("seed up");
    config::ospf_enter(d, 1).expect("seed ospf");
    config::ospf_net_add(d, [10, 0, 0, 0], [0, 0, 0, 255], 0).expect("seed net");
}

/// Returns the built rig plus the FIRST device in declared order whose
/// kind matches `kind` — the session device.
pub fn build(topo: TopoId, kind: DeviceKind) -> Result<(Topology, DeviceId), String> {
    let mut t = Topology::new();
    let order: Vec<DeviceId> = match topo {
        TopoId::SingleRouter => vec![router(&mut t, "R", 0.0)],
        TopoId::SingleSwitch => {
            vec![t.add_device(DeviceKind::Switch, "SW".to_string(), 0.0, 0.0)]
        }
        TopoId::TwoRouterLink => {
            let r1 = router(&mut t, "R1", 0.0);
            let r2 = router(&mut t, "R2", 100.0);
            assert!(t.add_link_ports(r1, 0, r2, 0).is_some(), "wire r1-r2");
            seed_r2(&mut t, r2);
            vec![r1, r2]
        }
        TopoId::SwitchHosts => {
            let sw = t.add_device(DeviceKind::Switch, "SW".to_string(), 0.0, 0.0);
            let h1 = t.add_device(DeviceKind::Host, "PC1".to_string(), 100.0, 0.0);
            let h2 = t.add_device(DeviceKind::Host, "PC2".to_string(), 200.0, 0.0);
            assert!(t.add_link_ports(h1, 0, sw, 0).is_some(), "wire h1-sw");
            assert!(t.add_link_ports(h2, 0, sw, 1).is_some(), "wire h2-sw");
            {
                let d = dev(&mut t.devices, sw);
                config::enter_vlan(d, 10).expect("seed vlan10");
                config::set_vlan_name(d, 10, "blue").expect("seed vlan name");
                config::set_sw_access_vlan(d, 2, 10).expect("seed fa0/3 access");
            }
            vec![sw, h1, h2]
        }
        TopoId::OspfPair => {
            let r1 = router(&mut t, "R1", 0.0);
            let r2 = router(&mut t, "R2", 100.0);
            assert!(t.add_link_ports(r1, 0, r2, 0).is_some(), "wire r1-r2");
            seed_ospf_side(&mut t, r1, 1);
            seed_ospf_side(&mut t, r2, 2);
            {
                let tick = t.tick;
                let d = dev(&mut t.devices, r1);
                config::add_route(d, [10, 9, 0, 0], MASK_24, [10, 0, 0, 2], None, tick)
                    .expect("seed r1 static");
            }
            vec![r1, r2]
        }
        TopoId::NatDhcp => {
            let r = router(&mut t, "R", 0.0);
            let h = t.add_device(DeviceKind::Host, "PC".to_string(), 100.0, 0.0);
            assert!(t.add_link_ports(h, 0, r, 0).is_some(), "wire h-r");
            {
                let tick = t.tick;
                let d = dev(&mut t.devices, r);
                config::set_ip(d, 0, [192, 168, 1, 1], MASK_24, tick).expect("seed g0/0");
                config::set_admin(d, 0, true, tick, &mut t.events).expect("seed g0/0 up");
                config::set_nat_role(d, 0, netlab_core::device::NatRole::Inside)
                    .expect("seed nat inside");
                config::set_ip(d, 1, [203, 0, 113, 1], MASK_24, tick).expect("seed g0/1");
                config::set_admin(d, 1, true, tick, &mut t.events).expect("seed g0/1 up");
                config::set_nat_role(d, 1, netlab_core::device::NatRole::Outside)
                    .expect("seed nat outside");
                config::set_dhcp_service(d, true).expect("seed dhcp svc");
                config::dhcp_excluded_add(d, [192, 168, 1, 1], None).expect("seed excl");
            }
            vec![r, h]
        }
    };
    let Some(dev_id) = order
        .iter()
        .copied()
        .find(|id| t.devices[id].kind == kind)
    else {
        return Err(format!(
            "topo {} has no device of kind {kind:?}",
            topo_name(topo)
        ));
    };
    t.drain_events();
    Ok((t, dev_id))
}

pub fn topo_name(t: TopoId) -> &'static str {
    match t {
        TopoId::SingleRouter => "single-router",
        TopoId::SingleSwitch => "single-switch",
        TopoId::TwoRouterLink => "two-router-link",
        TopoId::SwitchHosts => "switch-hosts",
        TopoId::OspfPair => "ospf-pair",
        TopoId::NatDhcp => "nat-dhcp",
    }
}
