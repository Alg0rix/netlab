// x86 virtual machines (MikroTik RouterOS v6, any 32-bit x86 image) on v86.
//
// Same surface as IOSNode so the page treats every emulator alike:
//   state, onState, onLog, onFrame(dev, port, bytes), tx(port, bytes), stop(), kill()
// plus: serial console bytes (onSerial / sendSerial), a VGA screen element with
// its own keyboard handling, and the guest disk (onDisk) for persistence.
//
// v86 emulates one NIC per VM, so a VM has exactly one interface (port 0).

let V86 = null;
async function loadV86() {
  if (!V86) V86 = (await import('./v86/libv86.mjs')).V86;
  return V86;
}

export const VM_DEFAULTS = {
  routeros: { ram: 256, diskMB: 128, nic: 'ne2k' },
  x86: { ram: 128, diskMB: 256, nic: 'ne2k' },
};

const DISK_SAVE_IDLE_MS = 8000;   // save the disk this long after the guest stops writing
const DISK_SAVE_MAX_MS = 45000;   // ...but never leave it dirty longer than this
const SERIAL_PACE_MS = 12;        // the emulated UART drops bytes sent in a burst

// US-layout set-1 scancodes for the keys a console needs
const SC = {
  Enter: [0x1c], Backspace: [0x0e], Tab: [0x0f], Escape: [0x01], ' ': [0x39],
  ArrowUp: [0xe0, 0x48], ArrowDown: [0xe0, 0x50], ArrowLeft: [0xe0, 0x4b], ArrowRight: [0xe0, 0x4d],
  Home: [0xe0, 0x47], End: [0xe0, 0x4f], PageUp: [0xe0, 0x49], PageDown: [0xe0, 0x51],
  Insert: [0xe0, 0x52], Delete: [0xe0, 0x53],
  F1: [0x3b], F2: [0x3c], F3: [0x3d], F4: [0x3e], F5: [0x3f], F6: [0x40], F7: [0x41], F8: [0x42], F9: [0x43], F10: [0x44],
};
const LETTER = 'qwertyuiop'.split('').map((c, i) => [c, 0x10 + i])
  .concat('asdfghjkl'.split('').map((c, i) => [c, 0x1e + i]))
  .concat('zxcvbnm'.split('').map((c, i) => [c, 0x2c + i]));
const LETTER_SC = Object.fromEntries(LETTER);
const press = codes => codes.concat(codes.length === 2 ? [codes[0], codes[1] | 0x80] : [codes[0] | 0x80]);

export class VMNode {
  constructor(devId, name) {
    this.devId = devId;
    this.name = name;
    this.vm = null;
    this.state = 'stopped';
    this.onState = null;
    this.onLog = null;
    this.onFrame = null;
    this.onSerial = null;     // (Uint8Array) => void
    this.onDisk = null;       // (ArrayBuffer) => Promise — persist guest disk
    this.disk = null;
    this.serialQ = [];
    this.serialTimer = null;
    this.saveTimer = null;
    this.dirtySince = 0;
    this.saving = null;
    this.screen = makeScreen();
  }

  log(m) { this.onLog && this.onLog(m); }
  setState(s) { this.state = s; this.onState && this.onState(s); }

  /**
   * media: ArrayBuffer of an ISO (attached as CD-ROM) or null
   * disk:  ArrayBuffer used as the first hard disk (modified in place by the guest)
   */
  async boot({ media = null, disk, ram, nic }) {
    this.kill();
    this.setState('booting');
    let V;
    try { V = await loadV86(); } catch (e) { this.log(`could not load the x86 emulator: ${e.message}`); this.setState('stopped'); return; }
    this.disk = disk;
    const cfg = {
      wasm_path: new URL('./v86/v86.wasm', import.meta.url).href,
      bios: { url: new URL('./v86/seabios.bin', import.meta.url).href },
      vga_bios: { url: new URL('./v86/vgabios.bin', import.meta.url).href },
      memory_size: ram * 1024 * 1024,
      vga_memory_size: 8 * 1024 * 1024,
      hda: { buffer: disk },
      boot_order: 0x132,          // hard disk first, then CD — an installed system boots itself
      net_device: { type: nic },
      screen_container: this.screen.el,
      disable_mouse: true,
      autostart: true,
    };
    if (media) cfg.cdrom = { buffer: media };
    const vm = new V(cfg);
    this.vm = vm;
    vm.add_listener('emulator-ready', () => {
      vm.keyboard_set_status(false);   // never listen to the whole page; keys come from our screen element
      this.setState('running');
      this.log(`VM started · ${ram} MB RAM · ${nic} NIC${media ? ' · install CD attached' : ''}`);
    });
    vm.add_listener('net0-send', bytes => this.onFrame && this.onFrame(this.devId, 0, bytes));
    let serBuf = [];
    vm.add_listener('serial0-output-byte', b => {
      serBuf.push(b);
      if (serBuf.length === 1) queueMicrotask(() => { const out = Uint8Array.from(serBuf); serBuf = []; this.onSerial && this.onSerial(out); });
    });
    vm.add_listener('ide-write-end', () => {
      const now = Date.now();
      if (!this.dirtySince) this.dirtySince = now;
      clearTimeout(this.saveTimer);
      // a guest that keeps writing would push an idle-only save back forever
      const wait = Math.max(0, Math.min(DISK_SAVE_IDLE_MS, this.dirtySince + DISK_SAVE_MAX_MS - now));
      this.saveTimer = setTimeout(() => this.saveDisk(), wait);
    });
  }

