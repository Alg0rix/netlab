# netlab

**netlab** is a zero-backend network simulation and emulation lab that runs entirely in the browser. It combines a deterministic **Rust → WebAssembly** L2/L3 switching and routing fabric with real hardware emulation compiled to WebAssembly—booting real **Cisco IOS routers & switches** (`c7200` / `c3725` with `NM-16ESW` 16-port EtherSwitch ASIC) via Dynamips and **MikroTik RouterOS / CRS Switches / Open vSwitch (Linux) / 32-bit x86 VMs** via v86 on the same virtual wire.

No backend server, local daemon, or cloud VM is required. Operating system images, NVRAM configurations, virtual disks, and topologies stay locally in your browser (`IndexedDB` / `localStorage`).

**Live demo:** https://netlab.rixlab.xyz

![netlab running in the browser](docs/screenshot.png)

---

## Features

- **Hybrid Simulated + Emulated Fabric**
  - **Simulated Nodes (`netlab-core` in Rust/WASM):** Instant-boot virtual PCs (VPCS), laptops, servers, L2 learning Ethernet switches (up to 16 ports), and L3 IPv4 routers (up to 8 ports) with ARP, static routing, ICMP, DHCP (client and server), DHCP relay, DNS, HTTP, TCP/UDP probes, source NAT (masquerade), extended ACLs, and a simplified OSPFv2. The simulated switch speaks 802.1Q: every port starts as access VLAN 1, so an existing flood still works, and a tag for any other VLAN is dropped until the port is a trunk.
  - **Real Emulated Routers & Managed Switches (`Dynamips` & `v86` in WASM):**
    - **Cisco 7200 Router (`ios`):** Full Cisco IOS L3 router (`f0/0`–`f6/0` + `f0/1`).
    - **Cisco Switch (`ios-sw`):** Up to 16-port managed L2/L3 switch via Dynamips **`NM-16ESW`** hardware EtherSwitch module (`fa1/0`–`fa1/15` on `c3725`/`c3745`/`c2691` images with hardware 802.1Q VLANs, Trunking, VTP, PVST+/STP, Port-Channel, and L3 SVIs) or **`PA-8E`** bridge groups (`e1/0`–`e2/7` on `c7200` images).
    - **MikroTik RouterOS (`routeros`) & MikroTik CRS Switch (`mikrotik-sw`):** Real RouterOS v6 x86 with 8 Ethernet ports (`ether1`–`ether8`), Bridge VLAN Filtering (`802.1Q` / `802.1ad` QinQ), STP/RSTP/MSTP, LACP (`802.3ad`), OSPF, BGP, VRRP, MPLS, and Firewall/NAT.
    - **Open vSwitch / Linux Switch (`ovs-sw`) & x86 VM (`x86`):** 8-port 32-bit x86 VM (`eth0`–`eth7`) for running Linux with Open vSwitch (`ovs-vsctl`), `iproute2` VLAN-aware bridges, or custom appliances.
    - **FRRouting (`frr`):** 8-port 32-bit Linux router (`eth0`–`eth7`) with a `vtysh` starter script for OSPF, IS-IS, BGP EVPN and BFD.
    - **Firewall (`firewall`):** 8-port perimeter appliance (`wan0`, `dmz0`, `lan0`–`lan5`) with a stateful `nftables` forward-chain starter script.
- **Multi-Ethernet Interface Support**
  - Configure multi-homed hosts, bastion servers, routers, switches, and emulated VMs directly from the Inspector (**`+ Add`** / **`- Remove`** interface buttons).
  - Supports up to **8 NICs** on End Hosts (`eth0`–`eth7`), Routers (`g0/0`–`g0/7`), Cisco 7200 (`f0/0`–`f6/0`, `f0/1`), MikroTik RouterOS & CRS Switch (`ether1`–`ether8`), Open vSwitch & x86 VMs (`eth0`–`eth7`), and up to **16 ports** on Ethernet Switches (`e0`–`e15`) and Cisco Switches (`fa1/0`–`fa1/15`).
