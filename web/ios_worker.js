// Web Worker hosting the Dynamips→WASM module (PROXY_TO_PTHREAD — main runs
// in a pthread, so exports stay callable from this worker's JS context).
//
// in : {type:'boot', image:ArrayBuffer, args:[...], ramMB}
//    : {type:'frame', link, data:Uint8Array}   // fabric -> NIO
//    : {type:'conin', data:Uint8Array}         // key input -> console
// out: {type:'ready'} / {type:'frame'...} / {type:'console', data}
//      {type:'log', msg} / {type:'exit', code}
//      {type:'nvram', data, final}            // NVRAM snapshot when it changed
// in : {type:'sync'}                          // snapshot NVRAM now (before stop)

'use strict';

importScripts('dynamips.js');

let M = null;

const post = (m, xfers) => postMessage(m, xfers || []);

// Emulator stdout can be a firehose (crash loops dump registers forever), so
// batch it: one message per 100 ms, at most LOG_CAP lines, with a drop count.
const LOG_CAP = 200;
let logBuf = [], logDropped = 0;
function logLine(t) {
  if (logBuf.length < LOG_CAP) logBuf.push(t);
  else logDropped++;
}
setInterval(() => {
  if (!logBuf.length && !logDropped) return;
  if (logDropped) logBuf.push(`… ${logDropped} more log lines dropped`);
  post({ type: 'log', msg: logBuf.join('\n') });
  logBuf = []; logDropped = 0;
}, 100);

// ---- NVRAM persistence -------------------------------------------------
// IOS `write memory` lands in the nvram device memory. Snapshot it when it
// changes so the page can keep it in IndexedDB.
let lastNvHash = 0;
function fnv(a) {
  let h = 0x811c9dc5;
  for (let i = 0; i < a.length; i++) { h ^= a[i]; h = Math.imul(h, 0x01000193); }
  return h >>> 0;
}
function snapNvram(final) {
  if (!M || !M._nl_nvram_ptr) return;
  const p = M._nl_nvram_ptr(), n = M._nl_nvram_len();
  if (!p || !n) { if (final) post({ type: 'nvram', data: null, final }); return; }
  const data = new Uint8Array(M.HEAPU8.buffer, p, n).slice();
  // a blank NVRAM (fresh router, nothing written yet) is not worth storing
  if (!final && !lastNvHash && !data.some(b => b)) return;
  const h = fnv(data);
  if (h === lastNvHash && !final) return;
  const changed = h !== lastNvHash;
  lastNvHash = h;
  post({ type: 'nvram', data: changed ? data : null, final }, changed ? [data.buffer] : []);
}

function drain() {
  if (!M || !M._nl_drain) return;
  const n = M._nl_drain();
  if (n <= 0) return;
  const buf = M._nl_drain_buf();
  const view = new Uint8Array(M.HEAPU8.buffer, buf, n);
  let pos = 0;
  while (pos + 4 <= n) {
    const type = view[pos];
    const link = view[pos + 1];
    const len = view[pos + 2] | (view[pos + 3] << 8);
    const data = view.slice(pos + 4, pos + 4 + len);
    if (type === 0) post({ type: 'frame', link, data }, [data.buffer]);
    else if (type === 1) post({ type: 'console', data }, [data.buffer]);
    pos += 4 + len;
  }
}

onmessage = async (e) => {
  const m = e.data;
  try {
    if (m.type === 'boot') {
      if (!M) {
        post({ type: 'log', msg: 'loading dynamips.wasm…' });
        M = await createIOS({
          print: logLine,
          printErr: logLine,
          locateFile: (p) => p,
          // pthread workers need to re-importScripts this file
          mainScriptUrlOrBlob: new URL('dynamips.js', self.location.href).href,
        });
        post({ type: 'log', msg: 'module ready' });
      }
      M.FS.writeFile('/ios.bin', new Uint8Array(m.image));
      if (m.nvram) {
        // Dynamips names the file per its naming mode; seed both c7200 nvram and c3725/c3745/c2691 rom
        const nv = new Uint8Array(m.nvram);
        M.FS.writeFile('/c7200_i0_nvram', nv);
        M.FS.writeFile('/c7200_default_nvram', nv);
        if (nv.length === 0x1c000) {
          const rom = new Uint8Array(2 * 1048576);
          rom.set(nv, 0xe0000);
          for (const p of ['c3725', 'c3745', 'c2691', 'c3600']) {
            M.FS.writeFile(`/${p}_i0_rom`, rom);
            M.FS.writeFile(`/${p}_default_rom`, rom);
          }
        }
        lastNvHash = fnv(nv);
        post({ type: 'log', msg: `restored NVRAM (${nv.length} bytes) from browser storage` });
      }
      post({ type: 'ready' });
      // callMain runs inside a pthread (PROXY_TO_PTHREAD) — returns immediately
      setTimeout(() => {
        try {
          M.callMain(m.args);
        } catch (ex) {
          post({ type: 'log', msg: 'callMain: ' + ex });
        }
      }, 0);
      setInterval(drain, 2);
      setInterval(() => snapNvram(false), 3000);
    } else if (m.type === 'frame' && M) {
      const p = M._nl_inbuf();
      M.HEAPU8.set(m.data.subarray(0, 2048), p);
      M._nl_frame_rx(m.link, Math.min(m.data.length, 2048));
    } else if (m.type === 'sync') {
      snapNvram(true);
    } else if (m.type === 'conin' && M) {
      const p = M._nl_inbuf();
      M.HEAPU8.set(m.data.subarray(0, 2048), p);
      M._nl_con_in(Math.min(m.data.length, 2048));
    }
  } catch (ex) {
    post({ type: 'log', msg: 'worker err: ' + (ex && ex.message || ex) });
  }
};