  async saveDisk() {
    clearTimeout(this.saveTimer);
    if (this.saving) return this.saving;          // one snapshot at a time
    if (!this.disk || !this.onDisk) return;
    this.dirtySince = 0;
    this.saving = this.onDisk(this.disk).finally(() => { this.saving = null; });
    return this.saving;
  }

  /** fabric → guest NIC */
  tx(port, bytes) {
    if (port === 0 && this.vm && this.state === 'running') this.vm.bus.send('net0-receive', bytes);
  }

  /** console → guest serial, paced so the UART doesn't drop characters */
  sendSerial(bytes) {
    for (const b of bytes) this.serialQ.push(b);
    if (this.serialTimer) return;
    this.serialTimer = setInterval(() => {
      const b = this.serialQ.shift();
      if (b === undefined || !this.vm) { clearInterval(this.serialTimer); this.serialTimer = null; return; }
      this.vm.bus.send('serial0-input', b);
    }, SERIAL_PACE_MS);
  }
  sendKeys(bytes) { this.sendSerial(bytes); }

  /** key on the focused VGA screen → PS/2 scancodes. Returns true if handled. */
  screenKey(e) {
    if (!this.vm) return false;
    const vm = this.vm;
    if (e.ctrlKey && !e.altKey && /^[a-z]$/i.test(e.key)) {
      vm.keyboard_send_scancodes([0x1d, ...press([LETTER_SC[e.key.toLowerCase()]]), 0x9d]);
      return true;
    }
    if (SC[e.key]) { vm.keyboard_send_scancodes(press(SC[e.key])); return true; }
    if (e.key.length === 1 && !e.metaKey) {
      // v86 only translates characters while its (page-wide) adapter is enabled; enable it
      // for this synchronous send only — our caller already stopped the event's propagation
      vm.keyboard_set_status(true);
      try { vm.keyboard_send_text(e.key); } finally { vm.keyboard_set_status(false); }
      return true;
    }
    return false;
  }

  /** Graceful stop: halt the CPU, then persist the disk. */
  async stop() {
    if (!this.vm) return;
    try { await this.vm.stop(); } catch { /* already stopped */ }
    await this.saveDisk();
    this.kill();
  }

  kill() {
    clearTimeout(this.saveTimer);
    clearInterval(this.serialTimer);
    this.serialTimer = null;
    this.serialQ = [];
    if (this.vm) {
      const vm = this.vm;
      this.vm = null;
      vm.destroy?.().catch?.(() => {});
      this.screen.clear();
      this.setState('stopped');
    }
  }

  terminate() { this.kill(); }
}

function makeScreen() {
  const el = document.createElement('div');
  el.className = 'vga';
  el.tabIndex = 0;
  el.innerHTML = '<div class="vga-text"></div><canvas class="vga-gfx" style="display:none"></canvas>';
  return {
    el,
    clear() { el.querySelector('.vga-text').textContent = ''; const c = el.querySelector('canvas'); c.style.display = 'none'; },
  };
}

/** Sniff an uploaded image. kind: 'ios' | 'iso' | 'disk'; error/warn are user-facing. */
export async function sniffImage(file) {
  const head = new Uint8Array(await file.slice(0, 0x8006).arrayBuffer());
  const at = (o, s) => [...s].every((c, i) => head[o + i] === c.charCodeAt(0));
  const name = file.name.toLowerCase();
  if (head[0] === 0x7f && at(1, 'ELF')) return { kind: 'ios' };
  if (at(0, 'QFI\xfb')) return { error: 'This is a qcow2 image. Convert it to raw first:  qemu-img convert -O raw in.qcow2 out.img' };
  if (at(0, 'KDMV') || name.endsWith('.vmdk')) return { error: 'This is a VMDK image. Convert it to raw first:  qemu-img convert -O raw in.vmdk out.img' };
  if (at(0x40, '<<< ') || at(0, '<<< ') || name.endsWith('.vdi')) return { error: 'This is a VirtualBox VDI image. Convert it to raw first:  qemu-img convert -O raw in.vdi out.img' };
  if (at(0, 'vhdxfile')) return { error: 'This is a VHDX image. Convert it to raw first:  qemu-img convert -O raw in.vhdx out.img' };
  const isIso = at(0x8001, 'CD001');
  let warn = null;
  if (/chr/.test(name)) warn = 'MikroTik CHR is a 64-bit (x86-64) system; this browser emulator runs 32-bit x86 only, so it will not boot. Use the RouterOS v6 x86 ISO instead.';
  else if (/mikrotik-7|routeros-7|-7\.\d/.test(name)) warn = 'RouterOS v7 for x86 is 64-bit only and will not boot here. Use RouterOS v6 (e.g. mikrotik-6.49.x.iso).';
  return { kind: isIso ? 'iso' : 'disk', warn };
}