- **Topology Template Marketplace (32 Curated Labs · Beginner → Expert)**
  - Press **`T`** (or click **Templates** in the top bar) to browse **32 ready-to-run network topologies** with auto-generated SVG schematics, difficulty tabs (`Beginner`, `Intermediate`, `Advanced`, `Expert`), category & tag filters, and step-by-step lab objectives:
    - **Beginner (6):** Point-to-Point Ethernet, Switched LAN, Single-Router Subnets, Cascaded Switches, SOHO Home Gateway, CIDR/VLSM Subnetting.
    - **Intermediate (11):** Branch Office (Cisco IOS Edge), Multi-Hop WAN Triangle, Multi-Homed DMZ Bastion, Hub-and-Spoke WAN, 5-Hop Traceroute Chain, Metro Fiber Ring, 3-Tier Microservices, Access VLANs and an 802.1Q Trunk, DHCP Relay, Two-Router OSPF, Static NAT and a Port Forward.
    - **Advanced (8):** Campus 3-Tier Hierarchy, Dual-ISP Enterprise Edge, IXP 4-AS Peering Fabric, Industrial ICS/SCADA Purdue Model, Smart Hospital PACS/HL7 Campus, Multi-Region Anycast CDN, **Cisco Switch 802.1Q VLAN Trunking & PVST+**, **MikroTik CRS Bridge VLAN Filtering & RSTP**.
    - **Expert (7):** Leaf-Spine (Clos) Data Center, Multi-Vendor Emulation (Cisco IOS + RouterOS + x86 VM), 4-Region Full-Mesh ($K_4$) Core, Zero-Trust SOC Cyber Range (4-NIC SIEM), Carrier MPLS P/PE Core, AI/GPU Rail-Optimized Training Cluster, **Multi-Vendor Managed Switching Ring (Cisco Switch + MikroTik CRS + Open vSwitch)**.
  - Publish any custom canvas topology to **My Templates** (`localStorage`) or export as `.netlab.json`.
- **Realtime & Step-by-Step Simulation Modes**
  - **Realtime Mode:** Runs the switching/routing fabric at ~1 tick/ms so emulated OS guests experience LAN latency and responsive interactive consoles.
  - **Simulation Mode:** Packet Tracer–style visual packet animation across links with Play/Pause, single-tick stepping (`.`), and adjustable speed (`0.25×` to `4×`).
- **Link quality**
  - Select a cable and set delay, jitter, loss, MTU, and a speed badge (`10M`, `100M`, `1G`, `10G`, `WAN Serial`). `10M`, `100M` and WAN Serial also add serialization delay. The canvas labels a link when it is not a plain 1G cable.
- **Startup scripts**
  - Every node has a plain-text startup script in the inspector (IOS config, MikroTik `.rsc`, or a shell script). **Apply** pastes it into the console. Simulated nodes can apply it on start, and `write memory` on a simulated node saves the running configuration into this script. IOS NVRAM on emulated Cisco nodes stays separate.
- **Built-in Packet Capture & Wireshark Export**
  - Attach a live sniffer to any link. Filter the live list (`stp`, `pvst`, `vlan == 10`, `arp`, `icmp`, `ospf`, `ip == 10.0.0.1`), with row colours for ARP, ICMP, TCP, UDP, DHCP, DNS, STP/PVST+, 802.1Q and routing protocols. Replay a captured frame, inject a crafted Ethernet/VLAN ARP or ICMP frame, or **Follow TCP** to concatenate the payload of the selected 4-tuple in both directions. Inspect frames with a built-in protocol dissector (Ethernet II, **802.1Q VLAN**, **802.1ad QinQ**, **STP / RSTP / PVST+ BPDUs**, **CDP / VTP**, **LLDP**, **LACP**, ARP, IPv4, ICMP, TCP, UDP, DHCP, DNS, HTTP, GRE, OSPF, EIGRP, VRRP), interactive hex dump, and one-click **`.pcap` export** for Wireshark.
- **Interactive Consoles**
  - **Simulated CLI:** Built-in VPCS shell for hosts and an IOS-style CLI for switches and routers. It has a mode ladder, Tab completion, and contextual `?` help. It also supports abbreviations (`sh ip int br`, `sh run`), confirmation dialogs, and script paste.
  - **Emulated Terminals:** Full `xterm.js` serial console and VGA screen tabs for Cisco IOS, RouterOS, Open vSwitch, and x86 VMs.
- **Diagramming & Portable Labs**
  - Infinite pan/zoom canvas with grid snap, minimap, interface labels, sticky notes, vector shapes (rectangles, ellipses, lines), dark/light theme toggle, PNG diagram export, and portable `.netlab.json` topology files. **File → Save YAML lab…** writes a readable `.netlab.yaml` of the same topology (startup blocks included; see [`docs/lab-format.md`](docs/lab-format.md)). **Edit → Undo / Redo** (`Ctrl+Z` / `Ctrl+Y`) restores a topology snapshot and stops running VMs. **Lab → Snapshots…** stores a named copy of the topology plus that moment's IOS NVRAM and VM disks.

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

### Docker

