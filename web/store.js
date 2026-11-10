// Persistence: router images in IndexedDB (they're 20–150 MB), the lab itself
// in localStorage (autosave) and as downloadable .netlab.json files.

const DB = 'netlab', STORE = 'images', NVRAM = 'nvram', DISKS = 'disks', SNAPS = 'snaps';
let dbp = null;
function db() {
  if (!dbp) dbp = new Promise((res, rej) => {
    try {
      const r = indexedDB.open(DB, 4);
      r.onupgradeneeded = () => {
        const d = r.result;
        if (!d.objectStoreNames.contains(STORE)) d.createObjectStore(STORE, { keyPath: 'name' });
        if (!d.objectStoreNames.contains(NVRAM)) d.createObjectStore(NVRAM, { keyPath: 'uid' });
        if (!d.objectStoreNames.contains(DISKS)) d.createObjectStore(DISKS, { keyPath: 'uid' });
        if (!d.objectStoreNames.contains(SNAPS)) d.createObjectStore(SNAPS, { keyPath: 'id' });
      };
      r.onsuccess = () => {
        r.result.onversionchange = () => r.result.close();   // never block a newer tab's upgrade
        res(r.result);
      };
      r.onerror = () => rej(r.error);
      // an old netlab tab holds the v1 database open: fall back to memory, tell the user
      r.onblocked = () => {
        dispatchEvent(new CustomEvent('netlab-storage-blocked'));
        rej(new Error('IndexedDB upgrade blocked by another netlab tab'));
      };
    } catch (e) { rej(e); }
  });
  return dbp;
}
const memImages = new Map(); // fallback when IndexedDB is unavailable

// Large payloads go in as Blobs: Chrome caps a single structured-clone value at
// ~127 MB, but Blobs are stored out of line. Older records hold ArrayBuffers.
async function materialize(rec) {
  if (!rec) return rec;
  if (rec.blob) return { ...rec, data: await rec.blob.arrayBuffer() };
  return rec;
}

async function tx(mode, fn, store = STORE) {
  const d = await db();
  return new Promise((res, rej) => {
    const t = d.transaction(store, mode);
    const r = fn(t.objectStore(store));
    t.oncomplete = () => res(r?.result);
    t.onerror = () => rej(t.error);
    t.onabort = () => rej(t.error ?? new Error('transaction aborted'));
  });
}

