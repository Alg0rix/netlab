use std::collections::HashMap;

use netlab_core::cli::{CompleteResult, HelpResult, ModeCtx, ModeId};
use netlab_core::{Cli, Device, DeviceId, DeviceKind, Topology};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct NetLab {
    topo: Topology,
    sessions: HashMap<DeviceId, Cli>,
    /// Per-device authored baseline — the snapshot `baseline_commit` last
    /// took. `reset_config` restores it; it lives exactly while its device
    /// lives. Seeded at `add_device` so a runtime-created node resets to
    /// factory defaults.
    baseline: HashMap<DeviceId, Device>,
}

impl Default for NetLab {
    fn default() -> Self {
        Self::new()
    }
}

fn missing_session(dev: DeviceId) -> JsValue {
    #[cfg(target_arch = "wasm32")]
    return JsValue::from_str(&format!("netlab: device {dev} was deleted"));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = dev;
        JsValue::NULL
    }
}

/// The bad-fields Err domain — `netlab: bad device fields: <why>` on the
/// wire; JsValue::from_str has no native impl, so tests get NULL.
fn bad_fields(msg: impl std::fmt::Display) -> JsValue {
    #[cfg(target_arch = "wasm32")]
    return JsValue::from_str(&format!("netlab: bad device fields: {msg}"));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = msg;
        JsValue::NULL
    }
}

#[wasm_bindgen]
impl NetLab {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            topo: Topology::new(),
            sessions: HashMap::new(),
            baseline: HashMap::new(),
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
        let id = self.topo.add_device(kind, name.to_string(), x, y);
        self.sessions.insert(id, Cli::new());
        self.baseline.insert(id, self.topo.devices[&id].clone());
        id
    }

    pub fn remove_device(&mut self, id: u32) {
        self.sessions.remove(&id);
        self.baseline.remove(&id);
        self.topo.remove_device(id);
    }

    /// Wholesale lab reset — frees every session; in-flight adapter
    /// references degrade to the missing-session Err.
    pub fn restore(&mut self) {
        self.topo = Topology::new();
        self.sessions.clear();
        self.baseline.clear();
    }

    /// Snapshot the device's current config as its authored baseline —
    /// call it once the lab document has finished applying one node.
    pub fn baseline_commit(&mut self, dev: u32) -> Result<(), JsValue> {
        let Some(d) = self.topo.devices.get(&dev) else {
            return Err(missing_session(dev));
        };
        self.baseline.insert(dev, d.clone());
        Ok(())
    }

    /// Reload replay target: the device returns to its authored baseline
    /// — the snapshot `baseline_commit` last took. Learned state clears,
    /// carrier bindings stay live, and the session seat returns to Exec
    /// with an empty context and no dialog.
    pub fn reset_config(&mut self, dev: u32) -> Result<(), JsValue> {
        if !self.sessions.contains_key(&dev) {
            return Err(missing_session(dev));
        }
        let Some(base) = self.baseline.get(&dev).cloned() else {
            return Err(missing_session(dev));
        };
        let tick = self.topo.tick;
        let Some(d) = self.topo.devices.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        d.restore_baseline(&base, tick);
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        cli.pending = None;
        cli.node = ModeId::Exec;
        cli.ctx = ModeCtx::default();
        Ok(())
    }

    pub fn cli_exec(&mut self, dev: u32, line: &str) -> Result<JsValue, JsValue> {
        let json = self.cli_exec_json(dev, line)?;
        Ok(js_sys::JSON::parse(&json).expect_throw("cli_exec wire encode"))
    }

    pub fn cli_key(&mut self, dev: u32, op: &str) -> Result<JsValue, JsValue> {
        let json = self.cli_key_json(dev, op)?;
        Ok(js_sys::JSON::parse(&json).expect_throw("cli_key wire encode"))
    }

    pub fn cli_help(&mut self, dev: u32, before_cursor: &str) -> Result<JsValue, JsValue> {
        let json = self.cli_help_json(dev, before_cursor)?;
        Ok(js_sys::JSON::parse(&json).expect_throw("cli_help wire encode"))
    }

    pub fn cli_complete(&mut self, dev: u32, before_cursor: &str) -> Result<JsValue, JsValue> {
        let json = self.cli_complete_json(dev, before_cursor)?;
        Ok(js_sys::JSON::parse(&json).expect_throw("cli_complete wire encode"))
    }

    pub fn cli_prompt(&mut self, dev: u32) -> Result<String, JsValue> {
        let Some(d) = self.topo.devices.get(&dev) else {
            return Err(missing_session(dev));
        };
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        Ok(cli.current_prompt(d))
    }

    pub fn cli_script(&mut self, dev: u32, text: &str) -> Result<JsValue, JsValue> {
        let json = self.cli_script_json(dev, text)?;
        Ok(js_sys::JSON::parse(&json).expect_throw("cli_script wire encode"))
    }

    pub fn cli_set_ifnames(&mut self, dev: u32, names: Vec<String>) -> Result<(), JsValue> {
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        cli.set_ifmap(&names)
            .expect_throw("non-canonical interface name");
        Ok(())
    }

    pub fn cli_set_startup(&mut self, dev: u32, text: Option<String>) -> Result<(), JsValue> {
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        cli.startup_text = text;
        Ok(())
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
        self.topo
            .add_link_ports(a, pa, b, pb)
            .map(|l| l as i32)
            .unwrap_or(-1)
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
            js_sys::Reflect::set(
                &o,
                &"bytes".into(),
                &js_sys::Uint8Array::from(&c.bytes[..]).into(),
            )
            .ok();
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

    /// Arm the traceroute job — the `trace` intent's probe.
    pub fn traceroute(&mut self, src: u32, dst_ip: &str) -> bool {
        self.topo.traceroute(src, dst_ip)
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
            js_sys::Reflect::set(
                &o,
                &"bytes".into(),
                &js_sys::Uint8Array::from(&bytes[..]).into(),
            )
            .ok();
            out.push(&o);
        }
        out
    }

    /// Link impairment. `delay` and `jitter` are ticks (1 ms in Realtime).
    /// `loss` is a percent. `mtu` 0 means no limit.
    /// `speed` is "10M" | "100M" | "1G" | "10G" | "WAN".
    pub fn set_link_impair(
        &mut self,
        id: u32,
        delay: u32,
        jitter: u32,
        loss: u8,
        mtu: u16,
        speed: &str,
    ) -> bool {
        self.topo
            .set_link_impair(id, delay, jitter, loss, mtu, speed)
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
    pub fn nat_static(
        &mut self,
        dev: u32,
        inside: &str,
        proto: &str,
        inside_port: u16,
        outside_port: u16,
    ) -> bool {
        let proto = match nat_proto(proto) {
            Some(p) => p,
            None => return false,
        };
        self.topo
            .nat_static(dev, inside, proto, inside_port, outside_port)
    }

    pub fn nat_static_clear(
        &mut self,
        dev: u32,
        inside: &str,
        proto: &str,
        outside_port: u16,
    ) -> bool {
        let proto = match nat_proto(proto) {
            Some(p) => p,
            None => return false,
        };
        self.topo.nat_static_clear(dev, inside, proto, outside_port)
    }

    /// `mode` is "access" or "trunk". `allowed` is "" / "all" or "1,10,20".
    pub fn set_switchport(
        &mut self,
        dev: u32,
        port: usize,
        mode: &str,
        access: u16,
        native: u16,
        allowed: &str,
    ) -> bool {
        self.topo
            .set_switchport(dev, port, mode == "trunk", access, native, allowed)
    }

    /// Empty `ip` clears the helper.
    pub fn set_helper(&mut self, dev: u32, port: usize, ip: &str) -> bool {
        self.topo.set_helper(dev, port, ip)
    }

    /// One ACE as a JS object: `{num, permit, proto, src, src_wild, dst,
    /// dst_wild, sport?, dport?}` — `proto` is "ip", "icmp", "tcp" or
    /// "udp"; `sport`/`dport` 0 or absent means any.
    pub fn acl_add(&mut self, dev: u32, spec: JsValue) -> bool {
        let Some(text) = js_json(&spec) else {
            return false;
        };
        self.acl_add_json(dev, &text)
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
        self.topo
            .ospf_interface(dev, port, if area < 0 { None } else { Some(area as u32) })
    }

    pub fn ospf_clear(&mut self, dev: u32) {
        self.topo.ospf_clear(dev);
    }

    /// Lab-document seat apply — one grouped call for every codec seat the
    /// per-field exports cannot reach: port admin_up/method/stats, static
    /// routes with ad/metric/learned_tick, mac/arp rows, vlan seats, toggles,
    /// and the ospf block. `fields` is the JSON object labyaml's applySeats()
    /// emits; the missing-session Err matches every cli_* export. A rejected
    /// seat set returns `netlab: bad device fields` and leaves the device
    /// untouched — the apply runs on a scratch clone and commits whole.
    pub fn apply_device_fields(&mut self, dev: u32, fields: &str) -> Result<(), JsValue> {
        if !self.sessions.contains_key(&dev) {
            return Err(missing_session(dev));
        }
        let seats: DeviceSeats = serde_json::from_str(fields).map_err(bad_fields)?;
        let tick = self.topo.tick;
        let Some(d) = self.topo.devices.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let mut scratch = d.clone();
        apply_seats(&mut scratch, seats, tick).map_err(bad_fields)?;
        *d = scratch;
        Ok(())
    }

    /// JSON snapshot of the whole topology for rendering.
    pub fn snapshot(&self) -> String {
        serde_json::to_string(&self.topo.snapshot()).unwrap_or_else(|_| "{}".into())
    }
}

