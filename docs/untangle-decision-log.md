# netlab untangle — decision log

Scope: `core/`, `wasm/`, first-party `web/` (excludes `web/pkg`, `web/vendor`, `web/v86`, `web/dynamips.*`, `emu/dynamips` — generated/vendored). Additions: comments + tests. Plus modularize monoliths.

## Fixes (each covers the problems nested under it)

### R1. Split `topology.rs` into `topology/` modules
Sim engine, config facade, actions, and snapshot DTO are four reasons to change in one file.

- `topology/graph` (mod.rs): `Topology` struct, device/link CRUD, `set_link_up/capture/impair`, `serial_ticks`→`LinkSpeed`
- `topology/sim.rs`: `step`, `enqueue`, `inject_link`, drains, rng, captures
- `topology/config.rs`: all per-device setters (ip/route/switchport/helper/nat/acl/ospf/services/dns/power)
- `topology/actions.rs`: `ping*`, `dhcp_client`, `dns_query`, `probe_*`, `http_get`
- `topology/snapshot.rs`: `snapshot()` + 9 `*Snap` structs

Problems covered: 1009-line file mixing 4 concerns; snapshot structs change whenever Device/Link internals do; `remove_route` positional index.

### R2. Split `services.rs` into `services/` modules
DHCP, DNS, TCP/UDP flows, NAT — four independent protocol stacks sharing one file and loose helpers.

- `services/mod.rs`: `poll` (minus ospf), `local_l4` dispatch, `note`, `first_ip`, `alloc_port`, `next_id`, `udp_frame`, `l4_tuple`
- `services/dhcp.rs`: client+server+relay, `DhcpClient`, `DhcpMsg`, codec, `dhcp_state`, `DhcpState` enum
- `services/dns.rs`: query/qname/response/answer codec + `on_dns`
- `services/flows.rs`: `Flow{kind→FlowKind enum}`, TCP flags, start/on/client/server/send_tcp, on_udp, on_unreach, send_unreach
- `services/nat.rs`: `NatEnt`, `StaticNat`, `nat_ingress`, `nat_egress`

Problems covered: Flow.kind magic 1-4 dispatch table ×3; DhcpClient.state read as `!=3` from topology.rs; port 67/68/53 magic everywhere localized per-protocol.

### R3. One IPv4 header builder
`push_ipv4` exists but ICMP-in-encode and `send_unreach` hand-roll the same header.
- `encode` ICMP arm uses `push_ipv4`; new `eth::build_icmp` used by `send_unreach`; delete dead `build_udp` placeholder.
- `raw_label`: single proto→name table (drop the shadowed duplicated match arm).

Problems covered: 3× IPv4-header construction; dead placeholder packet; duplicated proto-name table.

### R4. Kill magic numbers with enums + one helper per duplicated computation
- `Route.proto` → keep u8 but add named consts `PROTO_STATIC`/`PROTO_OSPF`? Better: `enum RouteProto` with repr(u8). Sites: device.rs:73, ospf.rs:214,263, topology.rs:871,922.
- `Flow.kind`/`state` → `FlowKind`/`FlowState` enums (1 tcp, 2 http, 3 udp, 4 tcp-server; 1 syn, 2 est, 3 done).
- `DhcpClient.state` → `DhcpState` enum (Discover/Request/Bound); `dhcp_state()` reads it.
- merge `l4_of`(device) + `l4_tuple`(services) → one `frame_l4` helper (returns proto/sport/dport/icmp_id).
- `owns_ip` → `pub(crate)`, reuse in services' 3 inline copies + `local_l4`'s `mine`.
- `bcast` predicate → `Frame::is_broadcast()`.
- `find port mac by src ip` ×4 in services → `Device::mac_of_ip(src)`.
- ephemeral port `40_000 + ip_id % 20_000` ×2 → `alloc_port` only (NAT uses it too).
- `mac6`/`u64_mac`/`mask_len` wrappers → delete, call `eth::mac_bytes`/`mac_of`/`mask_bits` direct.
- DHCP cookie `[99,130,83,99]` → `const DHCP_MAGIC`.
- OSPF hello/dead literals in `hello()` → use HELLO_EVERY/DEAD consts.
- default-route replace (set_gateway vs client_dhcp) → `Device::set_default_route(gw, port)`.
- VLAN normalization `.max(1)` scattered → `Port::access_vid()`/`native_vid()` helpers.

Problems covered: hidden links #3-15, #26-31, #36 (core audit).