export const images = {
  async list() {
    try {
      const all = await tx('readonly', s => s.getAll());
      return all.map(({ name, size, added, platform, kind }) => ({ name, size, added, platform, kind: kind ?? 'ios' }));
    } catch {
      return [...memImages.values()].map(({ name, size, added, platform, kind }) => ({ name, size, added, platform, kind: kind ?? 'ios' }));
    }
  },
  async put(file, kind = 'ios') {
    const rec = { name: file.name, size: file.size, added: Date.now(), platform: guessPlatform(file.name), kind, blob: file };
    try { await tx('readwrite', s => s.put(rec)); }
    catch (e) { memImages.set(rec.name, rec); if (e?.name === 'QuotaExceededError') throw e; }
    return rec;
  },
  /** returns the record with `data` as a fresh ArrayBuffer */
  async get(name) {
    try {
      const r = await tx('readonly', s => s.get(name));
      if (r) return materialize(r);
    } catch { /* fall through */ }
    return materialize(memImages.get(name) ?? null);
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
  async list() {
    try { return (await tx('readonly', s => s.getAll(), NVRAM)).map(({ uid, saved, data }) => ({ uid, saved, size: data?.byteLength ?? 0 })); }
    catch { return [...memNvram.values()].map(({ uid, saved, data }) => ({ uid, saved, size: data?.byteLength ?? 0 })); }
  },
};

/** Per-node VM hard disks (whole raw image), keyed by uid. Large: 64 MB – 1 GB. */
const memDisks = new Map();
export const disks = {
  async get(uid) {
    try { const r = await tx('readonly', s => s.get(uid), DISKS); if (r) return materialize(r); } catch { /* fall through */ }
    return materialize(memDisks.get(uid) ?? null);
  },
  /** [{ uid, size, saved }] without reading the data */
  async list() {
    try {
      return await new Promise(async (res, fail) => {
        const d = await db();
        const out = [];
        const req = d.transaction(DISKS).objectStore(DISKS).openCursor();
        req.onsuccess = () => { const c = req.result; if (!c) return res(out); out.push({ uid: c.value.uid, size: c.value.size, saved: c.value.saved }); c.continue(); };
        req.onerror = () => fail(req.error);
      });
    } catch { return [...memDisks.values()].map(({ uid, size, saved }) => ({ uid, size, saved })); }
  },
  /** cheap existence/metadata check without reading the whole disk */
  async info(uid) {
    try { const r = await tx('readonly', s => s.get(uid), DISKS); if (r) return { size: r.size, saved: r.saved }; } catch { /* fall through */ }
    const m = memDisks.get(uid);
    return m ? { size: m.size, saved: m.saved } : null;
  },
  async put(uid, data) {
    // snapshot now (Blob copies synchronously) so the running guest can keep writing
    const rec = { uid, blob: new Blob([data]), size: data.byteLength, saved: Date.now() };
    try { await tx('readwrite', s => s.put(rec), DISKS); }
    catch (e) { memDisks.set(uid, rec); throw e; }   // kept for this session; caller tells the user
    return rec;
  },
  async remove(uid) {
    try { await tx('readwrite', s => s.delete(uid), DISKS); } catch { /* ignore */ }
    memDisks.delete(uid);
  },
};

/** Named lab snapshots: topology JSON plus a copy of each node's NVRAM and VM disk. */
const memSnaps = new Map();
export const snapshots = {
  async list() {
    try {
      const d = await db();
      return await new Promise((res, rej) => {
        const out = [];
        const req = d.transaction(SNAPS).objectStore(SNAPS).openCursor();
        req.onsuccess = () => {
          const c = req.result;
          if (!c) return res(out.sort((a, b) => b.at - a.at));
          if (!String(c.key).startsWith('disk:')) out.push({ id: c.value.id, name: c.value.name, at: c.value.at });
          c.continue();
        };
        req.onerror = () => rej(req.error);
      });
    } catch {
      return [...memSnaps.values()].filter(r => !String(r.id).startsWith('disk:')).map(({ id, name, at }) => ({ id, name, at }));
    }
  },
  async save(name, lab) {
    const id = newUid();
    const nv = [];
    for (const n of lab.nodes ?? []) {
      if (!n.uid) continue;
      const v = await nvram.get(n.uid);
      if (v?.data) nv.push({ uid: n.uid, data: v.data });
      const disk = await disks.get(n.uid);
      if (disk?.data) {
        const rec = { id: `disk:${id}:${n.uid}`, blob: new Blob([disk.data]), size: disk.data.byteLength };
        try { await tx('readwrite', s => s.put(rec), SNAPS); } catch { memSnaps.set(rec.id, rec); }
      }
    }
    const rec = { id, name: name || 'snapshot', at: Date.now(), lab, nvram: nv };
    try { await tx('readwrite', s => s.put(rec), SNAPS); } catch { memSnaps.set(id, rec); }
    return id;
  },
  async load(id) {
    let rec = memSnaps.get(id) ?? null;
    try { rec = (await tx('readonly', s => s.get(id), SNAPS)) ?? rec; } catch { /* memory */ }
    if (!rec?.lab) return null;
    for (const v of rec.nvram ?? []) {
      if (v?.uid && v.data) await nvram.put(v.uid, new Uint8Array(v.data));
    }
    for (const n of rec.lab.nodes ?? []) {
      if (!n.uid) continue;
      const key = `disk:${id}:${n.uid}`;
      let disk = memSnaps.get(key) ?? null;
      try { disk = (await tx('readonly', s => s.get(key), SNAPS)) ?? disk; } catch { /* memory */ }
      if (disk?.blob) await disks.put(n.uid, await disk.blob.arrayBuffer()).catch(() => {});
      else if (disk?.data) await disks.put(n.uid, disk.data).catch(() => {});
    }
    return rec.lab;
  },
  async remove(id) {
    const drop = async (key) => {
      try { await tx('readwrite', s => s.delete(key), SNAPS); } catch { /* ignore */ }
      memSnaps.delete(key);
    };
    await drop(id);
    let keys = [];
    try {
      const d = await db();
      keys = await new Promise((res, rej) => {
        const out = [];
        const req = d.transaction(SNAPS).objectStore(SNAPS).openCursor();
        req.onsuccess = () => {
          const c = req.result;
          if (!c) return res(out);
          if (String(c.key).startsWith(`disk:${id}:`)) out.push(c.key);
          c.continue();
        };
        req.onerror = () => rej(req.error);
      });
    } catch { keys = [...memSnaps.keys()].filter(k => String(k).startsWith(`disk:${id}:`)); }
    for (const k of keys) await drop(k);
  },
};

/** ask the browser not to evict our images/disks under storage pressure */
export const persistStorage = () => navigator.storage?.persist?.().catch(() => false);

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
