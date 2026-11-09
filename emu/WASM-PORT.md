# Dynamips → WASM port status

Goal: run real Cisco IOS in browser (GNS3-in-WASM) wired into netlab's fabric. MikroTik RouterOS later via v86 (x86).

## Build

`make emu` (root Makefile). Docker `emscripten/emsdk:3.1.74`, mounts `emu/` → `/src`.
Output: `emu/dynamips/build-wasm/stable/dynamips.{js,wasm}` (~584 KB wasm).
Flags: nojit arch, `-pthread`, `PTHREAD_POOL_SIZE=16`, `PROXY_TO_PTHREAD`,
`ALLOW_MEMORY_GROWTH`, `INITIAL_MEMORY=256MB`, `MODULARIZE`+`EXPORT_NAME=createIOS`,
`INVOKE_RUN=0`, `ASSERTIONS=1`, `--pre-js /src/wasm/pre.js`.
Runtime methods exported: FS, HEAPU8, ccall, callMain.

## Working (verified in node)

- Module loads; exports callable: `_nl_drain`, `_nl_drain_buf`, `_nl_inbuf`,
  `_nl_frame_rx`, `_nl_con_in`, `callMain`, `FS`.
- MIPS64 CPU executes code (register dumps prove it).
- libelf **shim** parses real big-endian MIPS IOS ELFs (`emu/shim/libelf.{h,c}`).
- Console bridge works: VTTY output → shared-mem queue → `nl_drain()` (70 bytes seen).
- `wasm:` NIO type parses (`-s 0:0:wasm:0`), port adapters bind (`-p 0:C7200-IO-FE`).
- Without `-s`, VM gets as far as ELF load + CPU start.

## Blocking bug — FIXED

`-s slot:port:wasm:link` crashed at `invokeEntryPoint`: "null function or
function signature mismatch". Root cause class: drivers loosely-cast callbacks
to different arities — legal UB natively, fatal on wasm `call_indirect`.

Fixed 19 handlers total:
- 9 × `netio_rx_handler_t` rx handlers (4-param impls vs 5-param typedef)
- 10 × `ptask_callback` tx handlers (1-param impls vs 2-param typedef,
  e.g. `dev_dec21140_handle_txring` — the actual -s attach crash)

VM now survives 3s with `-s 0:0:wasm:0 -s 1:0:wasm:1`, `nl_drain()` = 70
(console bytes). Register dump at end = expected crash of the 52-byte fake
ELF image, not a port issue.

Remaining latent same-class suspects if new traps appear: `timer_create_entry`
callbacks, `dev`/`pci` descriptor fn ptrs, `vm->` driver vtables.

## Patches in dynamips/ (vs upstream v0.2.25)

- `common/dynamips_common.h` — wasm arch → little-endian
- `stable/CMakeLists.txt` — EMSCRIPTEN uses `../pregen/*.inc` (rom2c is host tool);
  `target_sources += ../wasm/nl_bridge.c`; rom2c cmd prefixed w/ cross emulator
- `stable/mips64_jit.c` — `__builtin___clear_cache` → `#ifndef __EMSCRIPTEN__`
- `common/dev_vtty.h/.c` — `VTTY_TYPE_WASM`; forced under EMSCRIPTEN;
  `put_char` → `nl_console_out`
- `common/net_io.h/.c` — `NETIO_TYPE_WASM` + "wasm" table entry +
  `netio_desc_create_wasm` (calls `nl_nio_wasm_*` from nl_bridge.c)
- `common/cisco_card.c` — `vm_slot_cmd_add_nio` case: `slot:port:wasm:link_id`
- rx handlers: 9 patched to 5-param typedef (am79c971, dec21140, i8254x, i8255x,
  ethsw, netio_bridge, mueslix, pos_oc3, ti1570)

## Files

- `emu/wasm/nl_bridge.c` — shared-mem queues. Msg: u8 type|u8 link|u16 len|data.
  type 0=eth frame, 1=console. `nl_frame_rx(link)` after JS fills `nl_inbuf()`.
- `emu/wasm/pre.js` — `__emscripten_thread_crashed` shim (emscripten 3.1.74 bug)
- `emu/shim/libelf.{h,c}` — minimal read-only ELF32 (both byte orders)
- `emu/pregen/*.inc` — microcode dumps from native rom2c+shim

## Browser integration — DONE (verified end-to-end)

- `web/ios_worker.js` — classic Worker hosting the module. **Must pass
  `mainScriptUrlOrBlob`** to createIOS or spawned pthreads try
  `new Worker(undefined)` (shows as `/undefined` GETs).
- `web/ios.js` — `IOSNode` (worker lifecycle, tx/console) + `iosArgs(nPAs)`
  building `-p`/`-s wasm:` args.
- `web/serve.py` — COOP/COEP headers (required for SharedArrayBuffer).
  `make serve` uses it (port 8471).
- netlab-core: `DeviceKind::Ext` + `eth::` codec (real ARP/IPv4/ICMP
  encode/decode, original bytes preserved through forwards). WASM API:
  `ext_inject`, `drain_ext`, kind `"ext"`.
- Verified in Chrome preview: worker boots, dynamips console output reaches
  the page ("ROMMON emulation microcode… Launching IOS image"), fake
  wire-ARP injected → fabric → PC1 replied → real wire ARP reply captured.
- UI: "+ IOS" toolbar tool (orange router glyph), config bar with
  "Load IOS .bin…" file picker + zero-dep console pane (ANSI-stripped).

## Remaining

1. Real IOS image end-to-end — user-supplied .bin (can't ship, like GNS3).
   Everything else on the Cisco path is proven.
2. MikroTik: v86 x86 node; RouterOS CHR = legal free download (mikrotik.com).
   Same bridge design: NIO→ext device ports.
3. Polish: ext port↔NIO mapping is positional (port index == link_id);
   idle-pc table; ANSI console → real xterm.js if needed.

## Node smoke test (worked)

```js
const createIOS = require('./dynamips.js');
createIOS({}).then(M => {
  const e = new Uint8Array(52);          // minimal ELF32 MSB MIPS header
  e[0]=0x7f; e[1]=0x45; e[2]=0x4c; e[3]=0x46; e[4]=1; e[5]=2;
  e[18]=0; e[19]=8; e[24]=0x80;
  M.FS.writeFile('/ios.bin', e);
  M.callMain(['-P','7200','-r','64','-p','0:C7200-IO-FE','/ios.bin']);
  setTimeout(()=>{ console.log(M._nl_drain()); }, 2500);   // -> 70
});
```

## NVRAM persistence (IOS `write memory`)

Dynamips mmaps NVRAM from a MEMFS file (`c7200_i0_nvram`) and only msyncs it
on clean shutdown — which never happens when the page terminates the worker.
So the bridge exposes the live device memory instead:
`nl_nvram_ptr()` / `nl_nvram_len()` (looks up the `nvram` device of VM
"default"). `ios_worker.js` snapshots it every 3 s (FNV hash, only posts on
change, skips all-zero) and on `{type:'sync'}` before stop; the page stores it
in IndexedDB keyed by the node's stable `uid`. Restore = write the file before
`callMain` (`memzone_create_file` keeps existing contents). Verified: a marker
written to the file is visible in live device memory after boot.

## Reproducible build

Upstream is not vendored. `emu/fetch-dynamips.sh` clones GNS3/dynamips at the
pinned revision and applies `emu/dynamips.patch`; `make emu` also rebuilds
`emu/shim/libelf.a` (emcc `-pthread` — the final link uses shared memory).
Regenerate the patch after editing upstream files:
`git -C emu/dynamips diff > emu/dynamips.patch`.
