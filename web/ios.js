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

/** Default dynamips args for a c7200 router.
 *  nPAs fast-ethernet PAs mapped to NIO link ids 0..nPAs-1 (slot0=IO-FE). */
export function iosArgs(nPAs = 3, ram = 256) {
  const args = ['-P', '7200', '-r', String(ram)];
  for (let i = 0; i < nPAs; i++) {
    args.push('-p', `${i}:${i === 0 ? 'C7200-IO-FE' : 'PA-FE-TX'}`);
    args.push('-s', `${i}:0:wasm:${i}`);
  }
  args.push('/ios.bin');
  return args;
}
