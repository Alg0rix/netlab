# netlab

**netlab** is a zero-backend network simulation and emulation lab that runs entirely in the browser. It combines a deterministic **Rust → WebAssembly** L2/L3 switching and routing fabric with real hardware emulation compiled to WebAssembly—booting real **Cisco 7200 IOS** images via Dynamips and **MikroTik RouterOS / 32-bit x86 VMs** via v86 on the same virtual wire.

No backend server, local agent, or cloud VM is required. Operating system images, NVRAM configurations, virtual disks, and topologies stay locally in your browser (`IndexedDB` / `localStorage`).

![netlab running in the browser](docs/screenshot.png)

🎬 **[Watch the 88-second showreel](docs/netlab-showreel.mp4)**: what netlab is, how it works, and a live demo. It is built entirely from code in [`promo/`](promo/).

---

## Features

- **Hybrid Simulated + Emulated Fabric**
  - **Simulated Nodes (`netlab-core` in Rust/WASM):** Instant-boot virtual PCs (VPCS), laptops, servers, 8-port L2 learning Ethernet switches, and 4-port L3 IPv4 routers with ARP, static routing, and ICMP echo/reply.
  - **Real Emulated NOS (`Dynamips` & `v86` in WASM):** Wire real Cisco 7200 routers, MikroTik RouterOS v6 instances, and custom 32-bit x86 virtual machines directly into simulated switches and routers using real Ethernet frames.
- **Realtime & Step-by-Step Simulation Modes**
  - **Realtime Mode:** Runs the switching/routing fabric at ~1 tick/ms so emulated OS guests experience LAN latency and responsive interactive consoles.
  - **Simulation Mode:** Packet Tracer–style visual packet animation across links with Play/Pause, single-tick stepping (`.`), and adjustable speed (`0.25×` to `4×`).
- **Built-in Packet Capture & Wireshark Export**
  - Attach a live sniffer to any link. Inspect frames with a built-in protocol dissector (Ethernet II, 802.1Q, ARP, IPv4, ICMP, TCP, UDP, OSPF, EIGRP), interactive hex dump, and one-click **`.pcap` export** for Wireshark.
- **Interactive Consoles**
  - **Simulated CLI:** Built-in VPCS shell for hosts and a Cisco-like CLI for switches and routers featuring Tab completion, command abbreviation (`sh ip int br`, `sh ip ro`, `sh run`), and contextual `?` help.
  - **Emulated Terminals:** Full `xterm.js` serial console and VGA screen tabs for Cisco IOS, RouterOS, and x86 VMs.
