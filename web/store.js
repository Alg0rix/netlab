// Persistence: router images in IndexedDB (they're 20–150 MB), the lab itself
// in localStorage (autosave) and as downloadable .netlab.json files.

const DB = 'netlab', STORE = 'images', NVRAM = 'nvram';
let dbp = null;
function db() {
  if (!dbp) dbp = new Promise((res, rej) => {
    try {
      const r = indexedDB.open(DB, 2);
      r.onupgradeneeded = () => {
        const d = r.result;
        if (!d.objectStoreNames.contains(STORE)) d.createObjectStore(STORE, { keyPath: 'name' });
        if (!d.objectStoreNames.contains(NVRAM)) d.createObjectStore(NVRAM, { keyPath: 'uid' });
      };
      r.onsuccess = () => res(r.result);
      r.onerror = () => rej(r.error);
    } catch (e) { rej(e); }
  });
  return dbp;
}
const memImages = new Map(); // fallback when IndexedDB is unavailable

async function tx(mode, fn, store = STORE) {
  const d = await db();
  return new Promise((res, rej) => {
    const t = d.transaction(store, mode);
    const r = fn(t.objectStore(store));
    t.oncomplete = () => res(r?.result);
    t.onerror = () => rej(t.error);
  });
}

export const images = {
  async list() {
    try {
      const all = await tx('readonly', s => s.getAll());
      return all.map(({ name, size, added, platform }) => ({ name, size, added, platform }));
    } catch {
      return [...memImages.values()].map(({ name, size, added, platform }) => ({ name, size, added, platform }));
    }
  },
  async put(file) {
    const data = await file.arrayBuffer();
    const rec = { name: file.name, size: file.size, added: Date.now(), platform: guessPlatform(file.name), data };
    try { await tx('readwrite', s => s.put(rec)); } catch { memImages.set(rec.name, rec); }
    return rec;
  },
  async get(name) {
    try {
      const r = await tx('readonly', s => s.get(name));
      if (r) return r;
    } catch { /* fall through */ }
    return memImages.get(name) ?? null;
  },
  async remove(name) {
    try { await tx('readwrite', s => s.delete(name)); } catch { /* ignore */ }
    memImages.delete(name);
  },
};

/** Per-node IOS NVRAM (startup-config), keyed by the node's stable uid. */
const memNvram = new Map();
export const nvram = {
  async get(uid) {
    try { const r = await tx('readonly', s => s.get(uid), NVRAM); if (r) return r; } catch { /* fall through */ }
    return memNvram.get(uid) ?? null;
  },
  async put(uid, data) {
    const rec = { uid, data: data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength), saved: Date.now() };
    try { await tx('readwrite', s => s.put(rec), NVRAM); } catch { memNvram.set(uid, rec); }
    return rec;
  },
  async remove(uid) {
    try { await tx('readwrite', s => s.delete(uid), NVRAM); } catch { /* ignore */ }
    memNvram.delete(uid);
  },
};

export const newUid = () => (crypto.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`);

function guessPlatform(n) {
  const m = n.toLowerCase().match(/^c(\d{4})/);
  return m ? `c${m[1]}` : 'c7200';
}

const KEY = 'netlab.autosave.v1';
export const autosave = {
  load() { try { return JSON.parse(localStorage.getItem(KEY) ?? 'null'); } catch { return null; } },
  save(lab) { try { localStorage.setItem(KEY, JSON.stringify(lab)); } catch { /* quota / private mode */ } },
  clear() { try { localStorage.removeItem(KEY); } catch { /* ignore */ } },
};

export const prefs = {
  get(k, d) { try { const v = localStorage.getItem('netlab.pref.' + k); return v == null ? d : JSON.parse(v); } catch { return d; } },
  set(k, v) { try { localStorage.setItem('netlab.pref.' + k, JSON.stringify(v)); } catch { /* ignore */ } },
};

export function download(blob, name) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 4000);
}

export function pickFile(accept) {
  return new Promise(res => {
    const i = document.createElement('input');
    i.type = 'file';
    i.accept = accept;
    i.onchange = () => res(i.files[0] ?? null);
    i.click();
  });
}
