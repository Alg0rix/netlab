# Vendored third-party files

| File | Source | Licence |
|---|---|---|
| `libv86.mjs`, `v86.wasm` | npm `v86@0.5.472` (`build/`) | BSD-2-Clause — see `LICENSE` |
| `seabios.bin` | github.com/copy/v86 `bios/seabios.bin` @ `b8a39b11dd2076870699e6cac053556271b9bfab` (SeaBIOS rel-1.16.2) | LGPL-3.0 (SeaBIOS) |
| `vgabios.bin` | github.com/copy/v86 `bios/vgabios.bin` @ `b8a39b11dd2076870699e6cac053556271b9bfab` | LGPL-3.0 (SeaVGABIOS) |

SeaBIOS source: https://www.seabios.org/ (git://git.seabios.org/seabios.git, tag rel-1.16.2).
The BIOS images are unmodified binaries loaded by the emulator at runtime.

`../vendor/xterm/`: npm `@xterm/xterm@5.5.0` and `@xterm/addon-fit@0.10.0`, MIT — see `../vendor/xterm/LICENSE`.