### R5. `Frame` constructors
12 sites build `Frame{…, raw: None, icmp_echo: None, vlan: 0}` or `raw: Some(..)` literally. Add `Frame::new(payload)` (empty fields) + `Frame::raw_ip(raw_bytes, src, dst)` — collapses the literal tail everywhere.

### R6. Own your own state
- `Device::power_off()` encapsulates clearing runtime state (outbox/pending/arp/mac_table/flows/nat/ospf reset/dhcp); topology.rs:289-307 calls it. Also clears `ext_out` (currently missed — latent inconsistency).
- `ospf::tick` called from `Topology::step`, not `services::poll` — removes device→services→ospf cycle; poll keeps dhcp+flows only.
- `Ospf::reset()` method; `ospf_clear` uses it.
- `Link::other` returns `Option<Endpoint>`-ish safety — keep signature, it's fine (only called with real endpoints).

### R7. Prune public surface
- `pub mod eth` → `mod eth` (all callers in-crate; `pub` items inside become crate-visible only).
- lib.rs re-exports: keep `DeviceKind`, `Topology` (only consumers: wasm + tests) + types needed by tests.
- `SimEvent` kind doc comment refreshed to actual vocabulary.

### R8. JS: `util.js` shared helpers + constants
- `maskLen`/`lenMask` (currently ×3: main.js:2622, templates.js:5, cli.js:20-21), `PROTO_NAME`, `protoName`, `TICK_MS` (from capture.js), `$`/`$$`/`esc`/`h`, `clamp`, `b64`/`unb64`, `isTyping`.
- One proto↔name table in JS (dup at main.js:231,2538; cli.js:705; capture.js protoClass).

### R9. JS: fix confirmed bugs found by audit
- `f.color='ospf'` has no `--p-ospf` CSS var → add var + mapping (style.css + protoClass).
- `ifName` c7200/PA-8E off-by-one vs `iosArgs` slot map — centralize naming rule in icons.js and fix ios 8-port c7200 mapping.
- `cli.js` `WORDS` missing `reload`/`no`/`in`/`out`/`any`/`all`/`on`/`off`/`set`/`pcname`/`nc`; expansion limit `i<4`.
- `labyaml.js`: `+l.delay || 4` → `?? 4`; parseLab drops `uid`/`image`/`portCount`/`vm`/`dns`/`notes`/`shapes`/`view` on round-trip → emit+parse them.
- `DEMO()` duplicates `branch-office` template → build from TEMPLATES.

### R10. JS: modularize main.js
`state.js` exporting single mutable `S` object holding all shared state; split main.js into ui/state/history/labops/sim/canvas/input/inspector/dock/files/marketplace/library/consoles + slim boot main.js. Fixes checkpoint→mutate→refresh→dirty ordering via `commit()` helper in history.js; `dockTab` string union stays (low churn, documented); console/emu duck-types get a documented interface comment.

### R11. JS surplus
`capShown`, `chooseImage` alias, `roundRect` dup, `openMenu` unused `onClose`, `readColors` unused keys, ios.js `terminate()`/`typeof opts==='number'`, vm.js `terminate()`, evRendered redundant bookkeeping, `tick` arrow wrapper, `maskLen`/`maskToLen` dups (R8), icon `switch` geometry dup (icons.js:77-92).