- **Diagramming & Portable Labs**
  - Infinite pan/zoom canvas with grid snap, minimap, interface labels, sticky notes, vector shapes (rectangles, ellipses, lines), dark/light theme toggle, PNG diagram export, and portable `.netlab.json` topology files.

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) and [`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/) (to build the Rust simulation fabric)
- Python 3 (for the local static HTTP server)
- Docker *(optional — only needed if you want to recompile Dynamips from source; prebuilt `web/dynamips.{js,wasm}` artifacts are included)*

### Build & Run

```sh
# 1. Compile the Rust switching/routing fabric to WebAssembly (outputs to web/pkg/)
make wasm

# 2. Start the local web server on http://localhost:8471
make serve
```

Then open **`http://localhost:8471`** in a modern desktop browser (Chromium, Firefox, or Safari).

> **Why `make serve`?**
> Emulated Cisco IOS uses WebAssembly pthreads backed by `SharedArrayBuffer`, which requires the page to be served in a cross-origin isolated context (`Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`). `web/serve.py` sets these headers automatically. If you host `web/` on static hosting (Cloudflare Pages, Vercel, Netlify, GitHub Pages + service worker, or Nginx), configure those two HTTP response headers to enable Cisco IOS nodes.

---

## Device Catalog & Bring Your Own Image (BYOI)

Nothing proprietary ships with **netlab**. Simulated nodes work out of the box; for emulated nodes, you supply your own images via **Lab → Images…** (or directly in the node inspector). Images are stored locally in your browser's `IndexedDB`.

| Device | Type | Engine | Image Required | Interfaces |
|---|---|---|---|---|
| **VPCS / Laptop / Server** | Simulated Host | `netlab-core` (Rust/WASM) | None (built-in) | `eth0` |
| **Ethernet Switch** | Simulated L2 Switch | `netlab-core` (Rust/WASM) | None (built-in) | `e0` – `e7` (8 ports) |
| **Router** | Simulated L3 Router | `netlab-core` (Rust/WASM) | None (built-in) | `g0/0` – `g0/3` (4 ports) |
| **Cisco 7200** | Emulated Router | Dynamips → WASM | c7200 IOS `.bin` (uncompressed ELF) | `f0/0` – `f3/0` (4 × FastEthernet) |
| **MikroTik RouterOS** | Emulated Router | v86 (32-bit x86) | RouterOS **v6** x86 `.iso` or raw `.img` | `ether1` |
| **x86 VM** | Emulated Host/Appliance | v86 (32-bit x86) | Any 32-bit x86 `.iso` or raw `.img` | `eth0` |

### Emulated Image Notes

- **Cisco 7200 IOS (`Dynamips`):**
  - Supply an uncompressed `c7200` IOS `.bin` ELF image (if your image is a compressed `.bin`, decompress it first with `unzip -p image.bin > image.unpacked.bin`).
  - NVRAM (`write memory` / `copy running-config startup-config`) is automatically snapshotted to `IndexedDB` and is embedded inside exported `.netlab.json` lab files so your router configs travel with the lab.
- **MikroTik RouterOS (`v86`):**
  - **Use RouterOS v6 (32-bit x86)**, e.g., `mikrotik-6.49.18.iso`. MikroTik CHR and RouterOS v7 require a 64-bit CPU (`x86-64`) and cannot boot in `v86`.
  - On first boot from an ISO, open the node's **Screen** tab and install RouterOS onto the node's blank virtual disk (press `a`, `i`, `n`, `y`, then `Enter` to reboot). Subsequent boots run directly from the installed per-node disk and use the **Serial** console tab.
  - Without a license key, RouterOS v6 runs in free trial mode with a 24-hour uptime countdown shown at login.
- **General x86 VMs & Virtual Disks:**
  - Disk images must be **raw** format. Convert `qcow2`, `vmdk`, or `vdi` images beforehand:
    ```sh
    qemu-img convert -O raw input.qcow2 output.img
    ```
  - Each VM's writable disk is persisted to `IndexedDB` ~8 seconds after guest disk writes and on node Stop. Orphans from deleted labs can be cleaned up in **Lab → Images…**.
  - `v86` emulates a single NIC (`ne2k`/`virtio`), so x86 VMs expose one Ethernet interface. Note that RouterOS binds its initial MAC address into its disk configuration—cloning an already-installed raw disk across multiple nodes will also clone that MAC unless reset inside the guest.
- **Background Tab Throttling:**
  - Browsers throttle timers in background tabs, which slows down emulated guest traffic while the netlab tab is hidden.

---

## Simulated CLI Reference

Double-click any node on the canvas (or select it and open the **Console** tab in the bottom dock) to access its interactive shell. Both shells support **Tab** completion, command history (`↑`/`↓`), and contextual **`?`** help.

### End Hosts (`VPCS` / `Laptop` / `Server`)

```text
ip <addr/len> [gateway]     Configure IPv4 address, subnet mask, and optional default gateway
                            Example: ip 192.168.10.11/24 192.168.10.1
clear ip                    Remove the configured IPv4 address
show ip                     Display IPv4 address, gateway, and MAC address
show arp | arp              Display the ARP table
show route                  Display the routing table
ping <ip> [-c <count>]      Send ICMP echo requests
hostname <name>             Rename the host
reload                      Reset dynamic state (ARP cache)
```

### Simulated Switches & Routers

Abbreviations work just like IOS (`sh ip int br`, `sh ip ro`, `sh mac addr`, `sh run`):

```text
show ip interface brief     Summary of interfaces, IPv4 addresses, and link status
show interface              Detailed interface counters and MAC addresses
show ip route               Display connected and static routes (Router)
show ip arp | show arp      Display the ARP cache
show mac address-table      Display learned L2 forwarding table (Switch)
show running-config         Print current node configuration as CLI commands
ip address <if> <addr/len>  Assign an IPv4 address to an interface (Router)
no ip address <if>          Remove an interface's IPv4 address (Router)
ip route <prefix/len> <next-hop> [if]
                            Add a static route (Router)
no ip route <index>         Remove a static route by its index in `show ip route`
ping <ip> [-c <count>]      Send ICMP echo requests from the device
hostname <name>             Rename the device
reload                      Clear learned ARP and MAC tables
```

---

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `V` | Select tool |
| `C` | Link (cable) tool |
| `N` | Sticky note tool |
| `R` / `O` / `/` | Draw Rectangle / Ellipse / Line (`Shift` constrains aspect/angle) |
| `Space` + drag | Pan canvas |
| `+` / `-` / `0` | Zoom in / Zoom out / Reset zoom (100%) |
| `F` | Fit topology to view |
| `L` / `G` / `M` | Toggle interface labels / grid snap / minimap |
| `Shift` + `Space` | Play / Pause in Simulation mode |
| `.` | Step forward one simulation tick |
| `` ` `` | Toggle bottom dock (Events, Capture, Consoles) |
| `F2` | Rename selected node |
| `Ctrl/Cmd` + `D` | Duplicate selection |
| `Ctrl/Cmd` + `S` / `O` | Save `.netlab.json` / Open `.netlab.json` |
| `Delete` / `Backspace` | Delete selected items |
| `?` | Show keyboard shortcuts modal |

---

## Architecture & Repository Layout

```text
netlab/
├── core/                  # Pure-Rust network simulation engine (netlab-core)
│   ├── src/
│   │   ├── device.rs      # Host, Switch, Router, and Ext (emulated bridge) state
│   │   ├── eth.rs         # Real Ethernet II / ARP / IPv4 / ICMP wire frame codec
│   │   ├── frame.rs       # Internal simulation frame representations
│   │   ├── link.rs        # Point-to-point links, latency ticks, in-flight packets
│   │   ├── topology.rs    # Tick-driven L2 learning, L3 longest-prefix match, captures
│   │   └── events.rs      # Structured simulation events for UI inspection
│   └── tests/             # Integration tests (ping, L2/L3 forwarding, wire codec)
├── wasm/                  # wasm-bindgen bindings exposing NetLab to JavaScript
├── emu/                   # Dynamips -> WebAssembly port
│   ├── fetch-dynamips.sh  # Clones upstream GNS3/dynamips @ pinned commit + applies patch
│   ├── dynamips.patch     # Emscripten/WASM port patch (wasm NIO, VTTY, callback arity fixes)
│   ├── wasm/nl_bridge.c   # Shared-memory ring buffers for Ethernet frames, console & NVRAM
│   ├── shim/              # Minimal big/little-endian ELF32 loader shim (libelf)
│   └── WASM-PORT.md       # Technical write-up of the Dynamips WebAssembly port
└── web/                   # Zero-build ES module frontend
    ├── main.js            # Canvas topology editor, simulation loop, inspector, storage
    ├── cli.js             # Interactive VPCS & router/switch CLI with Tab/? completion
    ├── capture.js         # Packet dissector, hex viewer, and libpcap (.pcap) writer
    ├── ios.js             # Cisco 7200 worker manager & NVRAM sync
    ├── ios_worker.js      # Web Worker hosting the modularized Dynamips WASM build
    ├── vm.js              # v86 emulator integration (MikroTik RouterOS & x86 VMs)
    ├── store.js           # IndexedDB persistence for images, NVRAM, and VM disks
    └── serve.py           # Local dev server with COOP/COEP headers
```

### Rebuilding Dynamips WASM Artifacts (Optional)

Prebuilt `web/dynamips.js` and `web/dynamips.wasm` are checked into the repository. If you modify `emu/dynamips.patch` or `emu/wasm/nl_bridge.c`, rebuild them with Docker (`emscripten/emsdk:3.1.74`):

```sh
make emu && make emu-artifacts
```

See [`emu/WASM-PORT.md`](emu/WASM-PORT.md) for deep architectural details on how Dynamips' MIPS64 CPU core, VTTY console, NVRAM mmap zone, and `wasm:` NIO driver bridge into the browser.

---

## Running Tests

Run the Rust simulation core test suite (covering L2 learning, L3 static routing, ARP resolution, ICMP, link/power controls, and real Ethernet wire encoding/decoding):

```sh
cargo test -p netlab-core
```

---

## Third-Party Notices

- **Dynamips (`web/dynamips.{js,wasm}`, `emu/`):** Derived from [GNS3/dynamips](https://github.com/GNS3/dynamips) (GPL-2.0). Upstream source is fetched on demand via `emu/fetch-dynamips.sh` and patched with `emu/dynamips.patch`.
- **v86 (`web/v86/`):** Vendored `v86@0.5.472` (BSD-2-Clause) with unmodified `seabios.bin` and `vgabios.bin` (LGPL-3.0). See [`web/v86/NOTICE.md`](web/v86/NOTICE.md).
- **xterm.js (`web/vendor/xterm/`):** Vendored `@xterm/xterm@5.5.0` and `@xterm/addon-fit@0.10.0` (MIT). See [`web/vendor/xterm/LICENSE`](web/vendor/xterm/LICENSE).
