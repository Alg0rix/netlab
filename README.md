# netlab

A network lab that runs entirely in the browser: a Rust → WebAssembly
switching/routing fabric, plus real Cisco IOS booted inside Dynamips compiled
to WebAssembly. No backend; images, configs and labs stay in the browser.

## Run

```sh
make wasm     # Rust fabric -> web/pkg (needs wasm-pack)
make serve    # http://localhost:8471 (COOP/COEP headers, required for IOS)
```

`web/dynamips.{js,wasm}` are committed. To rebuild them (needs Docker):

```sh
make emu && make emu-artifacts   # clones GNS3/dynamips @ pinned rev, applies emu/dynamips.patch
```

## Layout

- `core/` — simulation engine (ARP, L2 learning, L3 routing, ICMP, link/power state, capture)
- `wasm/` — wasm-bindgen API over `core`
- `web/` — UI (canvas topology editor, consoles, packet capture, IOS worker)
- `emu/` — Dynamips WASM port: patch, bridge (`wasm/nl_bridge.c`), shims; see `emu/WASM-PORT.md`

## Tests

```sh
cargo test -p netlab-core
```