/// "ip"/"" → 0 (any), "tcp" → 6, "udp" → 17 — the proto vocabulary nat_static accepts.
fn nat_proto(s: &str) -> Option<u8> {
    Some(match s {
        "tcp" => 6,
        "udp" => 17,
        "ip" | "" => 0,
        _ => return None,
    })
}

#[derive(Serialize)]
struct PendingWire<'a> {
    prompt: &'a str,
}

#[derive(Serialize)]
struct CliResultWire<'a> {
    prompt: &'a str,
    out: &'a [netlab_core::Op],
    mutated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pending: Option<PendingWire<'a>>,
}

#[derive(Serialize)]
struct KeyResultWire<'a> {
    prompt: &'a str,
    out: &'a [netlab_core::Op],
}

#[derive(Serialize)]
struct HelpRowWire<'a> {
    word: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(tag = "t")]
enum HelpResultWire<'a> {
    #[serde(rename = "rows")]
    Rows {
        rows: Vec<HelpRowWire<'a>>,
        prompt: &'a str,
    },
    #[serde(rename = "inline")]
    Inline {
        inline: &'a [String],
        prompt: &'a str,
    },
    #[serde(rename = "err")]
    Err { index: usize, prompt: &'a str },
}

#[derive(Serialize)]
#[serde(tag = "t")]
enum CompleteResultWire<'a> {
    #[serde(rename = "line")]
    Line { line: &'a str },
    #[serde(rename = "list")]
    List { list: &'a [String] },
    #[serde(rename = "err")]
    Err { index: usize },
}

#[derive(Serialize)]
struct ScriptErrorWire<'a> {
    lineno: u32,
    msg: &'a str,
}