No Rust toolchain needed — the prebuilt `web/pkg/` artifacts are committed, so the image is just nginx serving `web/` with the COOP/COEP headers already configured:

```sh
docker compose up --build
```

Then open **`http://localhost:8471`**.

> **Why `make serve`?**
> Emulated Cisco IOS uses WebAssembly pthreads backed by `SharedArrayBuffer`, which requires the page to be served in a cross-origin isolated context (`Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`). `web/serve.py` sets these headers automatically, and netlab ships `web/_headers` with the same policy for hosts that honor it (including Cloudflare). On other static hosting (Vercel, Netlify, GitHub Pages + service worker, or Nginx), configure those two HTTP response headers to enable Cisco IOS nodes.

---

## Deploy to Cloudflare

Host your own copy as a Cloudflare Worker serving the static `web/` directory — no build step needed, the prebuilt `web/pkg/` wasm-bindgen output and COOP/COEP `_headers` are committed:

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/Alg0rix/netlab)

The button clones the repository into your GitHub account and deploys it to a `<name>.<subdomain>.workers.dev` URL. Or deploy from the command line with [Wrangler](https://developers.cloudflare.com/workers/wrangler/):

```sh
npx wrangler login
npx wrangler deploy
```

To attach a custom domain, add it under `routes` in [`wrangler.jsonc`](wrangler.jsonc) (the zone must be active on your Cloudflare account). `make wasm` is only needed if you modify `core/` or `wasm/`.

---

## Device Catalog & Bring Your Own Image (BYOI)

Nothing proprietary ships with **netlab**. Simulated nodes work out of the box; for emulated nodes, you supply your own images via **Lab → Images…** (or directly in the node inspector). Images are stored locally in your browser's `IndexedDB`.

| Device | Type | Engine | Image Required | Interfaces (Default → Max) |
|---|---|---|---|---|
| **VPCS / Laptop / Server** | Simulated Host | `netlab-core` (Rust/WASM) | None (built-in) | `eth0` → up to `eth7` (8 NICs) |
| **Ethernet Switch** | Simulated L2 Switch | `netlab-core` (Rust/WASM) | None (built-in) | `e0`–`e7` → up to `e15` (16 ports) |
| **Router** | Simulated L3 Router | `netlab-core` (Rust/WASM) | None (built-in) | `g0/0`–`g0/3` → up to `g0/7` (8 ports) |
| **Cisco 7200** | Emulated Router | Dynamips → WASM | `c7200` / `c3725` IOS `.bin` (uncompressed ELF) | `f0/0`–`f3/0` → up to 8 FastEthernet ports |
| **Cisco Switch** | Emulated Managed L2/L3 Switch | Dynamips (`NM-16ESW` / `PA-8E`) → WASM | `c3725` IOS `.bin` (recommended for `NM-16ESW`) or `c7200` IOS `.bin` | `fa1/0`–`fa1/7` → up to `fa1/15` (16 switchports) |
| **MikroTik RouterOS** | Emulated Router | v86 (32-bit x86) | RouterOS **v6** x86 `.iso` or raw `.img` | `ether1`–`ether4` → up to `ether8` (8 NICs) |
| **MikroTik CRS Switch** | Emulated Managed Switch | v86 (32-bit x86) | RouterOS **v6** x86 `.iso` or raw `.img` | `ether1`–`ether8` (8 switchports) |
| **Open vSwitch / Linux** | Emulated Managed Switch | v86 (32-bit x86) | 32-bit Linux `.iso` or raw `.img` (with OVS / `iproute2`) | `eth0`–`eth7` (8 switchports) |
| **FRRouting** | Emulated Router | v86 (32-bit x86) | 32-bit Linux `.iso` or raw `.img` with FRR | `eth0`–`eth7` (8 NICs) |
| **Firewall** | Emulated Security Gateway | v86 (32-bit x86) | 32-bit Linux `.iso` or raw `.img` with nftables or iptables | `wan0`, `dmz0`, `lan0`–`lan5` |
| **x86 VM** | Emulated Host/Appliance | v86 (32-bit x86) | Any 32-bit x86 `.iso` or raw `.img` | `eth0`–`eth1` → up to `eth7` (8 NICs) |

### Emulated Image Notes

- **Cisco 7200 Router & Cisco Switch (`Dynamips`):**
  - Supply an uncompressed `c3725` or `c7200` IOS `.bin` ELF image (if your image is a compressed `.bin`, decompress it first with `unzip -p image.bin > image.unpacked.bin`).
  - **Cisco Switch (`ios-sw`):** Automatically detects the IOS image platform from the ELF header:
    - With a **`c3725` / `c3745` / `c2691`** image (e.g. `c3725-adventerprisek9-mz.124-15.T14.bin`), netlab inserts the **`NM-16ESW`** 16-port hardware EtherSwitch module (`FastEthernet1/0`–`FastEthernet1/15` backed by Dynamips's Broadcom BCM5600 ASIC emulation), giving you full Cisco Catalyst–style CLI (`vlan database`, `switchport mode access/trunk`, `spanning-tree vlan`, `channel-group`, and `interface Vlan<id>`).
    - With a **`c7200`** image, netlab inserts **`PA-8E`** 8-port Ethernet adapters (`Ethernet1/0`–`Ethernet2/7`), supporting IEEE 802.1D Spanning Tree and Integrated Routing and Bridging (`bridge irb`, `bridge 1 protocol ieee`, `bridge-group 1`, `interface BVI1`).
  - NVRAM (`write memory` / `copy running-config startup-config`) is automatically snapshotted to `IndexedDB` on both `c7200` and `c3725` platforms and is embedded inside exported `.netlab.json` lab files.
- **MikroTik RouterOS & MikroTik CRS Switch (`v86`):**
  - **Use RouterOS v6 (32-bit x86)**, e.g., `mikrotik-6.49.18.iso`. MikroTik CHR and RouterOS v7 require a 64-bit CPU (`x86-64`) and cannot boot in `v86`.
  - On first boot from an ISO, open the node's **Screen** tab and install RouterOS onto the node's blank virtual disk (press `a`, `i`, `n`, `y`, then `Enter` to reboot). Subsequent boots run directly from the installed per-node disk and use the **Serial** console tab.
  - On **MikroTik CRS Switch (`mikrotik-sw`)**, all 8 `ether1`–`ether8` interfaces are enabled by default so you can configure `/interface bridge add name=br0 vlan-filtering=yes protocol-mode=rstp`, `/interface bridge port`, `/interface bridge vlan`, and `/interface bonding`.
- **General x86 VMs & Virtual Disks:**
  - Disk images must be **raw** format. Convert `qcow2`, `vmdk`, or `vdi` images beforehand:
    ```sh
    qemu-img convert -O raw input.qcow2 output.img
    ```
  - Each VM's writable disk is persisted to `IndexedDB` ~8 seconds after guest disk writes and on node Stop. Orphans from deleted labs can be cleaned up in **Lab → Images…**.
  - `v86` supports up to 8 PCI `ne2k` Ethernet adapters (`eth0`–`eth7`), each with a deterministic per-node/per-port MAC (`02:6e:00:xx:xx:pp`). Note that RouterOS binds its initial MAC addresses into its disk configuration—cloning an already-installed raw disk across multiple nodes will also clone those MACs unless reset inside the guest.
- **Background Tab Throttling:**
  - Browsers throttle timers in background tabs, which slows down emulated guest traffic while the netlab tab is hidden.

---

## Simulated CLI Reference

Double-click a simulated node on the canvas, or select it and open the **Console** tab in the bottom dock. Hosts, routers, and switches use an IOS-style CLI engine from `netlab-core`. The console runs that engine directly. Emulated nodes keep their serial and VGA terminals.

The full command surface, the mode ladder, confirmation dialogs, and the list of unsupported commands are in [`docs/cli.md`](docs/cli.md). [`docs/wasm-api.md`](docs/wasm-api.md) describes the JavaScript exports. [`docs/lab-format.md`](docs/lab-format.md) describes how a lab file stores configuration. [`docs/corpus.md`](docs/corpus.md) describes the transcript tests.

### Console keys

| Key | Action |
|---|---|
| `?` | List the keywords that can come next |
| `Tab` | Complete the word. Several matches print as a list |
| `↑` / `↓` | Recall earlier lines |
| `Ctrl+C` | Cancel a dialog. Stop a running ping or traceroute from printing |
| `Ctrl+Z` | Jump to privileged exec |
| `Ctrl+A` / `Ctrl+E` | Move to the start or end of the line |
| `Ctrl+B` / `Ctrl+F` | Move back or forward one character |
| `Ctrl+U` / `Ctrl+W` | Delete the line, or the word before the cursor |
| `Ctrl+L` | Clear the scrollback |
| Paste | Run every pasted line as one script |

### Modes (routers and switches)

```text
R1>                 user exec            enable ->
R1#                 privileged exec      configure terminal ->
R1(config)#         global config        interface <if> | router ospf <pid> | vlan <id>
R1(config-if)#      interface            exit | end
```

`end` and `Ctrl+Z` return to privileged exec. `do <command>` runs a privileged exec command from any config mode. Abbreviations work (`sh ip int br`, `conf t`).

Router interfaces start administratively down. Enter `no shutdown` on each interface you use.

### End hosts (`VPCS` / `Laptop` / `Server`)

```text
ip <addr/len> [gw]          Set the IPv4 address and optional gateway
ip <addr> <mask> [gw]
ip dhcp                     Request a lease
ping <ip> | trace <ip>      Send ICMP echo requests, or trace the path
arp | show arp | show ip    Show the ARP table, or the address and gateway
set pcname <name>           Rename the host
```

### Routers and switches

```text
show ip interface brief | show interfaces | show running-config | show version
show ip route | show arp | show ip ospf [neighbor | interface]      (router)
show mac address-table | show vlan brief | show switchport          (switch)
ping <ip> | traceroute <ip>

interface <if>
  ip address <ip> <mask>                  (router)
  ip nat inside | outside                 (router)
  ip access-group <1-2699> in|out         (router)
  switchport mode access | trunk          (switch)
  switchport access vlan <id>             (switch)
  switchport trunk allowed vlan <list>    (switch)
  shutdown | no shutdown
ip route <net> <mask> <next-hop> [distance]
access-list <1-2699> permit|deny ip|icmp|tcp|udp <addr> <addr>
ip nat inside source static <inside-local> <inside-global>
service dhcp | ip dhcp excluded-address <low> [<high>]
router ospf <pid>  ->  network <net> <wildcard> area <n> | router-id <ip>
vlan <id>  ->  name <word> | state active|suspend
```

Static routes beat OSPF when the prefix length is the same. A longer OSPF prefix still wins. Connected routes always win. OSPF routes are not written back as `ip route`.

### Saving and reloading

- `write memory` stores the running configuration as the node's startup script and turns on **Apply on start**. The script shows in the Inspector under **Startup script**.
- `write erase` deletes the startup script and turns **Apply on start** off.
- `reload` restores the configuration the lab loaded with. It clears learned state (ARP, MAC, NAT translations, OSPF neighbors). It then runs the startup script if **Apply on start** is on. Unsaved changes vanish.

[`docs/cli.md`](docs/cli.md#saving-erasing-and-reloading) lists the full rules. Forms from older versions of the console, such as `ip route <prefix/len> <next-hop>` on one line, are in [`docs/cli.md`](docs/cli.md#forms-from-the-earlier-browser-shell).

---

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `T` | Open **Topology Template Marketplace** (Beginner → Expert labs) |
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
| `Ctrl/Cmd` + `S` / `O` | Save `.netlab.json` / Open `.netlab.json` or `.netlab.yaml` |
| `Ctrl/Cmd` + `Z` / `Y` | Undo / redo the topology. This stops running VMs |
| `Delete` / `Backspace` | Delete selected items |
| `?` | Show keyboard shortcuts modal |

---

## Architecture & Repository Layout

```text
netlab/
├── core/                  # Pure-Rust network simulation engine (netlab-core)
│   ├── src/
│   │   ├── cli/           # IOS-style CLI engine: grammar, modes, dialogs, scripts (see docs/cli.md)
│   │   ├── device.rs      # Host, Switch, Router, and Ext (emulated bridge) state
│   │   ├── eth.rs         # Real Ethernet II / ARP / IPv4 / ICMP / TCP / UDP wire codec
│   │   ├── services.rs    # DHCP, DNS, HTTP, TCP/UDP probes, source NAT, DHCP relay
│   │   ├── ospf.rs        # Simplified OSPFv2 hellos, router LSA, SPF
│   │   ├── frame.rs       # Internal simulation frame representations
│   │   ├── link.rs        # Point-to-point links, delay, jitter, loss, MTU, speed
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
    ├── main.js            # Canvas topology editor, simulation loop, inspector, marketplace UI
    ├── templates.js       # 32 built-in topology templates (Beginner → Expert) + SVG previewer
    ├── cli.js             # Console adapter: connects the CLI engine to the keyboard and the scrollback
    ├── capture.js         # Packet dissector, TCP stream follow, hex viewer, libpcap writer
    ├── labyaml.js         # .netlab.yaml / .netlab.json lab-document codec (see docs/lab-format.md)
    ├── ios.js             # Cisco 7200 worker manager & NVRAM sync
    ├── ios_worker.js      # Web Worker hosting the modularized Dynamips WASM build
    ├── vm.js              # v86 emulator integration (MikroTik RouterOS & x86 VMs)
    ├── store.js           # IndexedDB for images, NVRAM, VM disks, and named lab snapshots
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
