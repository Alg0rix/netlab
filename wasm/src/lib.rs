use netlab_core::{DeviceKind, Topology};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct NetLab {
    topo: Topology,
}

#[wasm_bindgen]
impl NetLab {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            topo: Topology::new(),
        }
    }

    /// kind: "host" | "switch" | "router" | "ext"
    pub fn add_device(&mut self, kind: &str, name: &str, x: f64, y: f64) -> u32 {
        let kind = match kind {
            "switch" => DeviceKind::Switch,
            "router" => DeviceKind::Router,
            "ext" => DeviceKind::Ext,
            _ => DeviceKind::Host,
        };
        self.topo.add_device(kind, name.to_string(), x, y)
    }

    pub fn remove_device(&mut self, id: u32) {
        self.topo.remove_device(id);
    }

    pub fn move_device(&mut self, id: u32, x: f64, y: f64) {
        self.topo.move_device(id, x, y);
    }

    /// Returns link id, or -1 if either device has no free port.
    pub fn add_link(&mut self, a: u32, b: u32) -> i32 {
        self.topo.add_link(a, b).map(|l| l as i32).unwrap_or(-1)
    }

    /// Connect specific ports; -1 if a port is missing or busy.
    pub fn add_link_ports(&mut self, a: u32, pa: usize, b: u32, pb: usize) -> i32 {
        self.topo.add_link_ports(a, pa, b, pb).map(|l| l as i32).unwrap_or(-1)
    }

    pub fn remove_link(&mut self, id: u32) {
        self.topo.remove_link(id);
    }

    pub fn rename_device(&mut self, id: u32, name: &str) {
        self.topo.rename_device(id, name.to_string());
    }

    pub fn set_link_up(&mut self, id: u32, up: bool) {
        self.topo.set_link_up(id, up);
    }

    pub fn set_power(&mut self, id: u32, on: bool) {
        self.topo.set_power(id, on);
    }

    pub fn set_capture(&mut self, id: u32, on: bool) {
        self.topo.set_capture(id, on);
    }

    /// Captured frames since last call: array of {link, tick, from, bytes}.
    pub fn drain_captures(&mut self) -> js_sys::Array {
        let out = js_sys::Array::new();
        for c in self.topo.drain_captures() {
            let o = js_sys::Object::new();
            js_sys::Reflect::set(&o, &"link".into(), &(c.link as f64).into()).ok();
            js_sys::Reflect::set(&o, &"tick".into(), &(c.tick as f64).into()).ok();
            js_sys::Reflect::set(&o, &"from".into(), &(c.from as f64).into()).ok();
            js_sys::Reflect::set(&o, &"bytes".into(), &js_sys::Uint8Array::from(&c.bytes[..]).into()).ok();
            out.push(&o);
        }
        out
    }

    pub fn clear_ip(&mut self, dev: u32, port: usize) {
        self.topo.clear_ip(dev, port);
    }

    pub fn remove_route(&mut self, dev: u32, idx: usize) {
        self.topo.remove_route(dev, idx);
    }

    pub fn set_ip(&mut self, dev: u32, port: usize, ip: &str, mask: &str) -> bool {
        self.topo.set_ip(dev, port, ip, mask)
    }

    pub fn set_gateway(&mut self, dev: u32, gw: &str) -> bool {
        self.topo.set_gateway(dev, gw)
    }

    pub fn add_route(
        &mut self,
        dev: u32,
        prefix: &str,
        mask: &str,
        next_hop: &str,
        out_port: usize,
    ) -> bool {
        self.topo.add_route(dev, prefix, mask, next_hop, out_port)
    }

    pub fn ping(&mut self, src: u32, dst_ip: &str) -> bool {
        self.topo.ping(src, dst_ip)
    }

    /// ICMP echo with TTL. Used by traceroute.
    pub fn probe_ttl(&mut self, src: u32, dst_ip: &str, ttl: u8) -> bool {
        self.topo.ping_ttl(src, dst_ip, ttl)
    }

    pub fn tick(&mut self) {
        self.topo.step();
    }

    /// JSON array of events since the last call.
    pub fn drain_events(&mut self) -> String {
        serde_json::to_string(&self.topo.drain_events()).unwrap_or_else(|_| "[]".into())
    }

    /// Inject real Ethernet wire bytes from an emulated node into an ext
    /// device port.
    pub fn ext_inject(&mut self, dev: u32, port: usize, bytes: &[u8]) {
        self.topo.ext_inject(dev, port, bytes);
    }

    /// Drain wire bytes bound for emulated nodes.
    /// Returns a JS array of {dev, port, bytes: Uint8Array}.
    pub fn drain_ext(&mut self) -> js_sys::Array {
        let out = js_sys::Array::new();
        for (dev, port, bytes) in self.topo.drain_ext() {
            let o = js_sys::Object::new();
            js_sys::Reflect::set(&o, &"dev".into(), &(dev as f64).into()).ok();
            js_sys::Reflect::set(&o, &"port".into(), &(port as f64).into()).ok();
            js_sys::Reflect::set(&o, &"bytes".into(), &js_sys::Uint8Array::from(&bytes[..]).into())
                .ok();
            out.push(&o);
        }
        out
    }

    /// Link impairment. `delay` and `jitter` are ticks (1 ms in Realtime).
    /// `loss` is a percent. `mtu` 0 means no limit.
    /// `speed` is "10M" | "100M" | "1G" | "10G" | "WAN".
    pub fn set_link_impair(&mut self, id: u32, delay: u32, jitter: u32, loss: u8, mtu: u16, speed: &str) -> bool {
        self.topo.set_link_impair(id, delay, jitter, loss, mtu, speed)
    }

    /// Place a raw Ethernet frame onto a link, travelling away from `from`.
    pub fn inject_link(&mut self, link: u32, from: u32, bytes: &[u8]) -> bool {
        self.topo.inject_link(link, from, bytes)
    }

    pub fn set_services(&mut self, dev: u32, dhcp: bool, dns: bool, http: bool) {
        self.topo.set_services(dev, dhcp, dns, http);
    }

    pub fn set_dns(&mut self, dev: u32, ip: &str) -> bool {
        self.topo.set_dns(dev, ip)
    }

    pub fn add_dns_record(&mut self, dev: u32, name: &str, ip: &str) -> bool {
        self.topo.add_dns_record(dev, name, ip)
    }

    pub fn dhcp_client(&mut self, dev: u32, port: usize) -> bool {
        self.topo.dhcp_client(dev, port)
    }

    pub fn dns_query(&mut self, dev: u32, name: &str) -> bool {
        self.topo.dns_query(dev, name)
    }

    pub fn probe_tcp(&mut self, dev: u32, dst: &str, port: u16) -> bool {
        self.topo.probe_tcp(dev, dst, port)
    }

    pub fn probe_udp(&mut self, dev: u32, dst: &str, port: u16, payload: &str) -> bool {
        self.topo.probe_udp(dev, dst, port, payload)
    }

    pub fn http_get(&mut self, dev: u32, dst: &str, port: u16) -> bool {
        self.topo.http_get(dev, dst, port)
    }

    /// role: "inside" | "outside" | "off"
    pub fn set_nat(&mut self, dev: u32, port: usize, role: &str) -> bool {
        self.topo.set_nat(dev, port, role)
    }

    /// `proto` is "ip" (1:1) or "tcp" / "udp" (static port forward).
    pub fn nat_static(&mut self, dev: u32, inside: &str, proto: &str, inside_port: u16, outside_port: u16) -> bool {
        let proto = match proto {
            "tcp" => 6,
            "udp" => 17,
            "ip" | "" => 0,
            _ => return false,
        };
        self.topo.nat_static(dev, inside, proto, inside_port, outside_port)
    }

    pub fn nat_static_clear(&mut self, dev: u32, inside: &str, proto: &str, outside_port: u16) -> bool {
        let proto = match proto {
            "tcp" => 6,
            "udp" => 17,
            "ip" | "" => 0,
            _ => return false,
        };
        self.topo.nat_static_clear(dev, inside, proto, outside_port)
    }

    /// `mode` is "access" or "trunk". `allowed` is "" / "all" or "1,10,20".
    pub fn set_switchport(&mut self, dev: u32, port: usize, mode: &str, access: u16, native: u16, allowed: &str) -> bool {
        self.topo.set_switchport(dev, port, mode == "trunk", access, native, allowed)
    }

    /// Empty `ip` clears the helper.
    pub fn set_helper(&mut self, dev: u32, port: usize, ip: &str) -> bool {
        self.topo.set_helper(dev, port, ip)
    }

    /// `proto` is "ip", "icmp", "tcp" or "udp". Port 0 means any.
    pub fn acl_add(
        &mut self,
        dev: u32,
        num: u16,
        permit: bool,
        proto: &str,
        src: &str,
        src_wild: &str,
        dst: &str,
        dst_wild: &str,
        sport: u16,
        dport: u16,
    ) -> bool {
        let proto = match proto {
            "icmp" => 1,
            "tcp" => 6,
            "udp" => 17,
            "ip" | "any" => 0,
            _ => return false,
        };
        self.topo.acl_add(dev, num, permit, proto, src, src_wild, dst, dst_wild, sport, dport)
    }

    pub fn acl_clear(&mut self, dev: u32, num: u16) {
        self.topo.acl_clear(dev, num);
    }

    /// `dir` is "in", "out", "off-in" or "off-out".
    pub fn set_access_group(&mut self, dev: u32, port: usize, num: u16, dir: &str) -> bool {
        self.topo.set_access_group(dev, port, num, dir)
    }

    pub fn ospf_network(&mut self, dev: u32, prefix: &str, wild: &str, area: u32) -> bool {
        self.topo.ospf_network(dev, prefix, wild, area)
    }

    /// `area` < 0 clears `ip ospf` on that interface.
    pub fn ospf_interface(&mut self, dev: u32, port: usize, area: i32) -> bool {
        self.topo.ospf_interface(dev, port, if area < 0 { None } else { Some(area as u32) })
    }

    pub fn ospf_clear(&mut self, dev: u32) {
        self.topo.ospf_clear(dev);
    }

    /// JSON snapshot of the whole topology for rendering.
    pub fn snapshot(&self) -> String {
        serde_json::to_string(&self.topo.snapshot()).unwrap_or_else(|_| "{}".into())
    }
}