#[derive(Serialize)]
struct ScriptResultWire<'a> {
    out: &'a [netlab_core::Op],
    mutated: bool,
    errors: Vec<ScriptErrorWire<'a>>,
}

/// JsValue → JSON text for the serde decode path. Native builds cannot
/// stringify a JS value — there is no wire to cross — and return None.
#[cfg(target_arch = "wasm32")]
fn js_json(v: &JsValue) -> Option<String> {
    js_sys::JSON::stringify(v).ok().map(String::from)
}

#[cfg(not(target_arch = "wasm32"))]
fn js_json(_v: &JsValue) -> Option<String> {
    None
}

/// "10.0.0.1" → octets. A row with an unparseable address drops, same as the
/// codec's tolerant-decode edge.
fn parse_ip4(s: &str) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let mut it = s.split('.');
    for o in &mut out {
        *o = it.next()?.parse().ok()?;
    }
    it.next().is_none().then_some(out)
}

/// "aabb.ccdd.eeff" | "aa:bb:cc:dd:ee:ff" | "aa-bb-cc-dd-ee-ff" → u64 key.
fn parse_mac_u64(s: &str) -> Option<u64> {
    let hex: String = s.split(['.', ':', '-']).collect();
    if hex.len() != 12 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(&hex, 16).ok()
}

fn etype(s: &str) -> netlab_core::device::EntryType {
    match s {
        "static" => netlab_core::device::EntryType::Static,
        _ => netlab_core::device::EntryType::Dynamic,
    }
}

