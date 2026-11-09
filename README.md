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

## Bring your own image

Nothing proprietary ships with netlab. Images you add stay in the browser (IndexedDB).

| Node | Engine | Image | Interfaces |
|---|---|---|---|
| Cisco 7200 | Dynamips → WASM | c7200 IOS `.bin` (ELF) | 4 × FastEthernet |
| MikroTik RouterOS | v86 (32-bit x86) | RouterOS **v6** x86 install ISO, or a raw disk with v6 installed | 1 (`ether1`) |
| x86 VM | v86 (32-bit x86) | any 32-bit x86 ISO or raw `.img` | 1 |

- MikroTik **CHR and RouterOS v7 are 64-bit** and cannot run in v86 (the v7 kernel stops with
  "requires an x86-64 CPU"). Use RouterOS v6 (e.g. `mikrotik-6.49.18.iso`); unlicensed x86 runs in
  its 24 h trial mode, after which some features are limited — your licence is your business.
- Raw images only: convert qcow2/VMDK/VDI with `qemu-img convert -O raw in.qcow2 out.img`.
- First start of an ISO installs onto a blank per-node disk via the **Screen** tab (RouterOS:
  `a`, `i`, `n`, `y`, Enter). After that the VM boots from its disk and the **Serial** tab is the console.
- Each VM's disk is saved to the browser ~8 s after the guest writes and on Stop (not in lab files).
  IOS `write memory` NVRAM is saved likewise and *is* embedded in lab files.
- v86 emulates a single NIC, so x86 VMs have one interface. RouterOS keeps the MAC it first saw on
  its disk; cloning one installed raw image into several nodes clones that MAC too.
- Realtime mode runs the fabric at ~1 tick/ms so real guests see LAN latency; Simulation mode
  animates packets tick by tick.

## Layout

- `core/` — simulation engine (ARP, L2 learning, L3 routing, ICMP, link/power state, capture)
- `wasm/` — wasm-bindgen API over `core`
- `web/` — UI (canvas topology editor, consoles, packet capture, IOS worker, x86 VMs in `vm.js`)
- `web/v86/`, `web/vendor/xterm/` — vendored v86 0.5.472 (BSD-2) and xterm.js 5.5.0 (MIT)
- `emu/` — Dynamips WASM port: patch, bridge (`wasm/nl_bridge.c`), shims; see `emu/WASM-PORT.md`

## Tests

```sh
cargo test -p netlab-core
```
