// NetlabIOS — manages Dynamips worker lifecycle and the frame bridge between
// ext devices (netlab fabric) and wasm NIOs inside the worker.
//
// ext port index == dynamips NIO link_id (declared via -s slot:port:wasm:id).
// The worker is created lazily on boot() and torn down on stop(), so idle
// IOS nodes cost nothing.

export class IOSNode {
  constructor(devId, name) {
    this.devId = devId;
    this.name = name;
    this.worker = null;
    this.state = 'stopped'; // stopped | booting | running
    this.term = null;       // callback(bytes) set by the console pane
    this.onLog = null;
    this.onFrame = null;
    this.onState = null;
    this.onNvram = null;    // (Uint8Array) => void — persist IOS config
    this._syncWait = null;
  }

  log(msg) { this.onLog && this.onLog(msg); }

  setState(s) {
    this.state = s;
    this.onState && this.onState(s);
  }

  onMsg(m) {
    switch (m.type) {
      case 'log': this.log(m.msg); break;
      case 'ready':
        this.setState('running');
        this.log('Dynamips up — IOS is booting');
        break;
      case 'frame':
        this.onFrame && this.onFrame(this.devId, m.link, m.data);
        break;
      case 'console':
        this.term && this.term(m.data);
        break;
      case 'nvram':
        if (m.data) this.onNvram && this.onNvram(m.data);
        if (m.final && this._syncWait) { this._syncWait(); this._syncWait = null; }
        break;
    }
  }

  /** Boot with a user-supplied IOS image + arg list (+ saved NVRAM). */
  boot(image, args, nvram = null) {
    this.kill();
    this.worker = new Worker('./ios_worker.js', { type: 'classic' });
    this.worker.onmessage = (e) => this.onMsg(e.data);
    this.worker.onerror = (e) => {
      this.log(`worker error: ${e.message || 'unknown'}`);
      this.kill();
    };
    this.setState('booting');
    const xfer = nvram ? [image, nvram] : [image];
    this.worker.postMessage({ type: 'boot', image, args, nvram }, xfer);
  }

  /** Graceful stop: grab a final NVRAM snapshot, then tear the worker down. */
  async stop() {
    if (!this.worker) return;
    await new Promise(res => {
      this._syncWait = res;
      this.worker.postMessage({ type: 'sync' });
      setTimeout(res, 1500);
    });
    this._syncWait = null;
    this.kill();
  }

  kill() {
    if (this.worker) {
      this.worker.terminate();
      this.worker = null;
      this.setState('stopped');
    }
  }

  /** fabric -> NIO: send raw Ethernet bytes to NIO link `port`. */
  tx(port, bytes) {
    this.worker?.postMessage({ type: 'frame', link: port, data: bytes });
  }

  /** key input -> console vtty. */
  sendKeys(bytes) {
    this.worker?.postMessage({ type: 'conin', data: bytes });
  }

  terminate() { this.kill(); }
}

/** Detect Dynamips platform from the ELF32 e_machine header (big-endian u16 at offset 18). */
export function detectIosPlatform(buf) {
  if (!buf || buf.byteLength < 20) return '7200';
  const u8 = buf instanceof Uint8Array ? buf : new Uint8Array(buf, 0, 20);
  if (u8[0] !== 0x7f || u8[1] !== 0x45 || u8[2] !== 0x4c || u8[3] !== 0x46) return '7200';
  const mach = (u8[18] << 8) | u8[19];
  switch (mach) {
    case 0x61: return '3725';
    case 0x69: return '3745';
    case 0x66: return '2691';
    case 0x1e:
    case 0x34: return '3600';
    case 0x19:
    default:   return '7200';
  }
}

/** Build Dynamips CLI arguments for a Cisco IOS router or managed L2/L3 switch.
 *  - On c3725/c3745/c2691/c3600 images:
 *      * Switch mode (`isSwitch: true`) installs `NM-16ESW` in slot 1 (16× FastEthernet hardware switchports `Fa1/0`–`Fa1/15`).
 *      * Router mode uses integrated dual FastEthernet (`Fa0/0`, `Fa0/1`) + `NM-16ESW` for ports 2..nPorts-1.
 *  - On c7200 images:
 *      * Switch mode (`isSwitch: true`) installs 8-port Ethernet modules `PA-8E` in slots 1..2 (`Et1/0`–`Et1/7`, `Et2/0`–`Et2/7`) for IEEE 802.1D / IRB bridging.
 *      * Router mode uses `C7200-IO-FE` (or `C7200-IO-2FE` when 8 ports) + `PA-FE-TX` in slots 1..6. */
export function iosArgs(nPorts = 4, opts = {}) {
  const o = typeof opts === 'number' ? { ram: opts } : (opts || {});
  const platform = o.platform || '7200';
  const ram = o.ram || 256;
  const isSwitch = !!o.isSwitch;
  const n = Math.max(1, Math.min(isSwitch ? 16 : 8, nPorts));

  if (platform !== '7200') {
    const args = ['-P', platform, '-r', String(ram)];
    if (isSwitch) {
      args.push('-p', '1:NM-16ESW');
      for (let i = 0; i < n; i++) args.push('-s', `1:${i}:wasm:${i}`);
    } else {
      for (let i = 0; i < Math.min(2, n); i++) args.push('-s', `0:${i}:wasm:${i}`);
      if (n > 2) {
        args.push('-p', '1:NM-16ESW');
        for (let i = 2; i < n; i++) args.push('-s', `1:${i - 2}:wasm:${i}`);
      }
    }
    args.push('/ios.bin');
    return args;
  }

  const args = ['-P', '7200', '-r', String(ram)];
  if (isSwitch) {
    args.push('-p', '0:C7200-IO-FE');
    args.push('-p', '1:PA-8E');
    for (let i = 0; i < Math.min(8, n); i++) args.push('-s', `1:${i}:wasm:${i}`);
    if (n > 8) {
      args.push('-p', '2:PA-8E');
      for (let i = 8; i < n; i++) args.push('-s', `2:${i - 8}:wasm:${i}`);
    }
  } else if (n <= 7) {
    for (let i = 0; i < n; i++) {
      args.push('-p', `${i}:${i === 0 ? 'C7200-IO-FE' : 'PA-FE-TX'}`);
      args.push('-s', `${i}:0:wasm:${i}`);
    }
  } else {
    args.push('-p', '0:C7200-IO-2FE', '-s', '0:0:wasm:0', '-s', '0:1:wasm:1');
    for (let i = 2; i < n; i++) {
      args.push('-p', `${i - 1}:PA-FE-TX`, '-s', `${i - 1}:0:wasm:${i}`);
    }
  }
  args.push('/ios.bin');
  return args;
}