/// The applySeats() wire shape — every seat optional. Unknown keys are a
/// decode error, not a silent skip: a typo'd seat key must surface.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DeviceSeats {
    #[serde(default)]
    ports: Vec<SeatPort>,
    #[serde(default)]
    routes: Vec<SeatRoute>,
    #[serde(default)]
    mac: Vec<SeatMac>,
    #[serde(default)]
    arp: Vec<SeatArp>,
    #[serde(default)]
    vlans: HashMap<String, SeatVlan>,
    #[serde(default)]
    toggles: HashMap<String, bool>,
    ospf: Option<SeatOspf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatPort {
    index: usize,
    admin_up: Option<bool>,
    method: Option<String>,
    stats: Option<SeatStats>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatStats {
    rx: Option<u64>,
    tx: Option<u64>,
    rx_err: Option<u64>,
    tx_err: Option<u64>,
    drops: Option<u64>,
    cleared_tick: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatRoute {
    prefix: String,
    mask: Option<String>,
    next_hop: String,
    #[serde(default)]
    out_port: usize,
    /// The decoded route rows keep their `proto` word — only `static` is a
    /// seat; connected re-derives and OSPF relearns.
    proto: Option<String>,
    ad: Option<u8>,
    #[serde(default)]
    metric: u32,
    #[serde(default)]
    learned_tick: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatMac {
    mac: String,
    port: usize,
    #[serde(default = "one")]
    vlan: u16,
    #[serde(default)]
    created_tick: u64,
    #[serde(default)]
    etype: String,
}

fn one() -> u16 {
    1
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatArp {
    ip: String,
    mac: String,
    port: usize,
    #[serde(default)]
    created_tick: u64,
    #[serde(default)]
    etype: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatVlan {
    name: Option<String>,
    state: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatOspfNet {
    net: Option<String>,
    wild: Option<String>,
    #[serde(default)]
    area: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeatOspf {
    pid: u16,
    router_id: Option<String>,
    #[serde(default)]
    networks: Vec<SeatOspfNet>,
}

/// Authored seats land as config, not runtime effects — admin/method/stats
/// write fields directly (no link events during load), while routes, vlans,
/// toggles and ospf ride the kind-gated config fns so a wrong-kind seat just
/// skips. Only the device's own seats apply; `routing_enabled` is a
/// constructor capability and never rewrites.
///
/// Seat ports are validated: an out-of-range `out_port`/`port` is an Err on
/// the whole call — a raw index reaching `emit_ip`'s `ports[route.out_port]`
/// would panic. KindGate skips; every other SetErr and every range violation
/// is a bad-fields Err.
fn apply_seats(d: &mut Device, seats: DeviceSeats, tick: u64) -> Result<(), String> {
    use netlab_core::config::{self, ToggleKind};
    use netlab_core::device::{ArpEntry, IpMethod, MacEntry, Route, RouteProto, Vlan, VlanState};

    for sp in &seats.ports {
        let Some(p) = d.ports.get_mut(sp.index) else {
            continue;
        };
        if let Some(up) = sp.admin_up {
            p.admin_up = up;
        }
        if let Some(m) = &sp.method {
            p.method = match m.as_str() {
                "manual" => IpMethod::Manual,
                "dhcp" => IpMethod::Dhcp,
                _ => IpMethod::None,
            };
        }
        if let Some(st) = &sp.stats {
            if let Some(v) = st.rx {
                p.stats.rx = v;
            }
            if let Some(v) = st.tx {
                p.stats.tx = v;
            }
            if let Some(v) = st.rx_err {
                p.stats.rx_err = v;
            }
            if let Some(v) = st.tx_err {
                p.stats.tx_err = v;
            }
            if let Some(v) = st.drops {
                p.stats.drops = v;
            }
            if let Some(v) = st.cleared_tick {
                p.stats.cleared_tick = v;
            }
        }
    }
    for r in &seats.routes {
        match r.proto.as_deref() {
            Some("static" | "s") | None => {}
            Some(p) => return Err(format!("route proto {p} is not a seat")),
        }
        let (Some(prefix), Some(mask), Some(next_hop)) = (
            parse_ip4(&r.prefix),
            r.mask.as_deref().and_then(parse_ip4),
            parse_ip4(&r.next_hop),
        ) else {
            continue;
        };
        if r.out_port >= d.ports.len() {
            return Err(format!("route out_port {} has no port", r.out_port));
        }
        // The same path `ip route` runs: kind gate, contiguous mask, next-hop
        // port resolution. The seat's own out_port/metric/learned_tick then
        // overwrite — the document is the authority for those fields.
        match config::add_route(d, prefix, mask, next_hop, r.ad, tick) {
            Ok(()) => {
                let net = [
                    prefix[0] & mask[0],
                    prefix[1] & mask[1],
                    prefix[2] & mask[2],
                    prefix[3] & mask[3],
                ];
                if let Some(row) = d.routes.iter_mut().find(|x| {
                    x.proto == RouteProto::Static
                        && x.prefix == net
                        && x.mask == mask
                        && x.next_hop == next_hop
                }) {
                    row.out_port = r.out_port;
                    row.metric = r.metric;
                    row.learned_tick = r.learned_tick;
                }
            }
            Err(config::SetErr::KindGate) => {
                // No `ip route` grammar for this kind. A host still carries
                // the default route its `ip <addr> <mask> <gw>` command wrote.
                if d.kind == DeviceKind::Host {
                    d.routes.push(Route {
                        prefix,
                        mask,
                        next_hop,
                        out_port: r.out_port,
                        connected: false,
                        proto: RouteProto::Static,
                        ad: r.ad.unwrap_or(1),
                        metric: r.metric,
                        learned_tick: r.learned_tick,
                    });
                }
            }
            Err(e) => return Err(format!("route {}: {e:?}", r.prefix)),
        }
    }
    for m in &seats.mac {
        let Some(mac) = parse_mac_u64(&m.mac) else {
            continue;
        };
        if m.port >= d.ports.len() {
            return Err(format!("mac port {} has no port", m.port));
        }
        d.mac_table.insert(
            (mac, m.vlan),
            MacEntry {
                port: m.port,
                created_tick: m.created_tick,
                etype: etype(&m.etype),
            },
        );
    }
    for a in &seats.arp {
        let (Some(ip), Some(mac)) = (parse_ip4(&a.ip), parse_mac_u64(&a.mac)) else {
            continue;
        };
        if a.port >= d.ports.len() {
            return Err(format!("arp port {} has no port", a.port));
        }
        d.arp_cache.insert(
            ip,
            ArpEntry {
                mac,
                port: a.port,
                created_tick: a.created_tick,
                etype: etype(&a.etype),
            },
        );
    }
    if d.kind == DeviceKind::Switch {
        for (id, v) in &seats.vlans {
            let Ok(vid) = id.parse::<u16>() else {
                continue;
            };
            let row = d.vlans.entry(vid).or_insert_with(|| Vlan {
                name: format!("VLAN{vid:04}"),
                state: VlanState::Active,
            });
            if let Some(n) = &v.name {
                row.name = n.clone();
            }
            if let Some(s) = &v.state {
                row.state = if s == "suspend" {
                    VlanState::Suspend
                } else {
                    VlanState::Active
                };
            }
        }
    }
    for (k, on) in &seats.toggles {
        let kind = match k.as_str() {
            "domain_lookup" => ToggleKind::DomainLookup,
            "ip_routing" => ToggleKind::IpRouting,
            "proxy_arp" => ToggleKind::ProxyArp,
            "directed_broadcast" => ToggleKind::DirectedBroadcast,
            _ => continue,
        };
        let _ = config::set_toggle(d, kind, *on); // KindGate → the codec default already stands
    }
    if d.kind == DeviceKind::Router {
        if let Some(o) = &seats.ospf {
            if !d.ospf.on {
                let _ = config::ospf_enter(d, o.pid);
            } else {
                d.ospf.pid = o.pid; // an `ip ospf area` port seat pinned pid 1 — the authored block wins
            }
            for n in &o.networks {
                let (Some(net), Some(wild)) = (
                    n.net.as_deref().and_then(parse_ip4),
                    n.wild.as_deref().and_then(parse_ip4),
                ) else {
                    continue;
                };
                let _ = config::ospf_net_add(d, net, wild, n.area);
            }
            if let Some(rid) = o.router_id.as_deref().and_then(parse_ip4) {
                let _ = config::ospf_router_id_set(d, rid);
            }
        }
    }
    Ok(())
}

/// The labfile/inspector ACE shape — one flat object per entry.
#[derive(Deserialize)]
struct AceSpec {
    num: u16,
    permit: bool,
    proto: String,
    src: String,
    src_wild: String,
    dst: String,
    dst_wild: String,
    #[serde(default)]
    sport: u16,
    #[serde(default)]
    dport: u16,
}

impl NetLab {
    fn acl_add_json(&mut self, dev: DeviceId, text: &str) -> bool {
        let Ok(spec) = serde_json::from_str::<AceSpec>(text) else {
            return false;
        };
        let proto = match spec.proto.as_str() {
            "icmp" => 1,
            "tcp" => 6,
            "udp" => 17,
            "ip" | "any" => 0,
            _ => return false,
        };
        self.topo.acl_add(
            dev,
            netlab_core::AceArgs {
                num: spec.num,
                permit: spec.permit,
                proto,
                src: &spec.src,
                src_wild: &spec.src_wild,
                dst: &spec.dst,
                dst_wild: &spec.dst_wild,
                sport: spec.sport,
                dport: spec.dport,
            },
        )
    }

    fn cli_exec_json(&mut self, dev: DeviceId, line: &str) -> Result<String, JsValue> {
        if !self.sessions.contains_key(&dev) {
            return Err(missing_session(dev));
        }
        let Some(d) = self.topo.devices.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let r = cli.exec(d, line, self.topo.tick, &mut self.topo.events);
        Ok(serde_json::to_string(&CliResultWire {
            prompt: &r.prompt,
            out: &r.out,
            mutated: r.mutated,
            pending: r.pending.as_ref().map(|p| PendingWire {
                prompt: &p.prompt,
            }),
        })
        .expect_throw("cli_exec wire encode"))
    }

    fn cli_key_json(&mut self, dev: DeviceId, op: &str) -> Result<String, JsValue> {
        match op {
            "ctrlc" | "ctrlz" => {}
            _ => wasm_bindgen::throw_str("unknown KeyOp"),
        }
        if !self.sessions.contains_key(&dev) {
            return Err(missing_session(dev));
        }
        let Some(d) = self.topo.devices.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let r = cli.key(d, op, self.topo.tick, &mut []);
        Ok(serde_json::to_string(&KeyResultWire {
            prompt: &r.prompt,
            out: &r.out,
        })
        .expect_throw("cli_key wire encode"))
    }

    fn cli_help_json(&mut self, dev: DeviceId, before_cursor: &str) -> Result<String, JsValue> {
        if !self.sessions.contains_key(&dev) {
            return Err(missing_session(dev));
        }
        let Some(d) = self.topo.devices.get(&dev) else {
            return Err(missing_session(dev));
        };
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let r = cli.help(d, before_cursor);
        let w = match &r {
            HelpResult::Rows { rows, prompt } => HelpResultWire::Rows {
                rows: rows
                    .iter()
                    .map(|row| HelpRowWire {
                        word: &row.word,
                        help: row.help.as_deref(),
                    })
                    .collect(),
                prompt,
            },
            HelpResult::Inline { inline, prompt } => {
                HelpResultWire::Inline { inline, prompt }
            }
            HelpResult::Err { index, prompt } => HelpResultWire::Err {
                index: *index,
                prompt,
            },
        };
        Ok(serde_json::to_string(&w).expect_throw("cli_help wire encode"))
    }

    fn cli_complete_json(
        &mut self,
        dev: DeviceId,
        before_cursor: &str,
    ) -> Result<String, JsValue> {
        if !self.sessions.contains_key(&dev) {
            return Err(missing_session(dev));
        }
        let Some(d) = self.topo.devices.get(&dev) else {
            return Err(missing_session(dev));
        };
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let r = cli.complete(d, before_cursor);
        let w = match &r {
            CompleteResult::Line { line } => CompleteResultWire::Line { line },
            CompleteResult::List { list } => CompleteResultWire::List { list },
            CompleteResult::Err { index } => CompleteResultWire::Err { index: *index },
        };
        Ok(serde_json::to_string(&w).expect_throw("cli_complete wire encode"))
    }

    fn cli_script_json(&mut self, dev: DeviceId, text: &str) -> Result<String, JsValue> {
        if !self.sessions.contains_key(&dev) {
            return Err(missing_session(dev));
        }
        let Some(d) = self.topo.devices.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let Some(cli) = self.sessions.get_mut(&dev) else {
            return Err(missing_session(dev));
        };
        let r = cli.script(d, text, self.topo.tick, &mut self.topo.events);
        Ok(serde_json::to_string(&ScriptResultWire {
            out: &r.out,
            mutated: r.mutated,
            errors: r
                .errors
                .iter()
                .map(|e| ScriptErrorWire {
                    lineno: e.lineno,
                    msg: &e.msg,
                })
                .collect(),
        })
        .expect_throw("cli_script wire encode"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab_with_router() -> (NetLab, u32) {
        let mut lab = NetLab::new();
        let dev = lab.add_device("router", "R1", 0.0, 0.0);
        (lab, dev)
    }

    #[test]
    fn missing_session_returns_err_on_every_cli_export() {
        let mut lab = NetLab::new();
        assert!(lab.cli_exec_json(9, "show ?").is_err());
        assert!(lab.cli_key_json(9, "ctrlc").is_err());
        assert!(lab.cli_help_json(9, "").is_err());
        assert!(lab.cli_complete_json(9, "").is_err());
        assert!(lab.cli_prompt(9).is_err());
        assert!(lab.cli_script_json(9, "hostname x").is_err());
        assert!(lab.cli_set_ifnames(9, vec!["g0/0".to_string()]).is_err());
        assert!(lab.cli_set_startup(9, None).is_err());
        assert!(lab.baseline_commit(9).is_err());
        assert!(lab.reset_config(9).is_err());
    }

    #[test]
    fn remove_device_and_restore_free_the_session() {
        let (mut lab, dev) = lab_with_router();
        lab.remove_device(dev);
        assert!(lab.cli_exec_json(dev, "show ?").is_err());

        let (mut lab, dev) = lab_with_router();
        lab.restore();
        assert!(lab.cli_exec_json(dev, "show ?").is_err());
    }

    #[test]
    fn traceroute_arms_a_job_like_ping() {
        let (mut lab, dev) = lab_with_router();
        assert!(lab.traceroute(dev, "10.0.0.1"));
        assert!(!lab.traceroute(dev, "not-an-ip"));
        assert!(!lab.traceroute(9, "10.0.0.1"));
        lab.set_power(dev, false);
        assert!(!lab.traceroute(dev, "10.0.0.1"));
    }

    #[test]
    fn cli_exec_wire_shape() {
        let (mut lab, dev) = lab_with_router();
        let json: serde_json::Value =
            serde_json::from_str(&lab.cli_exec_json(dev, "show version").unwrap()).unwrap();
        assert!(json["prompt"].is_string());
        assert!(json["out"].is_array());
        assert!(json["mutated"].is_boolean());
        assert!(json.get("pending").is_none());
        lab.cli_exec_json(dev, "enable").unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&lab.cli_exec_json(dev, "reload").unwrap()).unwrap();
        assert!(json["pending"]["prompt"].is_string());
        assert_eq!(json["prompt"], json["pending"]["prompt"]);
        lab.cli_key_json(dev, "ctrlc").unwrap();
    }

    #[test]
    fn cli_key_wire_shape() {
        let (mut lab, dev) = lab_with_router();
        let json: serde_json::Value =
            serde_json::from_str(&lab.cli_key_json(dev, "ctrlc").unwrap()).unwrap();
        assert!(json["prompt"].is_string());
        assert!(json["out"].is_array());
        assert!(json.get("mutated").is_none());
        assert!(json.get("pending").is_none());
        let json: serde_json::Value =
            serde_json::from_str(&lab.cli_key_json(dev, "ctrlz").unwrap()).unwrap();
        assert_eq!(json["out"], serde_json::json!([]));
    }

    #[test]
    fn help_and_complete_wire_tags() {
        let (mut lab, dev) = lab_with_router();
        let help: serde_json::Value =
            serde_json::from_str(&lab.cli_help_json(dev, "sh").unwrap()).unwrap();
        assert_eq!(help["t"], "rows");
        assert!(help["prompt"].is_string());
        let complete: serde_json::Value =
            serde_json::from_str(&lab.cli_complete_json(dev, "en").unwrap()).unwrap();
        assert_eq!(complete["t"], "line");
        assert!(complete.get("prompt").is_none());
        let help: serde_json::Value =
            serde_json::from_str(&lab.cli_help_json(dev, "xyzzy ?").unwrap()).unwrap();
        assert_eq!(help["t"], "err");
        assert!(help["index"].is_number());
    }

    #[test]
    fn cli_script_wire_shape_and_echo_ops() {
        let (mut lab, dev) = lab_with_router();
        let json: serde_json::Value =
            serde_json::from_str(&lab.cli_script_json(dev, "hostname R2\nend").unwrap())
                .unwrap();
        assert!(json["out"].is_array());
        assert!(json["mutated"].is_boolean());
        assert!(json["errors"].is_array());
        assert!(json.get("prompt").is_none());
        assert!(
            json["out"]
                .as_array()
                .unwrap()
                .iter()
                .any(|op| op["t"] == "echo")
        );
        let exec_json: serde_json::Value =
            serde_json::from_str(&lab.cli_exec_json(dev, "show version").unwrap()).unwrap();
        assert!(
            !exec_json["out"]
                .as_array()
                .unwrap()
                .iter()
                .any(|op| op["t"] == "echo"),
            "Op::Echo must never appear in CliResult.out"
        );
    }

    #[test]
    fn cli_prompt_is_bare_string_and_pending_owns_space() {
        let (mut lab, dev) = lab_with_router();
        assert_eq!(lab.cli_prompt(dev).unwrap(), "R1>");
        lab.cli_exec_json(dev, "enable").unwrap();
        assert_eq!(lab.cli_prompt(dev).unwrap(), "R1#");
        lab.cli_exec_json(dev, "reload").unwrap();
        let prompt = lab.cli_prompt(dev).unwrap();
        assert!(prompt.ends_with(' '), "armed pending prompt owns its trailing space: {prompt:?}");
        lab.cli_key_json(dev, "ctrlc").unwrap();
    }

    #[test]
    fn set_ifnames_and_startup_roundtrip() {
        let (mut lab, dev) = lab_with_router();
        lab.cli_set_ifnames(dev, vec!["g0/0".to_string()]).unwrap();
        lab.cli_set_startup(dev, Some("hostname R9".to_string()))
            .unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&lab.cli_exec_json(dev, "show startup-config").unwrap())
                .unwrap();
        let texts: Vec<&str> = json["out"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|op| op["text"].as_str())
            .collect();
        assert!(texts.iter().any(|t| t.contains("hostname R9")), "{texts:?}");
        lab.cli_set_startup(dev, None).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&lab.cli_exec_json(dev, "show startup-config").unwrap())
                .unwrap();
        let texts: Vec<&str> = json["out"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|op| op["text"].as_str())
            .collect();
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Non-volatile configuration memory is not present")),
            "{texts:?}"
        );
    }

    #[test]
    #[should_panic]
    fn noncanonical_ifnames_die() {
        let (mut lab, dev) = lab_with_router();
        let _ = lab.cli_set_ifnames(dev, vec!["GigabitEthernet0/0".to_string()]);
    }

    #[test]
    fn empty_and_short_ifnames_fall_back_to_canonical() {
        let (mut lab, dev) = lab_with_router();
        lab.cli_set_ifnames(dev, vec![]).unwrap();
        lab.cli_set_ifnames(dev, vec!["g0/0".to_string(), "g0/1".to_string()])
            .unwrap();
        lab.cli_exec_json(dev, "enable").unwrap();
        lab.cli_exec_json(dev, "configure terminal").unwrap();
        lab.cli_exec_json(dev, "interface g0/5").unwrap();
        assert_eq!(lab.cli_prompt(dev).unwrap(), "R1(config-if)#");
    }

    #[test]
    #[should_panic]
    fn unknown_keyop_dies() {
        let (mut lab, dev) = lab_with_router();
        let _ = lab.cli_key_json(dev, "ctrlb");
    }

    #[test]
    fn acl_add_decodes_the_flat_object() {
        let (mut lab, dev) = lab_with_router();
        assert!(lab.acl_add_json(
            dev,
            r#"{"num":100,"permit":true,"proto":"tcp","src":"10.0.0.0","src_wild":"0.0.0.255","dst":"0.0.0.0","dst_wild":"255.255.255.255","dport":80}"#
        ));
        let d = &lab.topo.devices[&dev];
        let ace = &d.acls[0].entries[0];
        assert_eq!(d.acls[0].num, 100);
        assert_eq!(ace.proto, 6);
        assert_eq!(ace.dport, Some(80));
        assert_eq!(ace.sport, None);
        assert!(!lab.acl_add_json(dev, r#"{"num":100,"permit":true,"proto":"gre","src":"0.0.0.0","src_wild":"255.255.255.255","dst":"0.0.0.0","dst_wild":"255.255.255.255"}"#));
        assert!(!lab.acl_add_json(dev, "{}"));
        assert!(!lab.acl_add_json(dev, "not json"));
        assert!(!lab.acl_add(dev, JsValue::NULL));
    }

    #[test]
    fn reset_config_restores_the_authored_baseline() {
        let (mut lab, dev) = lab_with_router();
        // authored config lands through the decode path the lab document
        // drives, then baseline_commit snapshots it.
        assert!(lab.acl_add_json(
            dev,
            r#"{"num":100,"permit":false,"proto":"udp","src":"0.0.0.0","src_wild":"255.255.255.255","dst":"0.0.0.0","dst_wild":"255.255.255.255","dport":53}"#
        ));
        assert!(lab.set_access_group(dev, 0, 100, "in"));
        lab.cli_exec_json(dev, "enable").unwrap();
        lab.cli_exec_json(dev, "configure terminal").unwrap();
        lab.cli_exec_json(dev, "hostname Authored").unwrap();
        lab.baseline_commit(dev).unwrap();
        // runtime mutations: list dropped, binding dropped, hostname changed,
        // a learned ARP-ish table entry — all must revert.
        lab.cli_exec_json(dev, "no access-list 100").unwrap();
        lab.cli_exec_json(dev, "hostname Drifted").unwrap();
        lab.set_access_group(dev, 0, 0, "off-in");
        {
            let d = lab.topo.devices.get_mut(&dev).unwrap();
            assert!(d.acls.is_empty(), "runtime `no access-list` must drop the list");
            assert_eq!(d.ports[0].in_acl, None, "runtime unbind must clear in_acl");
            d.arp_cache.insert(
                [10, 0, 0, 9],
                netlab_core::device::ArpEntry {
                    mac: 0x1122_3344_5566,
                    port: 0,
                    created_tick: lab.topo.tick,
                    etype: netlab_core::device::EntryType::Dynamic,
                },
            );
        }
        assert_eq!(lab.cli_prompt(dev).unwrap(), "Drifted(config)#");
        lab.reset_config(dev).unwrap();
        let d = &lab.topo.devices[&dev];
        assert_eq!(d.name, "Authored", "authored hostname must return");
        assert_eq!(d.acls.len(), 1, "authored ACL must return, not zero");
        assert_eq!(d.acls[0].num, 100);
        assert_eq!(d.acls[0].entries.len(), 1);
        assert!(!d.acls[0].entries[0].permit);
        assert_eq!(d.acls[0].entries[0].proto, 17);
        assert_eq!(d.acls[0].entries[0].dport, Some(53));
        assert_eq!(d.ports[0].in_acl, Some(100), "authored binding must return");
        assert!(d.arp_cache.is_empty(), "learned state must not survive");
        // session seat is back at Exec — the prompt is user-exec, not config.
        assert_eq!(lab.cli_prompt(dev).unwrap(), "Authored>");
    }

    #[test]
    fn reset_config_on_a_fresh_device_gives_factory_defaults() {
        let (mut lab, dev) = lab_with_router();
        lab.cli_exec_json(dev, "enable").unwrap();
        lab.cli_exec_json(dev, "configure terminal").unwrap();
        lab.cli_exec_json(dev, "hostname Changed").unwrap();
        lab.reset_config(dev).unwrap();
        let d = &lab.topo.devices[&dev];
        assert_eq!(d.name, "R1");
        assert!(d.acls.is_empty());
        assert_eq!(lab.cli_prompt(dev).unwrap(), "R1>");
    }

    #[test]
    fn apply_device_fields_lands_every_seat() {
        let (mut lab, dev) = lab_with_router();
        lab.apply_device_fields(
            dev,
            r#"{
                "ports": [{"index": 0, "admin_up": true, "method": "manual",
                           "stats": {"rx": 42, "drops": 3}}],
                "routes": [{"prefix": "192.168.9.0", "mask": "255.255.255.0",
                            "next_hop": "10.0.0.2", "out_port": 0, "ad": 200,
                            "metric": 7, "learned_tick": 99}],
                "arp": [{"ip": "10.0.0.9", "mac": "1122.3344.5566", "port": 0,
                         "created_tick": 5, "etype": "static"}],
                "toggles": {"directed_broadcast": true, "routing_enabled": false},
                "ospf": {"pid": 7, "router_id": "9.9.9.9",
                         "networks": [{"net": "10.0.0.0", "wild": "0.0.0.255", "area": 2}]}
            }"#,
        )
        .unwrap();
        let d = &lab.topo.devices[&dev];
        assert!(d.ports[0].admin_up, "router port authored up");
        assert_eq!(d.ports[0].method, netlab_core::device::IpMethod::Manual);
        assert_eq!(d.ports[0].stats.rx, 42);
        assert_eq!(d.ports[0].stats.drops, 3);
        let r = d
            .routes
            .iter()
            .find(|r| r.proto == netlab_core::device::RouteProto::Static)
            .unwrap();
        assert_eq!(r.prefix, [192, 168, 9, 0]);
        assert_eq!(r.ad, 200);
        assert_eq!(r.metric, 7);
        assert_eq!(r.learned_tick, 99);
        let arp = d.arp_cache.get(&[10, 0, 0, 9]).unwrap();
        assert_eq!(arp.mac, 0x1122_3344_5566);
        assert_eq!(arp.etype, netlab_core::device::EntryType::Static);
        assert!(d.toggles.directed_broadcast, "gated toggle applies");
        assert!(d.toggles.routing_enabled, "constructor capability never rewrites");
        assert!(d.ospf.on);
        assert_eq!(d.ospf.pid, 7);
        assert_eq!(d.ospf.router_id.value, [9, 9, 9, 9]);
        assert!(d.ospf.router_id.explicit);
        assert_eq!(d.ospf.nets.len(), 1);
        assert_eq!(d.ospf.nets[0].area, 2);
        assert!(lab.apply_device_fields(9, "{}").is_err(), "missing session");
    }

    #[test]
    fn apply_device_fields_kind_gates_and_switch_seats() {
        let mut lab = NetLab::new();
        let host = lab.add_device("host", "H1", 0.0, 0.0);
        let sw = lab.add_device("switch", "S1", 0.0, 0.0);
        // ospf on a host: the gated fns refuse — nothing applies.
        lab.apply_device_fields(host, r#"{"ospf":{"pid":1,"networks":[{"net":"10.0.0.0","wild":"0.0.0.255","area":0}]}}"#)
            .unwrap();
        assert!(!lab.topo.devices[&host].ospf.on);
        // switch: mac rows + vlan seats land.
        lab.apply_device_fields(
            sw,
            r#"{
                "mac": [{"mac": "aabb.ccdd.eeff", "port": 2, "vlan": 10,
                         "created_tick": 4, "etype": "static"}],
                "vlans": {"10": {"name": "users", "state": "suspend"},
                          "20": {}}
            }"#,
        )
        .unwrap();
        let d = &lab.topo.devices[&sw];
        let e = d.mac_table.get(&(0xaabb_ccdd_eeff, 10)).unwrap();
        assert_eq!(e.port, 2);
        assert_eq!(d.vlans[&10].name, "users");
        assert_eq!(
            d.vlans[&10].state,
            netlab_core::device::VlanState::Suspend
        );
        assert_eq!(d.vlans[&20].name, "VLAN0020", "empty seat names itself");
        assert_eq!(
            d.vlans[&20].state,
            netlab_core::device::VlanState::Active
        );
    }

    #[test]
    fn apply_device_fields_rejects_bad_seats_whole() {
        let (mut lab, dev) = lab_with_router();
        // out-of-range out_port: clean Err, and nothing applied — a raw index
        // reaching emit_ip's ports[route.out_port] would panic.
        assert!(lab
            .apply_device_fields(
                dev,
                r#"{"toggles":{"proxy_arp":false},
                    "routes":[{"prefix":"10.9.0.0","mask":"255.255.255.0","next_hop":"10.0.0.2","out_port":999}]}"#,
            )
            .is_err());
        let d = &lab.topo.devices[&dev];
        assert!(d.routes.is_empty(), "rejected seat must not partially apply");
        assert!(d.toggles.proxy_arp, "rejected seat must not partially apply");
        // NetLab stays usable: a good call lands right after the bad one.
        lab.apply_device_fields(
            dev,
            r#"{"routes":[{"prefix":"10.9.0.0","mask":"255.255.255.0","next_hop":"10.0.0.2","out_port":1}]}"#,
        )
        .unwrap();
        assert_eq!(lab.topo.devices[&dev].routes[0].out_port, 1);
        // non-integer / negative port seats fail the serde decode.
        assert!(lab
            .apply_device_fields(dev, r#"{"routes":[{"prefix":"10.9.0.0","mask":"255.255.255.0","next_hop":"10.0.0.2","out_port":1e30}]}"#)
            .is_err());
        assert!(lab
            .apply_device_fields(dev, r#"{"routes":[{"prefix":"10.9.0.0","mask":"255.255.255.0","next_hop":"10.0.0.2","out_port":-1}]}"#)
            .is_err());
        // a typo'd seat key is an Err, not a silent drop.
        assert!(lab.apply_device_fields(dev, r#"{"routs":[]}"#).is_err());
        assert!(lab
            .apply_device_fields(dev, r#"{"routes":[{"prefix":"10.9.0.0","mask":"255.255.255.0","next_hop":"10.0.0.2","out_port":0,"proto":"ospf"}]}"#)
            .is_err());
        // mac/arp seat ports are validated the same way.
        assert!(lab
            .apply_device_fields(dev, r#"{"mac":[{"mac":"aabb.ccdd.eeff","port":99}]}"#)
            .is_err());
        assert!(lab
            .apply_device_fields(dev, r#"{"arp":[{"ip":"10.0.0.9","mac":"aabb.ccdd.eeff","port":99}]}"#)
            .is_err());
        // a non-contiguous mask rides the same setter `ip route` does.
        assert!(lab
            .apply_device_fields(dev, r#"{"routes":[{"prefix":"10.9.0.0","mask":"255.255.0.255","next_hop":"10.0.0.2","out_port":0}]}"#)
            .is_err());
        // a host's authored default route still lands — its own grammar path.
        let host = lab.add_device("host", "H1", 0.0, 0.0);
        lab.apply_device_fields(
            host,
            r#"{"routes":[{"prefix":"0.0.0.0","mask":"0.0.0.0","next_hop":"10.0.0.1","out_port":0}]}"#,
        )
        .unwrap();
        assert_eq!(lab.topo.devices[&host].routes.len(), 1);
        // a route seat on a switch rides the modeful gate the grammar uses.
        let sw = lab.add_device("switch", "S1", 0.0, 0.0);
        lab.apply_device_fields(
            sw,
            r#"{"routes":[{"prefix":"10.9.0.0","mask":"255.255.255.0","next_hop":"10.0.0.2","out_port":0}]}"#,
        )
        .unwrap();
        assert_eq!(lab.topo.devices[&sw].routes.len(), 1);
    }
}