### R12. wasm facade
Keep mechanical forwarders (it's the boundary) but: single proto string→u8 helper (3 pasted copies), keep `probe_ttl` name (JS callers rely). Document in log only.

### T. Tests
No can't-fail tests exist (audit). Event-message strings are the de-facto contract — keep wording identical during refactor. Ping.rs `t.events` vs `drain_events` — normalize to drain (cosmetic). Gaps noted but not blocking: OSPF reconvergence, trunk allowed-lists, UDP probe, nat_static_clear.

### C. Comments
Remove restating comments in touched code (e.g. `p[0] = op; // 1 request, 2 reply`, `// htype eth`, `// find mac of the addressed port` on the duplicated dead line, `// 0 + type + class`). Keep external-knowledge ones (wire format offsets, RFC semantics, "emulated nodes", checksum rationale).

---

## PROGRESS CHECKPOINT (working state — update as you go)

DONE (edited, not yet compiling):
- `frame.rs`: added `Frame::ip(src,dst,ttl,payload)`, `Frame::raw_ip(src,dst,ttl,raw)`, `frame.is_broadcast()`.
- `link.rs`: `LinkSpeed::serial_ticks(len)` + `bit_rate()` (delete `serial_ticks` free fn in topology).
- `device.rs`: `RouteProto` enum {Static,Ospf} (Route.proto now enum; derived Ord gives Static<Ospf); `Port::access_vid/native_vid`; `Device::{owns_ip pub(crate), first_ip, mac_of_ip, set_default_route, power_off, power_on}`; powered guard in ext_inject; `forward_if_router` helper unifying 4 match arms; l4_of removed → `crate::services::l4_tuple`; Frame::ip literals; imports `services::{local_l4,nat_egress,nat_ingress}`.
- `eth.rs`: `l4_name` (single proto table, dedup'd raw_label), `icmp_packet(ty,body)`, `build_icmp(...)`, encode ICMP arm uses push_ipv4, removed tautology assert.
- `docs/untangle-decision-log.md` exists.

COMPILATION IS CURRENTLY BROKEN — device.rs references things services doesn't export yet:
`crate::services::{local_l4, nat_egress, nat_ingress, l4_tuple, DhcpState::Bound}`.

TODO in order:
1. Split `core/src/services/mod.rs` → `dhcp.rs` `dns.rs` `flows.rs` `nat.rs` (+semantic edits):
   - `DhcpState` enum {Discover,Request,Bound} replaces ST_* u8 (DhcpClient.state type changes; power_off compares `!= DhcpState::Bound`; dhcp_state() maps).
   - `FlowKind` {TcpProbe,Http,Udp,TcpServer} + `FlowState` {SynSent,Established,Done} enums replace u8; `FlowKind::event_kind()` returns "tcp"/"http"/"udp" (dedups 3 match tables).
   - `DHCP_MAGIC` const for [99,130,83,99] (parse+build).
   - Remove dead: send_tcp shadowed `mac` (dup recompute), send_unreach `build_udp` placeholder → use `eth::build_icmp`; `udp_frame` drop `_port` param; `client_tcp` drop `_ack`/`_in_port`; `send_unreach` drop `tick` param; `alloc_lease` drop `port` param; `dns_response` drop `name` param; `mac6`→`eth::mac_bytes`; `mask_len`→`crate::mask_bits`; `port_label`→inline; `first_ip` free fn → `dev.first_ip()`; ephemeral port formula → `alloc_port` reused by nat_egress.
   - `poll` = dhcp client retry + `flows::poll_flows` only — REMOVE `crate::ospf::tick` from poll (moved to step).
   - mod.rs re-exports so `crate::services::{DhcpClient,Flow,NatEnt,StaticNat,DhcpState,start_dhcp,start_dns,start_tcp,start_udp,nat_ingress,nat_egress,local_l4,l4_tuple,note,dhcp_state}` all resolve.
2. Split `core/src/topology/mod.rs` → `config.rs` `actions.rs` `snapshot.rs` (+edits):
   - mod.rs keeps: Capture, Topology (next_*/rng → `pub(crate)`; devices/links → `pub(crate)`), new/rng/alloc_mac, CRUD, set_link_up/capture/impair, drains, ext_inject, step (ADD `crate::ospf::tick` alongside services::poll), enqueue (use `link.speed.serial_ticks(len)`; delete old serial_ticks fn), inject_link.
   - config.rs `impl Topology`: all setters; set_power → `d.power_off()/d.power_on()`; set_gateway → `d.set_default_route(gw_ip, out_port)`; route pushes `proto: RouteProto::Static`; ospf_clear `r.proto != RouteProto::Ospf`.
   - actions.rs `impl Topology`: ping/ping_ttl (use `Frame::ip(...)`), dhcp_client, dns_query, probe_tcp, http_get, probe_udp.
   - snapshot.rs: Snap structs + snapshot(); `r.proto == RouteProto::Ospf → "O"`; needs `use crate::device::{DeviceKind,NatRole,RouteProto}`.
3. `ospf.rs`: add `Ospf::reset()` (clears nbrs/lsas/next_hello — NOT nets/if_area/on — called by power_off); `hello()` use HELLO_EVERY/DEAD consts; `proto: RouteProto::Ospf`; remove `let _=(tick,events)` in originate (or keep sig for callers); Frame::raw_ip in send().
4. `lib.rs`: `mod eth` private; re-exports = `DeviceKind, DeviceId, LinkId, Topology, Capture, SimEvent` + all `*Snap` structs (snapshot pub fields); helpers `pub(crate)`.
5. `wasm/lib.rs`: one `fn proto_num(s:&str)->Option<u8>` replacing 3 pasted proto matches.
6. `cargo build && cargo test` — event strings must stay identical (tests assert on them).
7. JS phase (web/):
   - `web/util.js`: $,$$,esc,h,clamp,b64,unb64,isTyping,maskLen,lenMask,PROTO_NAME,TICK_MS; rewire imports in main/cli/capture/labyaml/templates.
   - Bug fixes: missing `--p-ospf` CSS var (style.css + capture.js protoClass); ifName c7200 PA-8E off-by-one (icons.js vs ios.js slot map); cli.js `WORDS` missing reload/no/in/out/any/all/on/off/set/pcname/nc + `i<4` expansion limit; labyaml `+l.delay||4`→`??4` + emit/parse missing fields (uid,image,portCount,vm,dns,notes,shapes,view); DEMO()→build from branch-office template.
   - Surplus: capShown, chooseImage alias, roundRect dup, openMenu onClose, readColors unused keys, ios.js terminate()/number coercion, vm.js terminate().
   - main.js modularize via `state.js` `export const S={...}` + split per audit table (17 files). BIG — follow the audit's decomposition table.
8. Comments pass: strip restating comments in touched code; keep wire-format/external-knowledge ones.
9. Final: `node --check` all web/*.js; final re-sweep.

## CHECKPOINT 2 — RUST DONE, all tests green

Applied: frame.rs constructors (`Frame::ip`, `raw_ip`, `is_broadcast`), link.rs `LinkSpeed::serial_ticks`, device.rs (`RouteProto` enum, `Port::access_vid/native_vid`, `Device::{owns_ip,first_ip,mac_of_ip,set_default_route,power_off,power_on}`, powered guard on ext_inject, `forward_if_router` dedup, l4_of→l4_tuple), eth.rs (`l4_name`, `icmp_packet(ty,code,body)`, `build_icmp`, ICMP encode via push_ipv4), services/ split into `mod.rs`+`dhcp.rs`+`dns.rs`+`flows.rs`+`nat.rs` (`DhcpState`,`FlowKind`,`FlowState` enums; dead mac dedup in send_tcp; send_unreach→build_icmp; removed unused params/fns; `alloc_lease` drops port; `dns_response` drops name; `udp_frame` drops port arg; `client_tcp` drops ack/in_port), topology/ split into `mod.rs`+`config.rs`+`actions.rs`+`snapshot.rs` (set_power→power_off/on; set_gateway→set_default_route; `proto:`→RouteProto; snapshot "O"/"S" via RouteProto::Ospf; step() now calls services::poll + ospf::tick — poll no longer ticks ospf), ospf.rs (`Ospf::reset()`, RouteProto), lib.rs (`mod eth` private, helpers pub(crate), re-exports = DeviceId,DeviceKind,LinkId,SimEvent,Capture,Snapshot,Topology), wasm/lib.rs (`nat_proto` helper dedups nat_static+clear proto maps; acl_add keeps its own superset map).

Verified: `cargo build`/`test -p netlab-core` clean, 22/22 tests pass, `cargo build -p netlab-wasm` clean.

NEXT: web/ phase — util.js, bug fixes list, surplus list, main.js split. See "PROGRESS CHECKPOINT" section item 7 for the fix list.

## CHECKPOINT 3 — FINAL STATE

All work complete and verified:
- Rust: cargo build clean (0 warnings), 22/22 tests pass, netlab-wasm builds.
- JS: all 13 web modules pass node --check; labyaml round-trip verified live.
- Post-sweep fixes applied: allTemplates() call bug, --p-ospf/--p-raw CSS + readColors, dead menuOnClose, ios.js opts coercion, Frame literals → constructors throughout, dhcp is_broadcast/owns_ip/mac_of dedups, ospf::tick signature (dropped unused events), inject_link dead fallback removed, events.rs doc updated, poll→pub(crate), catOf 'ospf'→SYS, cli.js maskToLen→maskLen, restore() inline map→PROTO_NAME, ios-sw non-7200 → fa1/i (NM-16ESW).
- NOTE: web/pkg/*.wasm is a checked-in build artifact — run `make wasm` (needs wasm-pack) to ship the new core to the browser.
- Deferred (documented, not done): deep main.js modularization beyond util.js/ui.js — inspector (~620 lines), canvas (~600), panes (~300) need a state/DI design and browser verification; decomposition table is in the audit section above.
