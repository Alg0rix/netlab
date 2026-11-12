import init, { NetLab } from './pkg/netlab_wasm.js';
import { IOSNode, iosArgs, detectIosPlatform } from './ios.js';
import { VMNode, VM_DEFAULTS, sniffImage } from './vm.js';
import { TEMPLATES, tpl, iconSVG, iconImage, preloadIcons, ifName as ifNameFor, UI } from './icons.js';
import { dissect, hexRows, toPcap, TICK_MS, matchFilter, protoClass, craftFrame, tcpFlow } from './capture.js';
import { SimShell, isMissingSession, parse_event } from './cli.js';
import { images, nvram, disks, persistStorage, newUid, autosave, prefs, download, pickFile, snapshots } from './store.js';
import { emitLab, parseLab, decodeLab, encodeNode, applySeats } from './labyaml.js';
import { toast, modal, confirmBox, ask, openMenu, closeMenu, ico, hydrateIcons } from './ui.js';
import { addTab, activateTab, removeTab, hasTab, refreshTabs } from './consolewin.js';
import { LEVEL_META, allTemplates, userTemplates, renderTemplatePreviewSVG } from './templates.js';

/* ════════════════════════════ boot ════════════════════════════ */

await init();
await preloadIcons();

import { $, $$, esc, h, isTyping, clamp, b64, unb64, maskLen, lenMask, PROTO_NAME } from './util.js';

let lab = new NetLab();
window.netlab = { get lab() { return lab; }, get view() { return { ...view }; }, get shapes() { return shapes; }, emus: null, meta: null };

/* ════════════════════════════ state ════════════════════════════ */

const meta = new Map();      // devId -> { tpl, image?, startup?, startupBoot? }
window.netlab.meta = meta;
const emus = new Map();      // devId -> IOSNode | VMNode (emulated nodes)
window.netlab.emus = emus;
let notes = [];              // { id, x, y, text }
let shapes = [];             // { id, kind: rect|ellipse|line, x, y, w, h, stroke, fill, dash, width, text }
let nextShapeId = 1;
const SHAPE_COLORS = ['#4c9bff', '#3ccf7e', '#f2b84b', '#ff5d5d', '#a58bff', '#2fd0c0', '#8a94a6'];
const SHAPE_TOOLS = ['rect', 'ellipse', 'line'];
let nextNoteId = 1;
let labName = 'Untitled lab';

let snap = { tick: 0, devices: [], links: [] };
let byId = new Map(), linkById = new Map();

const sel = { nodes: new Set(), links: new Set(), notes: new Set(), shapes: new Set() };
const selCount = () => sel.nodes.size + sel.links.size + sel.notes.size + sel.shapes.size;
const clearSel = () => { for (const s of Object.values(sel)) s.clear(); };
const view = { x: 0, y: 0, z: 1 };
let tool = 'select';
let armed = null;            // template id waiting to be placed
let cable = null;            // { dev, port } first end of a link being drawn
let hover = null;            // { type, id }
let mouse = { sx: 0, sy: 0, wx: 0, wy: 0, inside: false };
let gesture = null;          // active pointer gesture
let spaceDown = false;

const SPEEDS = [0.25, 0.5, 1, 2, 4];
let mode = 'realtime';
let playing = false;
let speed = 1;
let tickAcc = 0;             // ms accumulated toward the next tick
let tickFrac = 0;            // 0..1 progress between ticks (for smooth packets)

const P = {
  labels: prefs.get('labels', true),
  grid: prefs.get('grid', true),
  minimap: prefs.get('minimap', true),
  theme: prefs.get('theme', matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark'),
  dockH: prefs.get('dockH', 260),
  dockMin: prefs.get('dockMin', false),
};

const events = [];           // { n, tick, device, link, kind, msg, cat, node, via }
let evSeq = 0;
const evFilter = { cats: new Set(['ARP', 'ICMP', 'L2', 'L4', 'EXT', 'SYS']), warnOnly: false, q: '' };
let evSelected = null;

const caps = new Map();      // linkId -> { name, packets: [], paused }
let capLink = null, capPkt = null, capSeq = 0;

const consoles = new Map();  // devId -> { shell?, out, buf }
const conPlace = new Map();  // devId -> 'dock' when its console lives in the bottom panel (absent = floating window)
let dockTab = 'events';

const flashes = [];          // { dev, color, t0 }
let C = {};                  // resolved CSS colours

const WALL_TICK = 70;        // real ms per tick at 1× — animation pace
const NODE_HIT_W = 30, NODE_HIT_H = 34, ICON = 48, ICON_DY = -8;

/* ════════════════════════════ helpers ════════════════════════════ */

function refresh() {
  snap = JSON.parse(lab.snapshot());
  byId = new Map(snap.devices.map(d => [d.id, d]));
  linkById = new Map(snap.links.map(l => [l.id, l]));
  for (const s of [sel.nodes, sel.links]) for (const id of s) if (!(s === sel.nodes ? byId : linkById).has(id)) s.delete(id);
}

const tplOf = id => meta.get(id)?.tpl ?? byId.get(id)?.kind ?? 'pc';
const ifName = (id, i) => {
  if (tplOf(id) === 'ios-sw' && meta.get(id)?.iosPlatform !== '7200') return `fa1/${i}`; // NM-16ESW
  return ifNameFor(tplOf(id), i, portCount(id));
};
const ifNames = id => Array.from({ length: portCount(id) }, (_, i) => ifName(id, i));
// cli_set_ifnames is die-class on a non-canonical name — a throw inside a
// &mut self wasm export poisons the whole NetLab object. Veto names outside
// the canonical family (type-word head + digit/slash tail — the IfMap::new
// rule) so an icon-derived display name can never reach the export.
const canonIf = n => {
  const i = n.search(/\d/);
  return i > 0 && /^(g|fa|e|eth|vl)$/i.test(n.slice(0, i)) && /^[\d/]+$/.test(n.slice(i));
};
const setIfNames = (id, names) => { if (names.every(canonIf)) lab.cli_set_ifnames(id, names); };
const portCount = id => {
  const d = byId.get(id);
  const t = tpl(tplOf(id));
  const want = meta.get(id)?.ports ?? t?.ports ?? 1;
  return Math.min(d?.ports.length ?? 0, want);
};

function setNodePorts(id, count, noHist) {
  if (!noHist) checkpoint();
  const d = byId.get(id), m = meta.get(id), t = tpl(tplOf(id));
  if (!d || !m) return;
  const max = Math.min(d.ports.length, t?.maxPorts ?? d.ports.length);
  const n = clamp(count | 0, 1, max);
  for (let i = n; i < d.ports.length; i++) {
    const lid = d.ports[i]?.link;
    if (lid != null) { lab.remove_link(lid); caps.delete(lid); }
    if (d.ports[i]?.ip) lab.clear_ip(id, i);
  }
  m.ports = n;
  if (isVM(id)) m.nics = n;
  refresh();
  if (d.kind !== 'ext') setIfNames(id, ifNames(id));   // port set changed — rebind the IfMap
  renderAll();
  dirty();
}

function nodeState(id) {
  const d = byId.get(id);
  if (!d) return 'stopped';
  if (d.kind === 'ext') return emus.get(id)?.state ?? 'stopped';
  return d.powered ? 'running' : 'stopped';
}

function peerOf(link, dev) {
  return link.a === dev ? { dev: link.b, port: link.b_port } : { dev: link.a, port: link.a_port };
}

function nextName(prefix) {
  let n = 0;
  for (const d of snap.devices) {
    const m = d.name.match(new RegExp(`^${prefix}-?(\\d+)$`));
    if (m) n = Math.max(n, +m[1]);
  }
  return `${prefix}${n + 1}`;
}

const toWorld = (sx, sy) => ({ x: (sx - view.x) / view.z, y: (sy - view.y) / view.z });
const toScreen = (wx, wy) => ({ x: wx * view.z + view.x, y: wy * view.z + view.y });
const snapV = v => (P.grid ? Math.round(v / 20) * 20 : v);


/* ════════════════════════════ persistence ════════════════════════════ */

function serialize() {
  return {
    format: 'netlab', version: 1, name: labName,
    view: { ...view },
    nodes: snap.devices.map(d => {
      const m = meta.get(d.id);
      // Live snap → canonical seats through the codec's own emit — key names
      // and default-suppression stay identical to the YAML save.
      const enc = encodeNode({
        id: d.id, tpl: tplOf(d.id), name: d.name, x: Math.round(d.x), y: Math.round(d.y),
        powered: d.powered,
        startup: m?.startup, startupBoot: !!m?.startupBoot,
        ports: d.ports.slice(0, portCount(d.id)),
        routes: d.routes || [],
        mac_table: d.mac_table || [], arp_table: d.arp_table || [],
        vlans: d.vlans ?? undefined, toggles: d.toggles, ospf: d.ospf ?? undefined,
      });
      const n = {
        id: d.id, tpl: enc.tpl, name: enc.name, x: enc.x, y: enc.y,
        powered: d.powered,
        image: m?.image ?? null, uid: m?.uid,
        portCount: portCount(d.id),
        ...(isVM(d.id) ? { vm: { ram: m.ram, diskMB: m.diskMB, nic: m.nic, nics: portCount(d.id) } } : {}),
        ...(enc.meta ? { meta: enc.meta } : {}),
        // JSON save keeps access lists, their bindings, and the 1:1 maps —
        // the seats the YAML form deliberately leaves to meta.startup.
        acls: d.acls || [],
        static_nat: (d.nat?.statics ?? []).map(s => ({ inside: s.inside_local })),
        svc: { dhcp: !!d.svc_dhcp, dns: !!d.svc_dns, http: !!d.svc_http },
        resolver: d.resolver || '',
        dns: d.dns_records || [],
      };
      for (const k of ['ports', 'routes', 'mac', 'arp', 'vlans', 'toggles', 'ospf']) {
        if (enc[k] != null) n[k] = enc[k];
      }
      d.ports.slice(0, portCount(d.id)).forEach((p, i) => {
        if (!p.in_acl && !p.out_acl) return;
        const row = (n.ports ??= [])[i] ??= {};
        if (p.in_acl) row.in_acl = p.in_acl;
        if (p.out_acl) row.out_acl = p.out_acl;
      });
      return n;
    }),
    links: snap.links.map(l => ({
      a: l.a, ap: l.a_port, b: l.b, bp: l.b_port, up: l.up,
      delay: l.delay, jitter: l.jitter, loss: l.loss, mtu: l.mtu, speed: l.speed,
    })),
    notes: notes.map(({ _b, ...n }) => n),
    shapes: shapes.map(sh => ({ ...sh })),
  };
}

function restore(data) {
  for (const n of emus.values()) n.stop();   // async: grabs a last NVRAM snapshot first
  emus.clear();
  meta.clear();
  for (const id of [...consoles.keys()]) closeConsole(id, true);
  caps.clear(); capLink = capPkt = null;
  events.length = 0; evSeq = 0;
  clearSel();
  cable = null;
  try { lab.free(); } catch { /* already freed */ }
  lab = new NetLab();
  simTick = 0;

  // The ONE decode site — .netlab.json, YAML, autosave, undo and templates
  // all land here canonical before a single setter runs.
  data = decodeLab(data);
  labName = data.name || 'Untitled lab';
  const idmap = new Map();
  let minted = false;   // nodes from older saves get a uid now — persist it
  for (const n of data.nodes) {
    if (!n.uid) minted = true;
    const t = tpl(n.tpl) ?? tpl('pc');
    const id = lab.add_device(t.kind, n.name, n.x, n.y);
    idmap.set(n.id, id);
    const savedPorts = n.portCount ?? n.vm?.nics ?? (n.ports?.length ? Math.max(t.ports, n.ports.length) : t.ports);
    registerNode(id, t.id, n.name, n.image, n.uid, n.vm, savedPorts);
    const m = meta.get(id);
    if (typeof n.startup === 'string') m.startup = n.startup;   // "" decodes as Some("")
    if (n.startupBoot) m.startupBoot = true;
    for (const a of n.acls ?? []) {
      for (const e of a.entries ?? []) {
        const proto = PROTO_NAME[e.proto] || (typeof e.proto === 'string' ? e.proto : 'ip');
        lab.acl_add(id, { num: a.num, permit: !!e.permit, proto, src: e.src, src_wild: e.src_wild, dst: e.dst, dst_wild: e.dst_wild, sport: e.sport || 0, dport: e.dport || 0 });
      }
    }
    (n.ports ?? []).forEach((p, i) => {
      if (!p) return;
      if (p.ip) lab.set_ip(id, i, p.ip, p.mask);
      if (p.nat) lab.set_nat(id, i, p.nat);
      if (p.trunk || p.access_vlan || p.native_vlan || p.allowed) {
        lab.set_switchport(id, i, p.trunk ? 'trunk' : 'access', p.access_vlan || 1, p.native_vlan || 1, p.allowed || '');
      }
      if (p.helper) lab.set_helper(id, i, p.helper);
      if (p.in_acl) lab.set_access_group(id, i, p.in_acl, 'in');
      if (p.out_acl) lab.set_access_group(id, i, p.out_acl, 'out');
      if (p.ospf_area != null) lab.ospf_interface(id, i, p.ospf_area);
    });
    for (const s of n.static_nat ?? []) {
      const proto = s.proto === 6 || s.proto === 'tcp' ? 'tcp' : s.proto === 17 || s.proto === 'udp' ? 'udp' : 'ip';
      lab.nat_static(id, s.inside, proto, s.inside_port || 0, s.outside_port || 0);
    }
    if (n.svc) lab.set_services(id, !!n.svc.dhcp, !!n.svc.dns, !!n.svc.http);
    for (const rec of n.dns ?? []) if (rec?.[0] && rec?.[1]) lab.add_dns_record(id, rec[0], rec[1]);
    if (n.resolver) lab.set_dns(id, n.resolver);
    if (n.powered === false) lab.set_power(id, false);
    // Every remaining codec seat — admin_up, method, stats, mac, arp, static
    // routes, vlans, toggles, ospf — lands before the baseline so a reload
    // returns to the authored state. ext nodes carry no engine config. A bad
    // seat set skips THIS device's apply and surfaces; the load must not die
    // half-built on one malformed node.
    if (t.kind !== 'ext') {
      try { lab.apply_device_fields(id, JSON.stringify(applySeats(n))); }
      catch (e) { toast(`${n.name || `#${id}`}: ${e?.message ?? e}`, 'err'); }
    }
    lab.baseline_commit(id);
  }
  for (const l of data?.links ?? []) {
    const a = idmap.get(l.a), b = idmap.get(l.b);
    if (a == null || b == null) continue;
    const id = lab.add_link_ports(a, l.ap, b, l.bp);
    if (id >= 0 && l.up === false) lab.set_link_up(id, false);
    if (id >= 0 && (l.delay != null || l.jitter || l.loss || l.mtu || (l.speed && l.speed !== '1G'))) {
      lab.set_link_impair(id, l.delay ?? 4, l.jitter ?? 0, l.loss ?? 0, l.mtu ?? 0, l.speed || '1G');
    }
  }
  notes = (data?.notes ?? []).map(n => ({ ...n }));
  nextNoteId = notes.reduce((m, n) => Math.max(m, n.id), 0) + 1;
  shapes = (data?.shapes ?? []).map(sh => ({ ...sh }));
  nextShapeId = shapes.reduce((m, sh) => Math.max(m, sh.id), 0) + 1;
  if (data?.view) Object.assign(view, data.view);
  $('#lab-name').value = labName;
  refresh();
  renderAll();
  // The load itself is the undo boundary — the caller checkpointed before
  // restore(); a per-device replay checkpoint would push N undo entries.
  histLock = true;
  try { for (const d of snap.devices) applyStartup(d.id); }   // mirror sync + gated replay — one path for every load
  finally { histLock = false; }
  if (minted) dirty();
}

let saveTimer = null;
const undoStack = [];
const redoStack = [];
let histLock = false;
function checkpoint() {
  if (histLock) return false;
  const s = JSON.stringify(serialize());
  if (undoStack.at(-1) === s) return false;
  undoStack.push(s);
  if (undoStack.length > 50) undoStack.shift();
  redoStack.length = 0;
  return true;
}
function undo() {
  if (!undoStack.length) return;
  histLock = true;
  try {
    redoStack.push(JSON.stringify(serialize()));
    restore(JSON.parse(undoStack.pop()));
    dirty();
    toast('Undone — running VMs were stopped');
  } finally { histLock = false; }
}
function redo() {
  if (!redoStack.length) return;
  histLock = true;
  try {
    undoStack.push(JSON.stringify(serialize()));
    restore(JSON.parse(redoStack.pop()));
    dirty();
    toast('Redone — running VMs were stopped');
  } finally { histLock = false; }
}
function dirty() {
  $('#save-state').textContent = 'Saving…';
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    autosave.save(serialize());
    $('#save-state').textContent = 'Saved locally';
  }, 500);
}

/* ════════════════════════════ node lifecycle ════════════════════════════ */

const isVM = id => tpl(tplOf(id))?.emu === 'v86';
const isIOS = id => tpl(tplOf(id))?.emu === 'dynamips';
const isIOSSwitch = id => tplOf(id) === 'ios-sw';
const isRos = id => tplOf(id) === 'routeros' || tplOf(id) === 'mikrotik-sw';

function registerNode(id, tplId, name, image = null, uid = null, vm = null, ports = null) {
  const t = tpl(tplId);
  const initialPorts = ports ?? vm?.nics ?? t?.ports ?? 1;
  const m = { tpl: tplId, image, uid: uid || newUid(), nvSaved: null, ports: initialPorts };
  meta.set(id, m);
  // inject canonical port names into the CLI session — IfMap binds names[i] to PortId(i)
  if (t?.kind !== 'ext') setIfNames(id, Array.from({ length: initialPorts }, (_, i) => ifNameFor(tplId, i, initialPorts)));
  if (t?.emu === 'v86') {
    Object.assign(m, VM_DEFAULTS[tplId] ?? VM_DEFAULTS.x86, vm ?? {}, { ports: initialPorts, nics: initialPorts, diskSaved: null, diskSize: 0 });
    const node = new VMNode(id, name);
    node.onLog = msg => vmConsoleOf(id).log(msg);
    node.onSerial = bytes => vmConsoleOf(id).serial(bytes);
    node.onFrame = (dev, port, bytes) => lab.ext_inject(dev, port, bytes);
    // guest disk → IndexedDB (debounced after writes, and on stop)
    node.onDisk = buf => disks.put(m.uid, buf).then(rec => {
      m.diskSaved = rec.saved; m.diskSize = rec.size;
      autosave.save(serialize());
      renderInspectorSoon();
    }).catch(e => toast(e?.name === 'QuotaExceededError'
      ? `${byId.get(id)?.name}: browser storage is full — the VM disk could not be saved`
      : `${byId.get(id)?.name}: could not save the VM disk (${e?.message ?? e})`, 'err'));
    node.onState = st => {
      renderTopo(); renderInspectorSoon(); refreshTabs(); renderDockTabs();
      consoles.get(id)?.onState?.(st);
      pushEvent({ tick: snap.tick, device: id, link: null, kind: 'sys', msg: `${byId.get(id)?.name ?? name}: ${st === 'running' ? 'VM started' : st}` });
    };
    disks.info(m.uid).then(r => { if (r) { m.diskSaved = r.saved; m.diskSize = r.size; renderInspectorSoon(); } });
    emus.set(id, node);
  }
  if (t?.emu === 'dynamips') {
    const node = new IOSNode(id, name);
    // IOS `write memory` → NVRAM snapshot from the worker → IndexedDB
    node.onNvram = data => nvram.put(m.uid, data).then(rec => {
      m.nvSaved = rec.saved;
      autosave.save(serialize());   // the uid this NVRAM is keyed by must be durable now
      pushEvent({ tick: snap.tick, device: id, link: null, kind: 'sys', msg: `${byId.get(id)?.name ?? name}: startup-config saved in browser (${(data.length / 1024).toFixed(0)} KB NVRAM)` });
      renderInspector(true);
    });
    nvram.get(m.uid).then(r => { if (r) { m.nvSaved = r.saved; renderInspectorSoon(); } });
    // emulator stdout belongs in the node's console, not the packet event log
    node.onLog = m => {
      if (!consoles.has(id)) consoles.set(id, iosConsole(id));
      consoles.get(id).log(m);
    };
    node.onFrame = (dev, port, bytes) => lab.ext_inject(dev, port, bytes);
    node.term = bytes => consoleWrite(id, bytes);
    node.onState = st => {
      renderTopo(); renderInspectorSoon(); refreshTabs(); renderDockTabs();
      consoles.get(id)?.onState?.(st);
      pushEvent({ tick: snap.tick, device: id, link: null, kind: 'sys', msg: `${byId.get(id)?.name ?? name}: ${st === 'running' ? 'Dynamips started' : st}` });
    };
    emus.set(id, node);
  }
}

const STARTUP = {
  ios: '!\n! Paste into the IOS console with Apply.\n!\nhostname Router\n',
  'ios-sw': '!\nvlan database\nvlan 10\nexit\n',
  routeros: '/system identity set name=MT\n',
  'mikrotik-sw': '/interface bridge add name=br0 vlan-filtering=yes protocol-mode=rstp\n',
  'ovs-sw': 'ip link add br0 type bridge vlan_filtering 1\nip link set br0 up\n',
  frr: '# eth0–eth7. Paste at the Linux shell.\nip link set eth0 up\nvtysh -c \'conf t\' -c \'router ospf\' -c \'network 10.0.0.0/8 area 0\'\n',
  firewall: '# wan0 outside, dmz0, lan0–lan5 inside\nnft add table inet filter\nnft add chain inet filter forward \'{ type filter hook forward priority 0 ; policy drop ; }\'\nnft add rule inet filter forward ct state established,related accept\nnft add rule inet filter forward iifname "lan*" accept\n',
};

function addNode(tplId, x, y, noHist) {
  const t = tpl(tplId);
  if (!t || t.disabled) return null;
  if (!noHist) checkpoint();
  const name = nextName(t.prefix);
  const id = lab.add_device(t.kind, name, snapV(x), snapV(y));
  registerNode(id, t.id, name);
  if (STARTUP[t.id]) { meta.get(id).startup = STARTUP[t.id]; setStartupText(id, STARTUP[t.id]); }
  if (t.id === 'server') lab.set_services(id, true, true, true);
  refresh();
  selectOnly('node', id);
  dirty();
  return id;
}

function deleteSelection() {
  const n = selCount();
  if (!n) return;
  checkpoint();
  for (const id of sel.links) { lab.remove_link(id); caps.delete(id); }
  for (const id of sel.nodes) {
    for (const l of snap.links) if (l.a === id || l.b === id) caps.delete(l.id);
    lab.remove_device(id);
    emus.get(id)?.kill();
    if (isIOS(id)) nvram.remove(meta.get(id).uid);
    if (isVM(id)) disks.remove(meta.get(id).uid);
    emus.delete(id);
    meta.delete(id);
    closeConsole(id, true);
  }
  notes = notes.filter(x => !sel.notes.has(x.id));
  shapes = shapes.filter(x => !sel.shapes.has(x.id));
  clearSel();
  if (capLink != null && !caps.has(capLink)) { capLink = null; capPkt = null; }
  refresh();
  renderAll();
  dirty();
}

function duplicateSelection() {
  const ids = [...sel.nodes];
  if (!ids.length) return;
  checkpoint();
  const fresh = [];
  for (const id of ids) {
    const d = byId.get(id);
    const nid = addNode(tplOf(id), d.x + 40, d.y + 40, true);
    if (nid != null) {
      setNodePorts(nid, portCount(id), true);
      fresh.push(nid);
    }
  }
  clearSel();
  fresh.forEach(id => sel.nodes.add(id));
  renderAll();
}

async function startNode(id) {
  const d = byId.get(id);
  if (!d) return;
  if (d.kind !== 'ext') {
    lab.set_power(id, true);
    refresh(); renderAll(); dirty();
    applyStartup(id);
    return;
  }
  if (isVM(id)) return startVM(id);
  const node = emus.get(id);
  if (!crossOriginIsolated) return toast('IOS needs cross-origin isolation (serve with COOP/COEP — make serve)', 'err');
  if (node.state !== 'stopped') return;
  let name = meta.get(id).image;
  if (!name || !(await images.get(name))) {
    name = await imageManager(id);
    if (!name) return;
  }
  const rec = await images.get(name);
  if (!rec) return toast(`Image ${name} is missing — upload it again`, 'err');
  node.name = d.name;
  const platform = detectIosPlatform(rec.data);
  meta.get(id).iosPlatform = platform;
  const nv = await nvram.get(meta.get(id).uid);
  node.boot(rec.data, iosArgs(portCount(id), { isSwitch: isIOSSwitch(id), platform }), nv?.data ?? null);
  openConsole(id);
  renderAll();
  pushEvent({ tick: snap.tick, device: id, link: null, kind: 'sys', msg: `${d.name}: booting ${name} (c${platform}, ${(rec.size / 1e6).toFixed(1)} MB)` });
}

async function startVM(id) {
  const d = byId.get(id), m = meta.get(id), node = emus.get(id);
  if (node.state !== 'stopped') return;
  let name = m.image;
  const saved = await disks.get(m.uid);
  // an installed VM boots from its own disk; the image is only needed the first time
  if (!saved && (!name || !(await images.get(name)))) {
    name = await imageManager(id);
    if (!name) return;
  }
  const rec = name ? await images.get(name) : null;
  let disk, media = null;
  if (saved) {
    disk = saved.data;
    if (rec?.kind === 'iso') media = rec.data;          // keep the CD in the drive; disk boots first
  } else if (rec.kind === 'disk') {
    disk = rec.data;                                    // fresh private copy of the user's raw disk
  } else {
    disk = new ArrayBuffer(m.diskMB * 1024 * 1024);     // blank disk to install onto
    media = rec.data;
  }
  persistStorage();
  node.name = d.name;
  const c = vmConsoleOf(id);
  // an installed (bootable) disk talks on the serial console; installers draw on the VGA screen
  const mbr = new Uint8Array(disk, 510, 2);
  c.setView(mbr[0] === 0x55 && mbr[1] === 0xaa ? 'serial' : 'screen');
  openConsole(id);
  pushEvent({ tick: snap.tick, device: id, link: null, kind: 'sys', msg: `${d.name}: booting ${saved ? 'its saved disk' : name}` });
  await node.boot({ media, disk, ram: m.ram, nic: m.nic, nics: portCount(id) });
  if (!saved) node.saveDisk();                          // the uid now has a disk; installs persist from here
}

function stopNode(id) {
  const d = byId.get(id);
  if (!d) return;
  if (d.kind === 'ext') emus.get(id)?.stop();
  else { lab.set_power(id, false); refresh(); }
  renderAll();
  dirty();
}

function reloadNode(id) {
  const d = byId.get(id);
  if (!d || d.kind === 'ext') return;
  const wasPowered = d.powered;       // restore_baseline reinstates the authored flag — reload keeps the live state
  lab.reset_config(id);               // restores the authored baseline — the replay target
  lab.set_power(id, wasPowered);      // a running device reboots and comes back up; a stopped one stays down
  refresh(); renderAll();
  toast(`${d.name} reloaded — ARP/MAC tables cleared`);
  applyStartup(id);                   // mirror → startup_gate → cli_script
}

// Load-path honesty: per-line replay failures land in errors[]; they reach the
// operator, never a silent swallow.
function scriptErrors(id, r) {
  if (!r?.errors?.length) return;
  const name = byId.get(id)?.name ?? `#${id}`;
  const shown = r.errors.slice(0, 3).map(e => `line ${e.lineno}: ${e.msg}`).join(' · ');
  toast(`${name}: startup script — ${r.errors.length} line${r.errors.length === 1 ? '' : 's'} failed (${shown}${r.errors.length > 3 ? ', …' : ''})`, 'err');
}

// One guarded mirror write — the missing-session Err carries the adapter's
// semantics here too: orphan the open console's shell, never rethrow it.
const setStartupText = (id, text) => {
  try { lab.cli_set_startup(id, text); }
  catch (e) { if (!isMissingSession(e)) throw e; consoles.get(id)?.shell?.deleted(); }
};

// Boot/load replay. The mirror syncs unconditionally (text exists → mirror,
// absent → clear with null, never ""); the script leg runs only under
// startup_gate ≡ meta.startupBoot == true ∧ meta.startup is a string, and
// only on sim nodes — ext consoles take the script by paste instead.
// Replay target: the document-decoded authored state at load / the
// reset_config-restored baseline at reload — never bare constructor defaults.
function applyStartup(id) {
  const m = meta.get(id);
  setStartupText(id, m?.startup ?? null);
  if (m?.startupBoot !== true || typeof m?.startup !== 'string' || byId.get(id)?.kind === 'ext') return;
  checkpoint();                       // unconditional before every cli_script — dedup absorbs reads
  const r = lab.cli_script(id, m.startup);
  scriptErrors(id, r);
  if (r?.mutated) { refresh(); renderInspectorSoon(); dirty(); }
}

// Inspector Apply: paste the script into the node now — ungated, explicit.
function pasteStartup(id) {
  const m = meta.get(id);
  const d = byId.get(id);
  const text = (m?.startup ?? '').replace(/\r/g, '');
  if (!d || !text.trim()) return toast('Startup script is empty', 'err');
  setStartupText(id, m.startup);
  if (d.kind === 'ext') {
    const emu = emus.get(id);
    if (!emu || nodeState(id) !== 'running') return toast(`${d.name}: start it, wait for a prompt, then paste`, 'err');
    emu.sendKeys(new TextEncoder().encode(text.replace(/\n/g, '\r') + '\r'));
    toast(`${d.name}: pasted the startup script`);
    return;
  }
  if (nodeState(id) !== 'running') return toast(`${d.name}: start it before applying the script`, 'err');
  if (!consoles.has(id)) consoles.set(id, simConsole(id));
  checkpoint();                       // unconditional — the adapter's own push dedups to one
  consoles.get(id).shell.paste(text); // the adapter's batch path: render, errors, cli_prompt
  toast(`${d.name}: applied the startup script`);
  refresh(); dirty();
}

const startAll = () => snap.devices.forEach(d => nodeState(d.id) === 'stopped' && (d.kind !== 'ext' || meta.get(d.id)?.image || meta.get(d.id)?.diskSaved) && startNode(d.id));
const stopAll = () => snap.devices.forEach(d => stopNode(d.id));

function rename(id, name) {
  name = name.trim();
  if (!name || name === byId.get(id)?.name) return;
  checkpoint();
  lab.rename_device(id, name);
  const n = emus.get(id); if (n) n.name = name;
  refresh();
  renderAll();
  dirty();
}

function toggleLink(id) {
  const l = linkById.get(id);
  if (!l) return;
  checkpoint();
  lab.set_link_up(id, !l.up);
  refresh(); renderAll(); dirty();
}

function linkBadge(l) {
  const bits = [];
  if (l.speed && l.speed !== '1G') bits.push(l.speed === 'WAN' ? 'WAN' : l.speed);
  if ((l.delay ?? 4) > 4 || l.jitter) bits.push(l.jitter ? `${l.delay}ms±${l.jitter}` : `${l.delay}ms`);
  if (l.loss) bits.push(`${l.loss}%`);
  if (l.mtu) bits.push(`MTU ${l.mtu}`);
  return bits.join(' · ');
}

function linkTitle(l) {
  const a = byId.get(l.a), b = byId.get(l.b);
  return `${a?.name} ${ifName(l.a, l.a_port)} ↔ ${b?.name} ${ifName(l.b, l.b_port)}`;
}

function setCapture(id, on) {
  const l = linkById.get(id);
  if (!l) return;
  lab.set_capture(id, on);
  if (on) {
    if (!caps.has(id)) caps.set(id, { name: linkTitle(l), packets: [], paused: false });
    capLink = id; capPkt = null;
    showDock('capture');
  }
  refresh(); renderAll();
}

/* ════════════════════════════ simulation loop ════════════════════════════ */

// Engine event vocabulary (EvKind): link, ping, trace, dhcp, syslog — plus
// the app-local 'sys'. Categories only color and group the rows.
function catOf(ev) {
  switch (ev.kind) {
    case 'ping': case 'trace': return 'ICMP';
    case 'link': return 'L2';
    case 'dhcp': return 'L4';
    case 'syslog': case 'sys': return 'SYS';
    default: return /ARP/.test(ev.msg) ? 'ARP' : /ICMP|echo|reply/i.test(ev.msg) ? 'ICMP' : /DHCP|DNS|TCP|HTTP|UDP/i.test(ev.msg) ? 'L4' : /eth\[|ext/i.test(ev.msg) ? 'EXT' : 'L2';
  }
}
const CAT_COLOR = { ARP: 'p-arp', ICMP: 'p-icmp', L2: 'p-l2', L4: 'p-tcp', EXT: 'p-ext', SYS: 'muted' };

// Event bookkeeping — the log row + port flash. The console render is a
// separate channel so a drain burst can batch it.
function recordEvent(ev) {
  ev.n = ++evSeq;
  ev.cat = catOf(ev);
  ev.node = byId.get(ev.device)?.name ?? '';
  const l = ev.link != null ? linkById.get(ev.link) : null;
  ev.via = l ? (() => { const p = peerOf(l, ev.device); return p.dev === ev.device ? '' : `${byId.get(p.dev)?.name ?? '?'}`; })() : '';
  events.push(ev);
  if (events.length > 5000) events.splice(0, 1000);
  if (ev.device != null) {
    const down = ev.kind === 'link' && /to down/.test(ev.msg);
    flashes.push({ dev: ev.device, color: down ? C.bad : C[CAT_COLOR[ev.cat]] ?? C.accent, t0: performance.now() });
    if (flashes.length > 60) flashes.shift();
  }
  evDirty = true;
}

function pushEvent(ev) {
  recordEvent(ev);
  for (const c of consoles.values()) c.shell?.onEvent(ev);
}

// Fabric clock. Realtime runs ~1 tick per millisecond on its own timer so real
// guests (IOS, RouterOS) see LAN-like latency and keep working when the tab is
// hidden (requestAnimationFrame stops there). Simulation keeps the slow,
// animated pace driven by play/step.
const RT_TICKS_PER_MS = 1;
const MAX_TICKS_PER_PUMP = 2000;
let simTick = 0;

/** advance the fabric n ticks; snapshot/events are collected once afterwards */
function runTicks(n) {
  if (n <= 0) return;
  for (let i = 0; i < n; i++) {
    lab.tick();
    simTick++;
    for (const { dev, port, bytes } of lab.drain_ext()) emus.get(dev)?.tx(port, bytes);
    for (const c of lab.drain_captures()) {
      const s = caps.get(c.link);
      if (s && !s.paused) {
        s.packets.push({ n: ++capSeq, tick: c.tick, from: c.from, bytes: c.bytes, d: dissect(c.bytes) });
        if (s.packets.length > 10000) s.packets.splice(0, 2000);
        capDirty = true;
      }
    }
    for (const c of consoles.values()) c.shell?.onTick();
  }
  // One batch render per console per pump — a burst writes its spans in a
  // single fragment instead of paying a layout read per event.
  const evs = JSON.parse(lab.drain_events());
  for (const ev of evs) recordEvent(ev);
  for (const c of consoles.values()) c.shell?.onEvents?.(evs);
  snapDirty = true;
}
const tick = () => { runTicks(1); refresh(); snapDirty = false; };

let snapDirty = false, lastPump = performance.now();
function pump() {
  const now = performance.now();
  const dt = Math.min(1000, now - lastPump);
  lastPump = now;
  if (mode === 'realtime') {
    runTicks(Math.min(MAX_TICKS_PER_PUMP, Math.round(dt * RT_TICKS_PER_MS * speed)));
    tickFrac = 0;
  } else if (playing) {
    tickAcc += dt * speed;
    let n = 0;
    while (tickAcc >= WALL_TICK && n < 8) { tickAcc -= WALL_TICK; n++; }
    if (n >= 8) tickAcc = 0;
    runTicks(n);
    tickFrac = tickAcc / WALL_TICK;
  } else tickFrac = 0;
  // the canvas only needs a fresh snapshot about once a frame
  if (snapDirty && (mode !== 'realtime' || now - lastSnap > 15)) { refresh(); snapDirty = false; lastSnap = now; }
}
let lastSnap = 0;
setInterval(pump, 4);

let evDirty = false, capDirty = false, slowT = 0;
function frame(now) {
  draw(now);
  if (P.minimap) drawMinimap();
  if (now - slowT > 200) {
    slowT = now;
    if (evDirty && dockTab === 'events') renderEvents();
    if (capDirty && dockTab === 'capture') renderCapture();
    if (evDirty || capDirty) renderDockTabs();
    evDirty = capDirty = false;
    renderStatus();
    renderTopo();
    renderInspectorSoon();
  }
  requestAnimationFrame(frame);
}

function stepOnce() {
  if (mode !== 'sim') setMode('sim');
  tick();
  tickAcc = 0;
}

function setMode(m) {
  if (m === 'sim' && mode !== 'sim' && [...emus.values()].some(n => n.state !== 'stopped')) {
    toast('Simulation pauses the network: running IOS/RouterOS/VM guests get no traffic until you press Play or switch back to Realtime.');
  }
  mode = m;
  playing = false;
  $$('#mode button').forEach(b => b.classList.toggle('on', b.dataset.mode === m));
  $('#sim-ctl').classList.toggle('live', m === 'sim');
  updatePlay();
  renderStatus();
}
function updatePlay() {
  const b = $('#sim-play');
  b.classList.toggle('on', playing);
  b.innerHTML = playing ? UI.pause : UI.play;
}

/* ════════════════════════════ canvas ════════════════════════════ */

const cv = $('#canvas'), ctx = cv.getContext('2d');
const mm = $('#minimap'), mctx = mm.getContext('2d');
let W = 0, H = 0, DPR = 1;

function resize() {
  DPR = devicePixelRatio || 1;
  // keep whatever was in the middle of the view in the middle
  if (W && H) { view.x += (cv.clientWidth - W) / 2; view.y += (cv.clientHeight - H) / 2; }
  W = cv.clientWidth; H = cv.clientHeight;
  cv.width = W * DPR; cv.height = H * DPR;
  mm.width = 200 * DPR; mm.height = 130 * DPR;
}
new ResizeObserver(resize).observe(cv);
resize();

function readColors() {
  const cs = getComputedStyle(document.documentElement);
  for (const k of ['canvas', 'panel', 'panel-2', 'line', 'line-2', 'text', 'text-2', 'muted', 'accent', 'accent-soft', 'ok', 'warn', 'bad', 'grid-dot', 'grid-major', 'link', 'p-arp', 'p-icmp', 'p-l2', 'p-ext', 'p-drop', 'p-ok', 'p-tcp', 'p-udp', 'p-dhcp', 'p-dns', 'p-stp', 'p-vlan', 'p-route', 'p-ospf', 'p-raw'])
    C[k] = cs.getPropertyValue('--' + k).trim();
}

function nodeAt(wx, wy) {
  for (let i = snap.devices.length - 1; i >= 0; i--) {
    const d = snap.devices[i];
    if (Math.abs(wx - d.x) <= NODE_HIT_W && wy >= d.y + ICON_DY - 28 && wy <= d.y + 44) return d;
  }
  return null;
}
function noteAt(wx, wy) {
  for (let i = notes.length - 1; i >= 0; i--) {
    const n = notes[i], b = n._b;
    if (b && wx >= b.x && wx <= b.x + b.w && wy >= b.y && wy <= b.y + b.h) return n;
  }
  return null;
}
function segDist(px, py, ax, ay, bx, by) {
  const dx = bx - ax, dy = by - ay, L = dx * dx + dy * dy || 1;
  const t = clamp(((px - ax) * dx + (py - ay) * dy) / L, 0, 1);
  return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
}
function linkAt(wx, wy) {
  let best = null, bd = 7 / view.z;
  for (const l of snap.links) {
    const a = byId.get(l.a), b = byId.get(l.b);
    if (!a || !b) continue;
    const d = segDist(wx, wy, a.x, a.y + ICON_DY, b.x, b.y + ICON_DY);
    if (d < bd) { bd = d; best = l; }
  }
  return best;
}
function hitAt(wx, wy) {
  const n = noteAt(wx, wy); if (n) return { type: 'note', id: n.id, obj: n };
  const d = nodeAt(wx, wy); if (d) return { type: 'node', id: d.id, obj: d };
  const l = linkAt(wx, wy); if (l) return { type: 'link', id: l.id, obj: l };
  const sh = shapeAt(wx, wy); if (sh) return { type: 'shape', id: sh.id, obj: sh };
  return null;
}

/* ── shapes ── */
const norm = sh => ({ x: Math.min(sh.x, sh.x + sh.w), y: Math.min(sh.y, sh.y + sh.h), w: Math.abs(sh.w), h: Math.abs(sh.h) });
function hexA(hex, a) {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
}
function shapeAt(wx, wy) {
  const tol = 6 / view.z;
  for (let i = shapes.length - 1; i >= 0; i--) {
    const sh = shapes[i], b = norm(sh), t = tol + sh.width / 2;
    if (sh.kind === 'line') {
      if (segDist(wx, wy, sh.x, sh.y, sh.x + sh.w, sh.y + sh.h) < t) return sh;
      continue;
    }
    if (sh.kind === 'rect') {
      const nearEdge = wx >= b.x - t && wx <= b.x + b.w + t && wy >= b.y - t && wy <= b.y + b.h + t &&
        (Math.abs(wx - b.x) < t || Math.abs(wx - b.x - b.w) < t || Math.abs(wy - b.y) < t || Math.abs(wy - b.y - b.h) < t);
      if (nearEdge || inLabel(sh, b, wx, wy)) return sh;
      continue;
    }
    const rx = b.w / 2 || 1, ry = b.h / 2 || 1, dx = wx - b.x - rx, dy = wy - b.y - ry;
    const r = Math.sqrt((dx / rx) ** 2 + (dy / ry) ** 2);
    if (Math.abs(r - 1) * Math.min(rx, ry) < t || inLabel(sh, b, wx, wy)) return sh;
  }
  return null;
}
// interiors are click-through (zones sit under nodes); the label is a grab handle
function inLabel(sh, b, wx, wy) {
  if (!sh.text) return false;
  const w = sh.text.length * 7 + 8;
  if (sh.kind === 'rect') return wx >= b.x + 6 && wx <= b.x + 10 + w && wy >= b.y + 4 && wy <= b.y + 24;
  const cx = b.x + b.w / 2, ty = b.y + Math.min(14, b.h / 4);
  return Math.abs(wx - cx) <= w / 2 && wy >= ty - 4 && wy <= ty + 18;
}
function shapeHandles(sh) {
  if (sh.kind === 'line') return [['p1', sh.x, sh.y], ['p2', sh.x + sh.w, sh.y + sh.h]];
  const b = norm(sh), mx = b.x + b.w / 2, my = b.y + b.h / 2, r = b.x + b.w, btm = b.y + b.h;
  return [['nw', b.x, b.y], ['n', mx, b.y], ['ne', r, b.y], ['e', r, my], ['se', r, btm], ['s', mx, btm], ['sw', b.x, btm], ['w', b.x, my]];
}
function handleAt(wx, wy) {
  if (sel.shapes.size !== 1 || selCount() !== 1) return null;
  const sh = shapes.find(x => sel.shapes.has(x.id));
  if (!sh) return null;
  for (const [k, x, y] of shapeHandles(sh)) if (Math.abs(wx - x) * view.z < 7 && Math.abs(wy - y) * view.z < 7) return { sh, k };
  return null;
}
const HANDLE_CURSOR = { nw: 'nwse-resize', se: 'nwse-resize', ne: 'nesw-resize', sw: 'nesw-resize', n: 'ns-resize', s: 'ns-resize', e: 'ew-resize', w: 'ew-resize', p1: 'move', p2: 'move' };

function drawShape(sh) {
  const b = norm(sh);
  ctx.save();
  ctx.lineWidth = sh.width;
  ctx.strokeStyle = sh.stroke;
  ctx.setLineDash(sh.dash ? [sh.width * 4, sh.width * 3] : []);
  ctx.lineCap = 'round';
  ctx.beginPath();
  if (sh.kind === 'line') { ctx.moveTo(sh.x, sh.y); ctx.lineTo(sh.x + sh.w, sh.y + sh.h); }
  else if (sh.kind === 'rect') ctx.roundRect(b.x, b.y, b.w, b.h, Math.min(8, b.w / 4, b.h / 4));
  else ctx.ellipse(b.x + b.w / 2, b.y + b.h / 2, b.w / 2, b.h / 2, 0, 0, Math.PI * 2);
  if (sh.fill && sh.kind !== 'line') { ctx.fillStyle = hexA(sh.stroke, 0.09); ctx.fill(); }
  ctx.stroke();
  if (sh.text) {
    UIF ??= uiFont();
    ctx.setLineDash([]);
    ctx.font = `600 12px ${UIF}`;
    ctx.fillStyle = sh.stroke;
    ctx.textBaseline = 'top';
    if (sh.kind === 'rect') { ctx.textAlign = 'left'; ctx.fillText(sh.text, b.x + 10, b.y + 8); }
    else if (sh.kind === 'ellipse') { ctx.textAlign = 'center'; ctx.fillText(sh.text, b.x + b.w / 2, b.y + Math.min(14, b.h / 4)); }
    else { ctx.textAlign = 'center'; ctx.textBaseline = 'bottom'; ctx.fillText(sh.text, sh.x + sh.w / 2, sh.y + sh.h / 2 - 6); }
  }
  ctx.restore();
}
function drawHandles(sh) {
  const b = norm(sh), r = 4 / view.z;
  ctx.save();
  if (sh.kind !== 'line') {
    ctx.strokeStyle = C.accent; ctx.lineWidth = 1 / view.z; ctx.setLineDash([4 / view.z, 3 / view.z]);
    ctx.strokeRect(b.x, b.y, b.w, b.h);
    ctx.setLineDash([]);
  }
  for (const [, x, y] of shapeHandles(sh)) {
    ctx.fillStyle = C.panel; ctx.strokeStyle = C.accent; ctx.lineWidth = 1.5 / view.z;
    ctx.beginPath(); ctx.rect(x - r, y - r, r * 2, r * 2); ctx.fill(); ctx.stroke();
  }
  ctx.restore();
}
async function editShapeText(sh) {
  const t = await ask('Shape label', sh.text ?? '', { ok: 'Save', placeholder: 'e.g. DMZ, VLAN 10, Branch A' });
  if (t != null) { sh.text = t.trim(); dirty(); renderInspector(true); }
}
function reorderShape(sh, front) {
  shapes = shapes.filter(x => x !== sh);
  front ? shapes.push(sh) : shapes.unshift(sh);
  dirty();
}

/** canvas-local CSS px; offsetX is unreliable for synthetic/transformed events */
function local(e) { const r = cv.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; }
function capturePointer(e) { try { cv.setPointerCapture(e.pointerId); } catch { /* synthetic or already released */ } }
function roundRect(c, x, y, w, h, r) { c.beginPath(); c.roundRect(x, y, w, h, r); }

function draw(now) {
  ctx.setTransform(DPR, 0, 0, DPR, 0, 0);
  ctx.fillStyle = C.canvas;
  ctx.fillRect(0, 0, W, H);
  ctx.setTransform(DPR * view.z, 0, 0, DPR * view.z, DPR * view.x, DPR * view.y);

  const tl = toWorld(0, 0), br = toWorld(W, H);
  if (P.grid) drawGrid(tl, br);

  for (const sh of shapes) drawShape(sh);

  // links
  const running = id => nodeState(id) === 'running';
  for (const l of snap.links) {
    const a = byId.get(l.a), b = byId.get(l.b);
    if (!a || !b) continue;
    const ax = a.x, ay = a.y + ICON_DY, bx = b.x, by = b.y + ICON_DY;
    const isSel = sel.links.has(l.id), isHover = hover?.type === 'link' && hover.id === l.id;
    ctx.lineCap = 'round';
    if (isSel || isHover) {
      ctx.strokeStyle = C.accent; ctx.globalAlpha = isSel ? .28 : .14; ctx.lineWidth = 8;
      ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.stroke(); ctx.globalAlpha = 1;
    }
    ctx.strokeStyle = !l.up ? C.bad : isSel ? C.accent : C.link;
    ctx.lineWidth = isSel ? 2.2 : 1.6;
    ctx.setLineDash(l.up ? [] : [6, 5]);
    ctx.beginPath(); ctx.moveTo(ax, ay); ctx.lineTo(bx, by); ctx.stroke();
    ctx.setLineDash([]);

    const len = Math.hypot(bx - ax, by - ay) || 1, ux = (bx - ax) / len, uy = (by - ay) / len;
    // endpoint status dots — green when the cable is up and both ends run (GNS3 semantics)
    const ok = l.up && running(l.a) && running(l.b);
    for (const [x, y, s] of [[ax, ay, 1], [bx, by, -1]]) {
      const off = Math.min(32, len / 2 - 4);
      ctx.fillStyle = ok ? C.ok : C.bad;
      ctx.beginPath(); ctx.arc(x + ux * off * s, y + uy * off * s, 3.2, 0, 7); ctx.fill();
    }
    if (P.labels && len > 120) {
      ctx.font = `500 10px ${getMono()}`;
      for (const [x, y, s, dev, port] of [[ax, ay, 1, l.a, l.a_port], [bx, by, -1, l.b, l.b_port]]) {
        // walk out along the link until clear of the node's icon + name/IP block
        let off = 30;
        const inBox = d => { const px = ux * d * s, py = uy * d * s; return Math.abs(px) < 46 && py > -36 && py < 62; };
        while (inBox(off) && off < len / 2 - 14) off += 3;
        off += 12;
        if (off > len / 2 - 8) continue;
        pill(x + ux * off * s, y + uy * off * s, ifName(dev, port), C.panel, C['text-2'], C.line);
      }
    }
    const badge = linkBadge(l);
    if (badge && len > 70) {
      const mx = (ax + bx) / 2, my = (ay + by) / 2;
      ctx.font = `600 10px ${getMono()}`;
      pill(mx, my + (l.capture ? 16 : 0), badge, C.panel, C['text-2'], C.line);
    }
    if (l.capture) {
      const mx = (ax + bx) / 2, my = (ay + by) / 2;
      ctx.fillStyle = C.panel; ctx.strokeStyle = C.bad; ctx.lineWidth = 1.5;
      ctx.beginPath(); ctx.arc(mx, my, 8, 0, 7); ctx.fill(); ctx.stroke();
      ctx.fillStyle = C.bad; ctx.globalAlpha = .55 + .45 * Math.sin(now / 250);
      ctx.beginPath(); ctx.arc(mx, my, 3.5, 0, 7); ctx.fill(); ctx.globalAlpha = 1;
    }
    // packets in flight, interpolated between ticks
    for (const f of l.frames) {
      const t = clamp(f.t + f.dir * tickFrac / (l.delay || 4), 0, 1);
      const x = ax + (bx - ax) * t, y = ay + (by - ay) * t;
      const proto = ({ arp: 'ARP', icmp: 'ICMP', tcp: 'TCP', udp: 'UDP', dhcp: 'DHCP', dns: 'DNS', ospf: 'OSPF', vlan: 'VLAN', stp: 'STP' })[f.color] || 'ETH';
      const col = C['p-' + (f.color || 'ext')] || C['p-ext'];
      if (view.z < .55) { ctx.fillStyle = col; ctx.beginPath(); ctx.arc(x, y, 5, 0, 7); ctx.fill(); continue; }
      ctx.font = `700 9.5px ${getMono()}`;
      const w = ctx.measureText(proto).width + 12;
      ctx.shadowColor = col; ctx.shadowBlur = 10;
      ctx.fillStyle = col; roundRect(ctx, x - w / 2, y - 8, w, 16, 5); ctx.fill();
      ctx.shadowBlur = 0;
      ctx.fillStyle = '#0b0e13'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.fillText(proto, x, y + .5);
    }
  }

  // link being drawn
  if (cable) {
    const a = byId.get(cable.dev);
    if (a) {
      ctx.strokeStyle = C.accent; ctx.lineWidth = 1.8; ctx.setLineDash([5, 5]);
      ctx.lineDashOffset = -now / 40;
      ctx.beginPath(); ctx.moveTo(a.x, a.y + ICON_DY); ctx.lineTo(mouse.wx, mouse.wy); ctx.stroke();
      ctx.setLineDash([]); ctx.lineDashOffset = 0;
    }
  }

  // notes
  for (const n of notes) drawNote(n);

  // flashes
  for (let i = flashes.length - 1; i >= 0; i--) {
    const f = flashes[i], k = (now - f.t0) / 550;
    if (k >= 1) { flashes.splice(i, 1); continue; }
    const d = byId.get(f.dev); if (!d) continue;
    ctx.strokeStyle = f.color; ctx.globalAlpha = (1 - k) * .8; ctx.lineWidth = 2;
    ctx.beginPath(); ctx.arc(d.x, d.y + ICON_DY, 24 + k * 18, 0, 7); ctx.stroke();
    ctx.globalAlpha = 1;
  }

  // nodes
  for (const d of snap.devices) drawNode(d, now);

  // shape being drawn / handles of the selected shape
  if (sel.shapes.size === 1 && selCount() === 1) {
    const sh = shapes.find(x => sel.shapes.has(x.id));
    if (sh) drawHandles(sh);
  }

  // ghost for placement
  if (armed && mouse.inside) {
    ctx.globalAlpha = .55;
    ctx.drawImage(iconImage(armed), snapV(mouse.wx) - ICON / 2, snapV(mouse.wy) + ICON_DY - ICON / 2, ICON, ICON);
    ctx.globalAlpha = 1;
  }

  // rubber band
  if (gesture?.type === 'band') {
    const { x0, y0 } = gesture, x1 = mouse.wx, y1 = mouse.wy;
    ctx.fillStyle = C['accent-soft']; ctx.strokeStyle = C.accent; ctx.lineWidth = 1 / view.z;
    ctx.fillRect(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
    ctx.strokeRect(Math.min(x0, x1), Math.min(y0, y1), Math.abs(x1 - x0), Math.abs(y1 - y0));
  }
}

let monoFont = null;
const getMono = () => monoFont ??= getComputedStyle(document.documentElement).getPropertyValue('--mono');
const uiFont = () => getComputedStyle(document.documentElement).getPropertyValue('--font');
let UIF = null;

function pill(x, y, text, bg, fg, border) {
  const w = ctx.measureText(text).width + 10;
  ctx.fillStyle = bg; ctx.strokeStyle = border; ctx.lineWidth = 1;
  roundRect(ctx, x - w / 2, y - 8, w, 16, 8); ctx.fill(); ctx.stroke();
  ctx.fillStyle = fg; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  ctx.fillText(text, x, y + .5);
}

function drawGrid(tl, br) {
  const step = view.z < .5 ? 100 : 20;
  ctx.fillStyle = C['grid-dot'];
  const r = 1.1 / view.z;
  for (let x = Math.floor(tl.x / step) * step; x <= br.x; x += step)
    for (let y = Math.floor(tl.y / step) * step; y <= br.y; y += step) {
      const major = x % 100 === 0 && y % 100 === 0;
      ctx.fillRect(x - r * (major ? 1.4 : 1), y - r * (major ? 1.4 : 1), r * (major ? 2.8 : 2), r * (major ? 2.8 : 2));
    }
}

function drawNode(d, now) {
  const x = d.x, y = d.y + ICON_DY;
  const isSel = sel.nodes.has(d.id), isHover = hover?.type === 'node' && hover.id === d.id;
  const isCableSrc = cable?.dev === d.id;
  const st = nodeState(d.id);
  UIF ??= uiFont();

  if (isSel || isHover || isCableSrc) {
    ctx.fillStyle = isSel || isCableSrc ? C['accent-soft'] : C['panel-2'];
    ctx.strokeStyle = isSel || isCableSrc ? C.accent : C.line;
    ctx.lineWidth = 1.2;
    roundRect(ctx, x - 40, y - 32, 80, 98, 12); ctx.fill();
    if (isSel || isCableSrc) ctx.stroke();
  }
  ctx.globalAlpha = st === 'stopped' ? .6 : 1;
  ctx.drawImage(iconImage(tplOf(d.id)), x - ICON / 2, y - ICON / 2, ICON, ICON);
  ctx.globalAlpha = 1;

  // status dot
  const sc = st === 'running' ? C.ok : st === 'booting' ? C.warn : C.bad;
  ctx.fillStyle = C.canvas; ctx.beginPath(); ctx.arc(x + 20, y - 18, 6, 0, 7); ctx.fill();
  ctx.fillStyle = sc;
  if (st === 'booting') ctx.globalAlpha = .5 + .5 * Math.sin(now / 160);
  ctx.beginPath(); ctx.arc(x + 20, y - 18, 4, 0, 7); ctx.fill();
  ctx.globalAlpha = 1;

  // labels with halo for legibility over links
  ctx.textAlign = 'center'; ctx.textBaseline = 'alphabetic';
  ctx.lineJoin = 'round';
  ctx.font = `600 12px ${UIF}`;
  ctx.strokeStyle = C.canvas; ctx.lineWidth = 4;
  ctx.strokeText(d.name, x, y + 38); ctx.fillStyle = C.text; ctx.fillText(d.name, x, y + 38);
  const ip = d.ports.find(p => p.ip)?.ip;
  if (ip) {
    ctx.font = `10.5px ${getMono()}`;
    ctx.strokeText(ip, x, y + 51); ctx.fillStyle = C.muted; ctx.fillText(ip, x, y + 51);
  }
}

function drawNote(n) {
  UIF ??= uiFont();
  ctx.font = `12.5px ${UIF}`;
  const lines = String(n.text || ' ').split('\n');
  const w = Math.max(60, ...lines.map(l => ctx.measureText(l).width)) + 20;
  const hh = lines.length * 17 + 14;
  n._b = { x: n.x, y: n.y, w, h: hh };
  const isSel = sel.notes.has(n.id);
  ctx.fillStyle = C['panel-2']; ctx.strokeStyle = isSel ? C.accent : C.line; ctx.lineWidth = isSel ? 1.6 : 1;
  roundRect(ctx, n.x, n.y, w, hh, 8); ctx.fill(); ctx.stroke();
  ctx.fillStyle = C.warn; roundRect(ctx, n.x, n.y + 6, 3, hh - 12, 2); ctx.fill();
  ctx.fillStyle = C['text-2']; ctx.textAlign = 'left'; ctx.textBaseline = 'top';
  lines.forEach((l, i) => ctx.fillText(l, n.x + 11, n.y + 8 + i * 17));
}

function contentBounds() {
  const xs = [], ys = [];
  for (const d of snap.devices) { xs.push(d.x - 40, d.x + 40); ys.push(d.y - 45, d.y + 55); }
  for (const n of notes) { const b = n._b ?? { x: n.x, y: n.y, w: 100, h: 30 }; xs.push(b.x, b.x + b.w); ys.push(b.y, b.y + b.h); }
  for (const sh of shapes) { const b = norm(sh); xs.push(b.x, b.x + b.w); ys.push(b.y, b.y + b.h); }
  if (!xs.length) return null;
  return { x0: Math.min(...xs), y0: Math.min(...ys), x1: Math.max(...xs), y1: Math.max(...ys) };
}

function fit() {
  const b = contentBounds();
  if (!b) { view.x = W / 2; view.y = H / 2; view.z = 1; return zoomChanged(); }
  const pad = 70;
  view.z = clamp(Math.min((W - pad * 2) / (b.x1 - b.x0 || 1), (H - pad * 2) / (b.y1 - b.y0 || 1)), .25, 1.6);
  view.x = W / 2 - (b.x0 + b.x1) / 2 * view.z;
  view.y = H / 2 - (b.y0 + b.y1) / 2 * view.z;
  zoomChanged();
}
function zoomAt(sx, sy, factor) {
  const w = toWorld(sx, sy);
  view.z = clamp(view.z * factor, .2, 3);
  view.x = sx - w.x * view.z; view.y = sy - w.y * view.z;
  zoomChanged();
}
function zoomChanged() { $('#zoom-read').textContent = Math.round(view.z * 100) + '%'; }

function drawMinimap() {
  const w = 200, hgt = 130;
  mctx.setTransform(DPR, 0, 0, DPR, 0, 0);
  mctx.clearRect(0, 0, w, hgt);
  const b = contentBounds();
  const vtl = toWorld(0, 0), vbr = toWorld(W, H);
  const x0 = Math.min(b?.x0 ?? vtl.x, vtl.x), y0 = Math.min(b?.y0 ?? vtl.y, vtl.y);
  const x1 = Math.max(b?.x1 ?? vbr.x, vbr.x), y1 = Math.max(b?.y1 ?? vbr.y, vbr.y);
  const s = Math.min((w - 16) / (x1 - x0 || 1), (hgt - 16) / (y1 - y0 || 1));
  const ox = (w - (x1 - x0) * s) / 2 - x0 * s, oy = (hgt - (y1 - y0) * s) / 2 - y0 * s;
  mm._t = { s, ox, oy };
  for (const sh of shapes) {
    const b = norm(sh);
    mctx.strokeStyle = hexA(sh.stroke, .6); mctx.lineWidth = 1;
    mctx.beginPath();
    if (sh.kind === 'line') { mctx.moveTo(sh.x * s + ox, sh.y * s + oy); mctx.lineTo((sh.x + sh.w) * s + ox, (sh.y + sh.h) * s + oy); }
    else if (sh.kind === 'rect') mctx.rect(b.x * s + ox, b.y * s + oy, b.w * s, b.h * s);
    else mctx.ellipse((b.x + b.w / 2) * s + ox, (b.y + b.h / 2) * s + oy, b.w / 2 * s, b.h / 2 * s, 0, 0, 7);
    mctx.stroke();
  }
  mctx.strokeStyle = C.link; mctx.lineWidth = 1;
  for (const l of snap.links) {
    const a = byId.get(l.a), c = byId.get(l.b); if (!a || !c) continue;
    mctx.beginPath(); mctx.moveTo(a.x * s + ox, a.y * s + oy); mctx.lineTo(c.x * s + ox, c.y * s + oy); mctx.stroke();
  }
  for (const d of snap.devices) {
    mctx.fillStyle = sel.nodes.has(d.id) ? C.accent : nodeState(d.id) === 'running' ? C.ok : C.muted;
    mctx.beginPath(); mctx.arc(d.x * s + ox, d.y * s + oy, 2.6, 0, 7); mctx.fill();
  }
  mctx.strokeStyle = C.accent; mctx.lineWidth = 1.2;
  mctx.fillStyle = C['accent-soft'];
  mctx.fillRect(vtl.x * s + ox, vtl.y * s + oy, (vbr.x - vtl.x) * s, (vbr.y - vtl.y) * s);
  mctx.strokeRect(vtl.x * s + ox, vtl.y * s + oy, (vbr.x - vtl.x) * s, (vbr.y - vtl.y) * s);
}
function minimapGo(e) {
  const r = mm.getBoundingClientRect(), t = mm._t; if (!t) return;
  const wx = (e.clientX - r.left - t.ox) / t.s, wy = (e.clientY - r.top - t.oy) / t.s;
  view.x = W / 2 - wx * view.z; view.y = H / 2 - wy * view.z;
}
mm.addEventListener('pointerdown', e => { minimapGo(e); mm.setPointerCapture(e.pointerId); mm.onpointermove = minimapGo; });
mm.addEventListener('pointerup', () => { mm.onpointermove = null; });

/* ════════════════════════════ canvas input ════════════════════════════ */

function selectOnly(type, id) {
  clearSel();
  if (type) sel[type + 's'].add(id);
  renderInspector(true);
  renderTopo(true);
}
function toggleSel(type, id) {
  const s = sel[type + 's'];
  s.has(id) ? s.delete(id) : s.add(id);
  renderInspector(true);
  renderTopo(true);
}

function setTool(t) {
  tool = t;
  armed = null;
  cable = null;
  $$('#toolbar [data-tool]').forEach(b => b.classList.toggle('on', b.dataset.tool === t));
  $$('.lib-item').forEach(x => x.classList.remove('armed'));
  updateHint();
  cv.style.cursor = t === 'select' ? 'default' : 'crosshair';
}

function updateHint() {
  let s;
  if (armed) s = `Click to place ${tpl(armed).label} · Shift-click to place several · Esc to cancel`;
  else if (tool === 'cable') s = cable ? `Linking from ${byId.get(cable.dev)?.name} ${ifName(cable.dev, cable.port)} → click the other device · Esc cancels` : 'Link · click a device, then pick an interface';
  else if (tool === 'note') s = 'Note · click anywhere to drop a note';
  else if (SHAPE_TOOLS.includes(tool)) s = `${tool[0].toUpperCase() + tool.slice(1)} · drag to draw · Shift ${tool === 'line' ? 'snaps to 45°' : 'keeps it square'} · Esc to cancel`;
  else s = 'Select · drag to move · drag empty space to box-select · Space-drag to pan · Ctrl+scroll to zoom';
  $('#hint').textContent = s;
}

cv.addEventListener('pointerdown', e => {
  cv.focus();
  closeMenu();
  const { x: wx, y: wy } = toWorld(...local(e));
  if (e.button === 1 || (e.button === 0 && spaceDown)) {
    gesture = { type: 'pan', sx: e.clientX, sy: e.clientY, vx: view.x, vy: view.y };
    capturePointer(e);
    cv.style.cursor = 'grabbing';
    return;
  }
  if (e.button !== 0) return;

  if (armed) {
    addNode(armed, wx, wy);
    if (!e.shiftKey) setTool('select');
    renderAll();
    return;
  }
  if (SHAPE_TOOLS.includes(tool)) {
    const isLine = tool === 'line';
    const pushed = checkpoint();
    const sh = { id: nextShapeId++, kind: tool, x: snapV(wx), y: snapV(wy), w: 0, h: 0,
      stroke: shapeStyle.stroke, fill: !isLine && shapeStyle.fill, dash: shapeStyle.dash, width: shapeStyle.width, text: '' };
    shapes.push(sh);
    gesture = { type: 'draw', sh, pushed };
    capturePointer(e);
    return;
  }
  const hnd = tool === 'select' ? handleAt(wx, wy) : null;
  if (hnd) {
    checkpoint();
    gesture = { type: 'resize', sh: hnd.sh, k: hnd.k, orig: { ...hnd.sh } };
    capturePointer(e);
    return;
  }
  const hit = hitAt(wx, wy);

  if (tool === 'note') {
    checkpoint();
    const n = { id: nextNoteId++, x: snapV(wx), y: snapV(wy), text: 'Note' };
    notes.push(n);
    setTool('select');
    selectOnly('note', n.id);
    editNote(n);
    dirty();
    return;
  }

  if (tool === 'cable') {
    if (hit?.type !== 'node') { cable = null; updateHint(); return; }
    pickPort(hit.id, e.clientX, e.clientY, port => {
      if (!cable) { cable = { dev: hit.id, port }; updateHint(); return; }
      if (cable.dev === hit.id) { toast('Pick a different device for the other end', 'err'); return; }
      checkpoint();
      const id = lab.add_link_ports(cable.dev, cable.port, hit.id, port);
      if (id < 0) toast('That interface is already in use', 'err');
      cable = null;
      refresh(); renderAll(); dirty(); updateHint();
    });
    return;
  }

  // select tool
  if (hit) {
    const type = hit.type;
    if (e.shiftKey || e.metaKey || e.ctrlKey) toggleSel(type, hit.id);
    else if (!sel[type + 's'].has(hit.id)) selectOnly(type, hit.id);
    if (type === 'link') return;
    const start = [...sel.nodes].map(id => ({ id, x: byId.get(id).x, y: byId.get(id).y }));
    const nstart = [...sel.notes].map(id => { const n = notes.find(n => n.id === id); return { n, x: n.x, y: n.y }; })
      .concat([...sel.shapes].map(id => { const n = shapes.find(x => x.id === id); return { n, x: n.x, y: n.y }; }));
    gesture = { type: 'move', wx, wy, start, nstart, moved: false };
    capturePointer(e);
  } else {
    if (!e.shiftKey) selectOnly(null);
    gesture = { type: 'band', x0: wx, y0: wy, add: e.shiftKey };
    capturePointer(e);
  }
});

cv.addEventListener('pointermove', e => {
  const w = toWorld(...local(e));
  mouse = { sx: local(e)[0], sy: local(e)[1], wx: w.x, wy: w.y, inside: true };
  $('#st-cursor').textContent = `x ${Math.round(w.x)}  y ${Math.round(w.y)}`;
  if (!gesture) {
    const hnd = tool === 'select' && !spaceDown ? handleAt(w.x, w.y) : null;
    if (hnd) { cv.style.cursor = HANDLE_CURSOR[hnd.k]; hover = null; return; }
    const hit = hitAt(w.x, w.y);
    hover = hit ? { type: hit.type, id: hit.id } : null;
    if (tool === 'select' && !spaceDown) cv.style.cursor = hit ? (hit.type === 'link' ? 'pointer' : 'move') : 'default';
    return;
  }
  if (gesture.type === 'pan') {
    view.x = gesture.vx + e.clientX - gesture.sx;
    view.y = gesture.vy + e.clientY - gesture.sy;
  } else if (gesture.type === 'draw') {
    const sh = gesture.sh;
    let dx = snapV(w.x) - sh.x, dy = snapV(w.y) - sh.y;
    if (e.shiftKey) {
      if (sh.kind === 'line') {
        const a = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4), L = Math.hypot(dx, dy);
        dx = Math.round(Math.cos(a) * L); dy = Math.round(Math.sin(a) * L);
      } else { const m = Math.max(Math.abs(dx), Math.abs(dy)); dx = Math.sign(dx || 1) * m; dy = Math.sign(dy || 1) * m; }
    }
    sh.w = dx; sh.h = dy;
  } else if (gesture.type === 'resize') {
    const { sh, k, orig } = gesture, X = snapV(w.x), Y = snapV(w.y);
    if (k === 'p1') { sh.x = X; sh.y = Y; sh.w = orig.x + orig.w - X; sh.h = orig.y + orig.h - Y; }
    else if (k === 'p2') { sh.w = X - orig.x; sh.h = Y - orig.y; }
    else {
      const b = norm(orig);
      let x0 = b.x, y0 = b.y, x1 = b.x + b.w, y1 = b.y + b.h;
      if (k.includes('w')) x0 = X;
      if (k.includes('e')) x1 = X;
      if (k.includes('n')) y0 = Y;
      if (k.includes('s')) y1 = Y;
      sh.x = x0; sh.y = y0; sh.w = x1 - x0; sh.h = y1 - y0;
    }
  } else if (gesture.type === 'move') {
    const dx = w.x - gesture.wx, dy = w.y - gesture.wy;
    if (!gesture.moved && Math.hypot(dx, dy) * view.z < 3) return;
    if (!gesture.moved) checkpoint();
    gesture.moved = true;
    for (const s of gesture.start) lab.move_device(s.id, s.x + dx, s.y + dy);
    for (const s of gesture.nstart) { s.n.x = s.x + dx; s.n.y = s.y + dy; }
    refresh();
  }
});
cv.addEventListener('pointerleave', () => { mouse.inside = false; hover = null; });

cv.addEventListener('pointerup', e => {
  const g = gesture;
  gesture = null;
  if (!g) return;
  if (g.type === 'pan') { cv.style.cursor = spaceDown ? 'grab' : tool === 'select' ? 'default' : 'crosshair'; return; }
  if (g.type === 'draw') {
    const sh = g.sh;
    if (Math.abs(sh.w) < 6 && Math.abs(sh.h) < 6) {
      shapes = shapes.filter(x => x !== sh);
      if (g.pushed) undoStack.pop();
    } else {
      if (sh.kind !== 'line') Object.assign(sh, norm(sh));
      setTool('select');
      selectOnly('shape', sh.id);
      dirty();
    }
    return;
  }
  if (g.type === 'resize') {
    if (g.sh.kind !== 'line') Object.assign(g.sh, norm(g.sh));
    dirty();
    return;
  }
  if (g.type === 'move' && g.moved) {
    for (const s of g.start) { const d = byId.get(s.id); lab.move_device(s.id, snapV(d.x), snapV(d.y)); }
    for (const s of g.nstart) { s.n.x = snapV(s.n.x); s.n.y = snapV(s.n.y); }
    refresh(); dirty();
  }
  if (g.type === 'band') {
    const w = toWorld(...local(e));
    const x0 = Math.min(g.x0, w.x), x1 = Math.max(g.x0, w.x), y0 = Math.min(g.y0, w.y), y1 = Math.max(g.y0, w.y);
    if (x1 - x0 > 3 || y1 - y0 > 3) {
      for (const d of snap.devices) if (d.x >= x0 && d.x <= x1 && d.y >= y0 && d.y <= y1) sel.nodes.add(d.id);
      for (const n of notes) if (n.x >= x0 && n.x <= x1 && n.y >= y0 && n.y <= y1) sel.notes.add(n.id);
      for (const sh of shapes) { const b = norm(sh); if (b.x >= x0 && b.x + b.w <= x1 && b.y >= y0 && b.y + b.h <= y1) sel.shapes.add(sh.id); }
      renderInspector(true); renderTopo(true);
    }
  }
});

cv.addEventListener('dblclick', e => {
  const { x, y } = toWorld(...local(e));
  const hit = hitAt(x, y);
  if (hit?.type === 'node') openConsole(hit.id);
  else if (hit?.type === 'note') editNote(hit.obj);
  else if (hit?.type === 'link') setCapture(hit.id, !hit.obj.capture);
  else if (hit?.type === 'shape') editShapeText(hit.obj);
});

cv.addEventListener('wheel', e => {
  e.preventDefault();
  if (e.ctrlKey || e.metaKey) zoomAt(...local(e), Math.exp(-e.deltaY * (e.deltaMode ? 0.05 : 0.0025)));
  else if (e.deltaMode === 1 || (Math.abs(e.deltaY) >= 50 && !e.deltaX && Number.isInteger(e.deltaY))) {
    zoomAt(...local(e), e.deltaY < 0 ? 1.12 : 1 / 1.12);   // mouse wheel → zoom
  } else { view.x -= e.deltaX; view.y -= e.deltaY; }                 // trackpad → pan
}, { passive: false });

cv.addEventListener('contextmenu', e => {
  e.preventDefault();
  const { x, y } = toWorld(...local(e));
  const hit = hitAt(x, y);
  if (hit && !sel[hit.type + 's'].has(hit.id)) selectOnly(hit.type, hit.id);
  contextMenu(hit, e.clientX, e.clientY, { x, y });
});

// drag from library
cv.addEventListener('dragover', e => { e.preventDefault(); const w = toWorld(...local(e)); mouse = { ...mouse, wx: w.x, wy: w.y, inside: true }; });
cv.addEventListener('drop', e => {
  e.preventDefault();
  const id = e.dataTransfer.getData('text/x-netlab');
  if (!id) return;
  const w = toWorld(...local(e));
  armed = null;
  addNode(id, w.x, w.y);
  setTool('select');
  renderAll();
});

async function editNote(n) {
  const text = await ask('Edit note', n.text, { multiline: true, ok: 'Save' });
  if (text != null) { n.text = text; dirty(); }
}



/* ════════════════════════════ menus ════════════════════════════ */


function pickPort(dev, x, y, done) {
  const d = byId.get(dev);
  const n = portCount(dev);
  const free = [];
  const items = [{ header: `${d.name} — interfaces` }];
  for (let i = 0; i < n; i++) {
    const p = d.ports[i];
    const l = p.link != null ? linkById.get(p.link) : null;
    const peer = l ? peerOf(l, dev) : null;
    if (!l) free.push(i);
    items.push({
      cls: 'port', label: ifName(dev, i), disabled: !!l,
      html: `${esc(ifName(dev, i))} <span class="peer">${l ? `→ ${esc(byId.get(peer.dev)?.name)} ${esc(ifName(peer.dev, peer.port))}` : p.ip ? esc(p.ip) : 'free'}</span>`,
      icon: l ? 'cable' : null,
      run: () => done(i),
    });
  }
  if (!free.length) return toast(`${d.name} has no free interfaces`, 'err');
  if (free.length === 1 && n === 1) return done(free[0]);   // single-NIC host: no question to ask
  openMenu(items, x, y);
}

function contextMenu(hit, x, y, w) {
  const multi = selCount() > 1;
  if (multi) {
    const ids = [...sel.nodes];
    return openMenu([
      { header: `${selCount()} selected` },
      { label: 'Start', icon: 'play', run: () => ids.forEach(startNode), disabled: !ids.length },
      { label: 'Stop', icon: 'stop', run: () => ids.forEach(stopNode), disabled: !ids.length },
      { label: 'Duplicate', icon: 'plus', kbd: 'Ctrl+D', run: duplicateSelection, disabled: !ids.length },
      '-',
      { label: 'Delete', icon: 'trash', kbd: 'Del', danger: true, run: deleteSelection },
    ], x, y);
  }
  if (hit?.type === 'node') {
    const d = hit.obj, st = nodeState(d.id), isExt = d.kind === 'ext';
    return openMenu([
      { header: d.name },
      { label: 'Start', icon: 'play', run: () => startNode(d.id), disabled: st !== 'stopped' },
      { label: 'Stop', icon: 'stop', run: () => stopNode(d.id), disabled: st === 'stopped' },
      ...(isExt ? [] : [{ label: 'Reload', icon: 'reload', run: () => reloadNode(d.id), disabled: st !== 'running' }]),
      { label: 'Console', icon: 'term', kbd: 'dbl-click', run: () => openConsole(d.id) },
      '-',
      ...(isExt ? [{ label: 'Choose image…', icon: 'image', run: () => imageManager(d.id) }] : []),
      { label: 'Rename', icon: 'tag', kbd: 'F2', run: () => focusRename() },
      { label: 'Duplicate', icon: 'plus', kbd: 'Ctrl+D', run: duplicateSelection },
      { label: 'Capture on all links', icon: 'capture', run: () => snap.links.filter(l => l.a === d.id || l.b === d.id).forEach(l => setCapture(l.id, true)), disabled: !d.ports.some(p => p.link != null) },
      '-',
      { label: 'Delete', icon: 'trash', kbd: 'Del', danger: true, run: deleteSelection },
    ], x, y);
  }
  if (hit?.type === 'link') {
    const l = hit.obj;
    return openMenu([
      { header: linkTitle(l) },
      { label: l.capture ? 'Stop capture' : 'Start capture', icon: 'capture', run: () => setCapture(l.id, !l.capture) },
      { label: l.up ? 'Unplug cable' : 'Plug cable in', icon: l.up ? 'unplug' : 'plug', run: () => toggleLink(l.id) },
      '-',
      { label: 'Delete link', icon: 'trash', kbd: 'Del', danger: true, run: deleteSelection },
    ], x, y);
  }
  if (hit?.type === 'shape') {
    const sh = hit.obj;
    return openMenu([
      { header: sh.text || sh.kind[0].toUpperCase() + sh.kind.slice(1) },
      { label: 'Edit label…', icon: 'tag', run: () => editShapeText(sh) },
      { label: 'Bring to front', icon: 'front', run: () => reorderShape(sh, true) },
      { label: 'Send to back', icon: 'back', run: () => reorderShape(sh, false) },
      '-',
      { label: 'Delete shape', icon: 'trash', kbd: 'Del', danger: true, run: deleteSelection },
    ], x, y);
  }
  if (hit?.type === 'note') {
    return openMenu([
      { label: 'Edit text', icon: 'note', run: () => editNote(hit.obj) },
      { label: 'Delete note', icon: 'trash', danger: true, run: deleteSelection },
    ], x, y);
  }
  openMenu([
    { header: 'Add device here' },
    ...TEMPLATES.filter(t => !t.disabled).map(t => ({ label: t.label, art: t.id, run: () => { addNode(t.id, w.x, w.y); renderAll(); } })),
    { label: 'Note', icon: 'note', run: () => { const n = { id: nextNoteId++, x: snapV(w.x), y: snapV(w.y), text: 'Note' }; notes.push(n); editNote(n); dirty(); } },
    '-',
    { label: 'Select all', kbd: 'Ctrl+A', icon: 'select', run: selectAll },
    { label: 'Fit to view', kbd: 'F', icon: 'fit', run: fit },
    '-',
    { label: 'Start all nodes', icon: 'play', run: startAll },
    { label: 'Stop all nodes', icon: 'stop', run: stopAll },
  ], x, y);
}

function selectAll() {
  snap.devices.forEach(d => sel.nodes.add(d.id));
  notes.forEach(n => sel.notes.add(n.id));
  shapes.forEach(sh => sel.shapes.add(sh.id));
  renderInspector(true); renderTopo(true);
}

function focusRename() {
  const el = $('#inspector .name');
  if (el) { el.focus(); el.select(); }
}

/* menubar */
const MENUS = {
  File: () => [
    { label: 'New lab', icon: 'plus', run: async () => { if (await confirmBox('New lab', 'Start a new empty lab? The current topology is replaced (save it first with Ctrl+S if you need it).', 'New lab', true)) { checkpoint(); restore({ name: 'Untitled lab' }); dirty(); } } },
    { label: 'Topology templates…', icon: 'templates', kbd: 'T', run: () => templateMarketplace() },
    { label: 'Save current as template…', icon: 'plus', run: () => templateMarketplace('all', true), disabled: !snap.devices.length },
    '-',
    { label: 'Open lab file…', icon: 'upload', kbd: 'Ctrl+O', run: openFile },
    { label: 'Save lab file', icon: 'download', kbd: 'Ctrl+S', run: saveFile },
    { label: 'Save YAML lab…', icon: 'download', run: saveYaml },
    '-',
    { label: 'Export canvas as PNG', icon: 'image', run: exportPng },
    { label: 'Load demo topology', icon: 'map', run: () => { checkpoint(); restore(DEMO()); fit(); dirty(); } },
  ],
  Edit: () => [
    { label: 'Undo', kbd: 'Ctrl+Z', run: undo, disabled: !undoStack.length },
    { label: 'Redo', kbd: 'Ctrl+Y', run: redo, disabled: !redoStack.length },
    '-',
    { label: 'Select all', icon: 'select', kbd: 'Ctrl+A', run: selectAll },
    { label: 'Duplicate', icon: 'plus', kbd: 'Ctrl+D', run: duplicateSelection, disabled: !sel.nodes.size },
    { label: 'Rename', icon: 'tag', kbd: 'F2', run: focusRename, disabled: sel.nodes.size !== 1 },
    '-',
    { label: 'Delete selection', icon: 'trash', kbd: 'Del', danger: true, run: deleteSelection, disabled: !(selCount()) },
  ],
  View: () => [
    { label: 'Zoom in', icon: 'zoomIn', kbd: '+', run: () => zoomAt(W / 2, H / 2, 1.2) },
    { label: 'Zoom out', icon: 'zoomOut', kbd: '−', run: () => zoomAt(W / 2, H / 2, 1 / 1.2) },
    { label: 'Actual size', kbd: '0', run: () => zoomAt(W / 2, H / 2, 1 / view.z) },
    { label: 'Fit to view', icon: 'fit', kbd: 'F', run: fit },
    '-',
    { label: `${P.labels ? 'Hide' : 'Show'} interface labels`, icon: 'tag', kbd: 'L', run: () => togglePref('labels') },
    { label: `${P.grid ? 'Hide' : 'Show'} grid & snapping`, icon: 'grid', kbd: 'G', run: () => togglePref('grid') },
    { label: `${P.minimap ? 'Hide' : 'Show'} minimap`, icon: 'map', kbd: 'M', run: () => togglePref('minimap') },
    { label: `${P.dockMin ? 'Show' : 'Hide'} bottom panel`, icon: 'dock', kbd: '`', run: toggleDock },
    { label: `${P.theme === 'dark' ? 'Light' : 'Dark'} theme`, icon: P.theme === 'dark' ? 'sun' : 'moon', run: toggleTheme },
  ],
  Lab: () => [
    { label: 'Start all nodes', icon: 'play', run: startAll },
    { label: 'Stop all nodes', icon: 'stop', run: stopAll },
    '-',
    { label: 'Realtime mode', run: () => setMode('realtime'), sub: mode === 'realtime' ? '●' : '' },
    { label: 'Simulation mode', run: () => setMode('sim'), sub: mode === 'sim' ? '●' : '' },
    '-',
    { label: 'Snapshots…', icon: 'download', run: snapshotsModal },
    { label: 'Topology templates…', icon: 'templates', kbd: 'T', run: () => templateMarketplace() },
    { label: 'Images…', icon: 'image', run: () => imageManager() },
    { label: 'Clear event log', icon: 'trash', run: () => { events.length = 0; renderEvents(); renderDockTabs(); } },
  ],
  Help: () => [
    { label: 'Keyboard shortcuts', icon: 'info', kbd: '?', run: shortcuts },
    { label: 'About netlab', icon: 'chip', run: about },
  ],
};
for (const name of Object.keys(MENUS)) {
  const b = h(`<button role="menuitem">${name}</button>`);
  const open = () => {
    $$('#menubar > button').forEach(x => x.classList.toggle('open', x === b));
    const r = b.getBoundingClientRect();
    openMenu(MENUS[name](), r.left, r.bottom + 4);
    b.classList.add('open');
  };
  b.onclick = () => (b.classList.contains('open') ? closeMenu() : open());
  b.onmouseenter = () => { if (!ctxEl.hidden && $('#menubar > button.open') && !b.classList.contains('open')) open(); };
  $('#menubar').append(b);
}

/* ════════════════════════════ file ops ════════════════════════════ */

function saveYaml() {
  const name = (labName || 'lab').replace(/[^\w.-]+/g, '_');
  download(new Blob([emitLab(serialize())], { type: 'application/yaml' }), `${name}.netlab.yaml`);
}
async function saveFile() {
  const name = (labName || 'lab').replace(/[^\w.-]+/g, '_');
  const data = serialize();
  // IOS configs travel with the lab file (NVRAM, base64); uids do not — they are
  // transient IndexedDB keys (F-20) and import re-mints + re-keys them.
  for (const n of data.nodes) {
    if (n.tpl === 'ios' && n.uid) {
      const rec = await nvram.get(n.uid);
      if (rec) n.nvram = b64(rec.data);
    }
    delete n.uid;
  }
  download(new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' }), `${name}.netlab.json`);
}
async function openFile() {
  const f = await pickFile('.json,.netlab,.yaml,.yml');
  if (!f) return;
  try {
    const text = await f.text();
    const data = /\.ya?ml$/i.test(f.name) ? parseLab(text) : JSON.parse(text);
    if (data.format !== 'netlab') throw new Error('not a netlab lab file');
    checkpoint();
    for (const n of data.nodes ?? []) {
      if (!n.nvram) continue;
      n.uid ||= newUid();
      await nvram.put(n.uid, unb64(n.nvram));
      delete n.nvram;
    }
    restore(data); fit(); dirty();
    toast(`Opened ${f.name}`, 'ok');
  } catch (e) { toast(`Could not open ${f.name}: ${e.message}`, 'err'); }
}
async function snapshotsModal() {
  const box = h(`<div style="display:flex;flex-direction:column;gap:10px">
    <p class="muted" style="margin:0">A snapshot copies the topology, IOS NVRAM, and each VM disk. Restoring one stops every running VM. Undo does the same.</p>
    <div class="row"><input class="field" data-name placeholder="Snapshot name" spellcheck="false" /><button class="btn sm primary" data-save>Save</button></div>
    <div data-list></div>
  </div>`);
  const m = modal('Lab snapshots', box);
  const draw = async () => {
    const list = await snapshots.list();
    const el = $('[data-list]', box);
    el.replaceChildren();
    if (!list.length) { el.append(h('<div class="empty-note">No snapshots yet.</div>')); return; }
    for (const s of list) {
      const when = new Date(s.at).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
      const row = h(`<div class="row" style="margin-top:6px"><span style="flex:1">${esc(s.name)} <span class="muted">${esc(when)}</span></span>
        <button class="btn sm" data-r>Restore</button><button class="btn sm danger" data-d>Delete</button></div>`);
      $('[data-r]', row).onclick = async () => {
        const labData = await snapshots.load(s.id);
        if (!labData) return toast('That snapshot is missing', 'err');
        checkpoint();
        restore(labData); fit(); dirty();
        toast('Snapshot restored — running VMs were stopped');
        m.close();
      };
      $('[data-d]', row).onclick = async () => { await snapshots.remove(s.id); draw(); };
      el.append(row);
    }
  };
  $('[data-save]', box).onclick = async () => {
    const name = $('[data-name]', box).value.trim() || 'snapshot';
    await snapshots.save(name, serialize());
    $('[data-name]', box).value = '';
    toast(`Saved snapshot ${name}`, 'ok');
    draw();
  };
  $('[data-name]', box).onkeydown = e => { if (e.key === 'Enter') $('[data-save]', box).click(); };
  draw();
}
function exportPng() {
  cv.toBlob(b => download(b, `${(labName || 'lab').replace(/[^\w.-]+/g, '_')}.png`));
}

/* ════════════════════════════ modals ════════════════════════════ */


function templateMarketplace(initialTab = 'all', openSave = false) {
  let activeLevel = initialTab;
  let activeCat = 'all';
  let query = '';
  let showSave = openSave && snap.devices.length > 0;

  const body = h(`<div style="display:flex;flex-direction:column;gap:14px">
    <div class="market-bar">
      <div class="market-tabs" role="tablist" aria-label="Difficulty filter"></div>
      <select class="field" data-cat style="width:auto;min-width:155px;padding:5px 10px;font-size:12px" aria-label="Category filter">
        <option value="all">All categories</option>
      </select>
      <span class="spacer" style="flex:1"></span>
      <label class="search">
        ${ico('search')}
        <input data-q placeholder="Search topologies, protocols, tags…" spellcheck="false" />
      </label>
      <button class="btn sm" data-toggle-save ${snap.devices.length ? '' : 'disabled title="Add devices to the canvas first"'}>${ico('plus')}Save current lab</button>
    </div>
    <div class="market-save-box" data-save-box hidden>
      <h4><span>Publish current canvas to My Templates</span><button class="icon-btn sm" data-close-save title="Cancel">${UI.x}</button></h4>
      <div class="market-save-grid">
        <input class="field" data-s-title placeholder="Template title" value="${esc(labName)}" spellcheck="false" />
        <select class="field" data-s-level>
          <option value="beginner">Beginner</option>
          <option value="intermediate" selected>Intermediate</option>
          <option value="advanced">Advanced</option>
          <option value="expert">Expert</option>
        </select>
        <input class="field" data-s-cat placeholder="Category (e.g. Routing, WAN, Security)" value="Custom Lab" spellcheck="false" />
      </div>
      <div class="market-save-grid" style="grid-template-columns: 2fr 1fr">
        <input class="field" data-s-desc placeholder="Short summary of what this topology demonstrates…" spellcheck="false" />
        <input class="field" data-s-tags placeholder="Tags (comma-separated, e.g. OSPF, Multi-NIC)" value="Custom, ${snap.devices.length} nodes" spellcheck="false" />
      </div>
      <div class="row" style="justify-content:flex-end">
        <button class="btn sm" data-close-save2>Cancel</button>
        <button class="btn sm primary" data-do-save>${ico('download')}Save to Marketplace</button>
      </div>
    </div>
    <div class="market-grid" data-grid></div>
  </div>`);

  const m = modal('Topology Template Marketplace', body, '', 'market');
  const tabsEl = $('.market-tabs', body);
  const catSel = $('[data-cat]', body);
  const gridEl = $('[data-grid]', body);
  const saveBox = $('[data-save-box]', body);
  const qInput = $('[data-q]', body);

  catSel.onchange = () => { activeCat = catSel.value; draw(); };

  const syncSaveBox = () => { saveBox.hidden = !showSave; };
  syncSaveBox();

  $('[data-toggle-save]', body).onclick = () => { showSave = !showSave; syncSaveBox(); if (showSave) $('[data-s-title]', body).focus(); };
  $('[data-close-save]', body).onclick = () => { showSave = false; syncSaveBox(); };
  $('[data-close-save2]', body).onclick = () => { showSave = false; syncSaveBox(); };

  $('[data-do-save]', body).onclick = () => {
    const title = $('[data-s-title]', body).value.trim() || labName || 'Custom Topology';
    const level = $('[data-s-level]', body).value;
    const category = $('[data-s-cat]', body).value.trim() || 'Custom';
    const summary = $('[data-s-desc]', body).value.trim() || `User-created topology with ${snap.devices.length} devices and ${snap.links.length} links.`;
    const tags = $('[data-s-tags]', body).value.split(',').map(s => s.trim()).filter(Boolean);
    const data = serialize();
    data.name = title;
    for (const n of data.nodes) delete n.uid;   // F-20: a template must not share the lab's store keys
    userTemplates.add({
      id: 'user-' + Date.now().toString(36),
      title, level, category, summary,
      tags: tags.length ? tags : ['Custom'],
      objectives: ['Saved from canvas — load to restore all nodes, links, IPs, routes, and notes.'],
      data,
    });
    showSave = false;
    syncSaveBox();
    toast(`Saved "${title}" to Marketplace`, 'ok');
    draw();
  };

  const loadTpl = t => {
    checkpoint();                                  // F-29: template load is undoable
    const data = t.build();
    for (const n of data.nodes ?? []) delete n.uid;   // F-20: stored uids are transient — mint fresh on load
    restore(data);
    fit();
    dirty();
    m.close();
    toast(`Loaded "${t.title}" (${LEVEL_META[t.level]?.label ?? t.level})`, 'ok');
  };

  const exportTpl = t => {
    const data = t.build();
    for (const n of data.nodes ?? []) delete n.uid;   // F-20: transient store keys never ship
    const fname = (t.id || t.title || 'template').toLowerCase().replace(/[^\w.-]+/g, '_') + '.netlab.json';
    download(new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' }), fname);
  };

  const draw = () => {
    const list = allTemplates();
    const cats = [...new Set(list.map(x => x.category).filter(Boolean))];
    catSel.innerHTML = `<option value="all">All categories (${cats.length})</option>` +
      cats.map(c => `<option value="${esc(c)}" ${activeCat === c ? 'selected' : ''}>${esc(c)}</option>`).join('');

    const counts = {
      all: list.length,
      beginner: list.filter(x => x.level === 'beginner').length,
      intermediate: list.filter(x => x.level === 'intermediate').length,
      advanced: list.filter(x => x.level === 'advanced').length,
      expert: list.filter(x => x.level === 'expert').length,
      custom: list.filter(x => x.custom).length,
    };

    const tabDefs = [
      ['all', 'All'],
      ['beginner', 'Beginner'],
      ['intermediate', 'Intermediate'],
      ['advanced', 'Advanced'],
      ['expert', 'Expert'],
      ...(counts.custom ? [['custom', 'My Templates']] : []),
    ];

    tabsEl.innerHTML = '';
    for (const [id, label] of tabDefs) {
      const b = h(`<button class="market-tab ${activeLevel === id ? 'on' : ''}" role="tab">
        <span>${esc(label)}</span><span class="cnt">${counts[id] ?? 0}</span>
      </button>`);
      b.onclick = () => { activeLevel = id; draw(); };
      tabsEl.append(b);
    }

    const q = query.trim().toLowerCase();
    const filtered = list.filter(t => {
      if (activeLevel === 'custom' && !t.custom) return false;
      if (activeLevel !== 'all' && activeLevel !== 'custom' && t.level !== activeLevel) return false;
      if (activeCat !== 'all' && t.category !== activeCat) return false;
      if (!q) return true;
      const hay = `${t.title} ${t.category} ${t.level} ${t.summary} ${(t.tags ?? []).join(' ')} ${(t.objectives ?? []).join(' ')}`.toLowerCase();
      return hay.includes(q);
    });

    gridEl.innerHTML = '';
    if (!filtered.length) {
      gridEl.append(h(`<div class="pane-empty" style="grid-column:1/-1;padding:36px"><div><b>No matching topology templates</b><span>Try clearing the search filter or switching difficulty tabs.</span></div></div>`));
      return;
    }

    for (const t of filtered) {
      const data = t.build();
      const lv = LEVEL_META[t.custom ? 'custom' : t.level] ?? LEVEL_META.beginner;
      const lvBadge = LEVEL_META[t.level] ?? lv;
      const nNodes = data.nodes?.length ?? 0;
      const nLinks = data.links?.length ?? 0;
      const svg = renderTemplatePreviewSVG(data);

      const card = h(`<article class="tpl-card">
        <div class="tpl-thumb">
          ${svg}
          <span class="tpl-cat">${esc(t.category || 'Topology')}</span>
          <span class="pill ${lvBadge.color} tpl-lvl">${esc(lvBadge.label)}${t.custom ? ' · Custom' : ''}</span>
          <span class="tpl-stats">${nNodes} device${nNodes === 1 ? '' : 's'} · ${nLinks} link${nLinks === 1 ? '' : 's'}</span>
        </div>
        <div class="tpl-body">
          <h3 class="tpl-title">${esc(t.title)}</h3>
          <p class="tpl-desc">${esc(t.summary)}</p>
          <div class="tpl-tags">${(t.tags ?? []).map(tag => `<button class="tpl-tag" data-tag="${esc(tag)}" title="Filter by ${esc(tag)}">${esc(tag)}</button>`).join('')}</div>
          ${t.objectives?.length ? `<details class="tpl-obj">
            <summary>Lab objectives (${t.objectives.length} steps)</summary>
            <ol>${t.objectives.map(o => `<li>${esc(o)}</li>`).join('')}</ol>
          </details>` : ''}
        </div>
        <div class="tpl-foot">
          <button class="btn sm primary" data-load>${ico('play')}Load topology</button>
          <button class="btn sm" data-export title="Download .netlab.json">${ico('download')}JSON</button>
          <span class="spacer"></span>
          ${t.custom ? `<button class="icon-btn sm" data-del title="Delete custom template">${UI.trash}</button>` : ''}
        </div>
      </article>`);

      $('[data-load]', card).onclick = () => loadTpl(t);
      $('[data-export]', card).onclick = () => exportTpl(t);
      $$('[data-tag]', card).forEach(tb => tb.onclick = () => {
        query = tb.dataset.tag;
        qInput.value = query;
        draw();
      });
      const delBtn = $('[data-del]', card);
      if (delBtn) delBtn.onclick = () => {
        userTemplates.remove(t.id);
        if (activeLevel === 'custom' && !userTemplates.list().length) activeLevel = 'all';
        toast(`Removed "${t.title}"`, 'ok');
        draw();
      };
      gridEl.append(card);
    }
    hydrateIcons(gridEl);
  };

  qInput.oninput = () => { query = qInput.value; draw(); };
  draw();
  setTimeout(() => qInput.focus(), 0);
}

const KIND_LABEL = { ios: 'IOS', iso: 'ISO', disk: 'Disk' };
function imageManager(forDev = null) {
  return new Promise(resolve => {
    const want = forDev == null ? null : isVM(forDev) ? ['iso', 'disk'] : ['ios'];
    const ros = forDev != null && isRos(forDev);
    const csw = forDev != null && isIOSSwitch(forDev);
    const help = want == null
      ? 'Cisco IOS images (<span class="mono">c3725</span>/<span class="mono">c7200</span> <span class="mono">.bin</span>), x86 install ISOs and raw disk images (<span class="mono">.iso</span>, <span class="mono">.img</span>).'
      : want[0] === 'ios'
        ? csw
          ? 'Use a Cisco <b>c3725</b> IOS image (recommended for 16-port <span class="mono">NM-16ESW</span> hardware VLAN/STP switching, e.g. <span class="mono">c3725-adventerprisek9-mz.124-15.T14.bin</span>) or a <b>c7200</b> IOS image (uses <span class="mono">PA-8E</span> IEEE 802.1D / IRB bridge groups).'
          : 'Use a Cisco 7200 or 3725 IOS image you are licensed to run, e.g. <span class="mono">c7200-adventerprisek9-mz.152-4.S5.bin</span>.'
        : ros
          ? 'Use the <b>RouterOS v6 x86</b> install ISO (e.g. <span class="mono">mikrotik-6.49.18.iso</span> from mikrotik.com) or a raw disk with v6 installed. CHR and RouterOS v7 are 64-bit and cannot run in this 32-bit emulator.'
          : 'Any 32-bit x86 install ISO or raw disk image (Linux + Open vSwitch / iproute2 bridge, Alpine, TinyCore, etc.). Convert qcow2/VMDK/VDI first: <span class="mono">qemu-img convert -O raw in.qcow2 out.img</span>.';
    const accept = want == null ? '.bin,.image,.iso,.img,.raw' : want[0] === 'ios' ? '.bin,.image' : '.iso,.img,.raw';
    const body = h(`<div>
      <div class="callout">${ico('info')}<div>Images stay in this browser (IndexedDB) and are never uploaded anywhere. ${help}</div></div>
      <ul class="imglist" hidden></ul>
      <div class="drop-zone" tabindex="0">${ico('upload')}<div><b>Drop an image here</b> or click to browse</div><div class="muted">${esc(accept.replaceAll(',', ' '))}</div></div>
      ${forDev == null ? '<div class="orphans" hidden><h4 style="margin:16px 0 8px;font-size:11px;letter-spacing:.06em;text-transform:uppercase;color:var(--muted)">Storage not used by this lab</h4><ul class="imglist"></ul></div>' : ''}
    </div>`);
    const m = modal(forDev != null ? `Choose an image for ${byId.get(forDev)?.name}` : 'Images', body);
    let chosen = null;
    m._onclose = () => resolve(chosen);
    const list = $('.imglist', body);
    const use = async name => {
      const node = forDev == null ? null : meta.get(forDev);
      // an installed VM boots its own disk, so a new image only takes effect on a fresh disk
      if (node && isVM(forDev) && node.diskSaved && name !== node.image) {
        if (nodeState(forDev) !== 'stopped') return toast(`${byId.get(forDev)?.name}: stop the VM before changing its image`, 'err');
        const ok = await confirmBox('Change image', `${byId.get(forDev)?.name} has an installed disk, which boots before any image. Using ${name} resets the disk: everything installed or configured inside the VM is lost.`, 'Reset disk', true);
        if (!ok) return;
        await disks.remove(node.uid);
        node.diskSaved = null; node.diskSize = 0;
      }
      if (node) node.image = name;
      chosen = name; dirty(); renderInspector(true); m.close();
    };
    const draw = async () => {
      const all = (await images.list()).filter(im => !want || want.includes(im.kind));
      list.hidden = !all.length;
      list.innerHTML = '';
      for (const im of all) {
        const li = h(`<li><span class="nm" title="${esc(im.name)}"><span class="pill" style="margin-right:8px">${KIND_LABEL[im.kind] ?? im.kind}</span>${esc(im.name)}</span><span class="muted mono">${(im.size / 1e6).toFixed(1)} MB</span>
          <span class="row">${forDev != null ? '<button class="btn sm primary" data-use>Use</button>' : ''}<button class="icon-btn sm" data-del title="Delete">${UI.trash}</button></span></li>`);
        $('[data-del]', li).onclick = async () => { await images.remove(im.name); draw(); };
        const u = $('[data-use]', li);
        if (u) u.onclick = () => use(im.name);
        list.append(li);
      }
    };
    // VM disks / IOS configs left behind by deleted labs (each disk is 64 MB+)
    const drawOrphans = async () => {
      const box = $('.orphans', body);
      if (!box) return;
      const live = new Set([...meta.values()].map(x => x.uid));
      const rows = [
        ...(await disks.list()).filter(x => !live.has(x.uid)).map(x => ({ ...x, what: 'VM disk', del: () => disks.remove(x.uid) })),
        ...(await nvram.list()).filter(x => !live.has(x.uid)).map(x => ({ ...x, what: 'IOS config', del: () => nvram.remove(x.uid) })),
      ];
      box.hidden = !rows.length;
      const ul = $('ul', box);
      ul.innerHTML = '';
      for (const r of rows) {
        const li = h(`<li><span class="nm"><span class="pill" style="margin-right:8px">${r.what}</span>${r.saved ? esc(new Date(r.saved).toLocaleString()) : ''}</span><span class="muted mono">${(r.size / 1e6).toFixed(1)} MB</span>
          <span class="row"><button class="icon-btn sm" title="Delete">${UI.trash}</button></span></li>`);
        $('button', li).onclick = async () => { await r.del(); drawOrphans(); };
        ul.append(li);
      }
    };
    drawOrphans();
    const add = async f => {
      if (!f) return;
      const s = await sniffImage(f);
      if (s.error) return toast(s.error, 'err');
      if (want && !want.includes(s.kind)) {
        return toast(want[0] === 'ios' ? `${f.name} is not a Cisco IOS image (expected an ELF .bin)` : `${f.name} is a Cisco IOS image — add it to a Cisco 7200 node instead`, 'err');
      }
      if (s.warn) toast(s.warn, 'err');
      if (s.kind === 'disk' && f.size > 512 * 1048576) toast(`${f.name} is ${(f.size / 1048576).toFixed(0)} MB — each node copies it and every disk save writes all of it; smaller images work much better.`, 'err');
      try { await images.put(f, s.kind); }
      catch (e) { return toast(e?.name === 'QuotaExceededError' ? 'Browser storage is full — delete unused images first' : `Could not store ${f.name}: ${e?.message ?? e}`, 'err'); }
      toast(`Stored ${f.name} (${KIND_LABEL[s.kind]})`, 'ok');
      if (forDev != null) return use(f.name);
      draw();
    };
    const dz = $('.drop-zone', body);
    dz.onclick = async () => add(await pickFile(accept));
    dz.onkeydown = e => { if (e.key === 'Enter') dz.click(); };
    dz.ondragover = e => { e.preventDefault(); dz.classList.add('over'); };
    dz.ondragleave = () => dz.classList.remove('over');
    dz.ondrop = e => { e.preventDefault(); dz.classList.remove('over'); add(e.dataTransfer.files[0]); };
    draw();
  });
}

function shortcuts() {
  const rows = [
    ['V / C / N', 'Select, Link, Note tools'], ['T', 'Topology template marketplace'], ['Esc', 'Cancel / clear selection'],
    ['Del / Backspace', 'Delete selection'], ['Ctrl+A', 'Select all'], ['Ctrl+D', 'Duplicate'], ['F2', 'Rename'],
    ['Space + drag', 'Pan (or middle mouse)'], ['Ctrl + scroll', 'Zoom at cursor'], ['+ / − / 0 / F', 'Zoom in, out, 100%, fit'],
    ['L / G / M', 'Labels, grid, minimap'], ['`', 'Toggle bottom panel'],
    ['Shift+Space', 'Simulation play / pause'], ['.', 'Step one tick'],
    ['Ctrl+S / Ctrl+O', 'Save / open lab file'], ['Ctrl+Z / Ctrl+Y', 'Undo / redo (stops running VMs)'],
    ['Double-click node', 'Open console'], ['Double-click link', 'Toggle capture'],
  ];
  modal('Keyboard shortcuts', `<div class="shortcuts">${rows.map(([k, v]) => `<kbd>${esc(k)}</kbd><span>${esc(v)}</span>`).join('')}</div>`);
}
function about() {
  modal('About netlab', `<p>netlab is a network lab that runs entirely in your browser — the switching/routing fabric is Rust compiled to WebAssembly,
    and real Cisco IOS boots inside Dynamips compiled to WebAssembly. Nothing leaves this machine.</p>
    <p>Marchel Maulana Fahrezi · <a href="https://github.com/Alg0rix/netlab" target="_blank" rel="noopener">github.com/Alg0rix/netlab</a></p>
    <p class="muted">Cross-origin isolated: ${crossOriginIsolated ? 'yes — IOS nodes available' : 'no — IOS nodes need COOP/COEP headers'}</p>`);
}

/* ════════════════════════════ library ════════════════════════════ */

function renderLibrary() {
  const q = $('#lib-filter').value.trim().toLowerCase();
  const list = $('#lib-list');
  list.innerHTML = '';
  let group = null;
  for (const t of TEMPLATES) {
    if (q && !`${t.label} ${t.sub} ${t.group}`.toLowerCase().includes(q)) continue;
    if (t.group !== group) { group = t.group; list.append(h(`<div class="lib-group">${esc(group)}</div>`)); }
    const el = h(`<div class="lib-item ${t.disabled ? 'disabled' : ''} ${armed === t.id ? 'armed' : ''}" draggable="${!t.disabled}" tabindex="0" title="${esc(t.sub)}">
      <div class="art">${iconSVG(t.id)}</div><div><b>${esc(t.label)}</b><small>${esc(t.sub)}</small></div></div>`);
    if (!t.disabled) {
      el.addEventListener('dragstart', e => { e.dataTransfer.setData('text/x-netlab', t.id); e.dataTransfer.effectAllowed = 'copy'; });
      const arm = () => {
        const was = armed === t.id;
        setTool('select');
        if (!was) { armed = t.id; el.classList.add('armed'); cv.style.cursor = 'crosshair'; }
        updateHint();
      };
      el.onclick = arm;
      el.onkeydown = e => { if (e.key === 'Enter') { addNode(t.id, (W / 2 - view.x) / view.z, (H / 2 - view.y) / view.z); renderAll(); } };
    }
    list.append(el);
  }
}
$('#lib-filter').addEventListener('input', renderLibrary);

/* ════════════════════════════ topology list ════════════════════════════ */

let topoSig = '';
function renderTopo(force) {
  const sig = JSON.stringify([snap.devices.map(d => [d.id, d.name, nodeState(d.id), d.ports.find(p => p.ip)?.ip]), [...sel.nodes]]);
  if (!force && sig === topoSig) return;
  topoSig = sig;
  $('#topo-count').textContent = snap.devices.length ? `${snap.devices.filter(d => nodeState(d.id) === 'running').length}/${snap.devices.length} up` : '';
  const list = $('#topo-list');
  list.innerHTML = '';
  if (!snap.devices.length) { list.append(h('<div class="empty-note" style="padding:4px 8px 6px">No devices yet</div>')); return; }
  for (const d of [...snap.devices].sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }))) {
    const st = nodeState(d.id);
    const row = h(`<div class="topo-row ${sel.nodes.has(d.id) ? 'sel' : ''}" title="${st}">
      <span class="dot ${st}"></span><span class="art">${iconSVG(tplOf(d.id))}</span>
      <span>${esc(d.name)}</span><span class="ip">${esc(d.ports.find(p => p.ip)?.ip ?? '')}</span></div>`);
    row.onclick = e => { e.shiftKey ? toggleSel('node', d.id) : selectOnly('node', d.id); };
    row.ondblclick = () => openConsole(d.id);
    row.oncontextmenu = e => { e.preventDefault(); selectOnly('node', d.id); contextMenu({ type: 'node', id: d.id, obj: d }, e.clientX, e.clientY); };
    list.append(row);
  }
}
$('[data-collapse="topo"]').onclick = () => $('#topo').classList.toggle('collapsed');

/* ════════════════════════════ inspector ════════════════════════════ */

let inspSig = '';
function renderInspectorSoon() {
  if ($('#inspector').contains(document.activeElement) && document.activeElement.matches('input, select, textarea')) return;
  renderInspector(false);
}

function renderInspector(force) {
  const el = $('#inspector');
  const nN = sel.nodes.size, nL = sel.links.size, nT = sel.notes.size + sel.shapes.size;
  const oneShape = sel.shapes.size === 1 && selCount() === 1 ? shapes.find(x => sel.shapes.has(x.id)) : null;
  let sig;
  if (nN === 1 && !nL && !nT) { const id = [...sel.nodes][0]; sig = JSON.stringify(['n', byId.get(id), nodeState(id), meta.get(id), snap.links.filter(l => l.a === id || l.b === id).map(l => [l.id, l.up])]); }
  else if (nL === 1 && !nN && !nT) { const l = linkById.get([...sel.links][0]); sig = JSON.stringify(['l', l && { ...l, frames: l.frames.length }, l && nodeState(l.a), l && nodeState(l.b)]); }
  else if (oneShape) sig = JSON.stringify(['s', oneShape]);
  else sig = JSON.stringify(['o', nN, nL, nT, snap.devices.length, snap.links.length, snap.devices.map(d => nodeState(d.id))]);
  if (!force && sig === inspSig) return;
  inspSig = sig;
  el.innerHTML = '';
  if (nN === 1 && !nL && !nT) el.append(nodeInspector([...sel.nodes][0]));
  else if (nL === 1 && !nN && !nT) el.append(linkInspector([...sel.links][0]));
  else if (oneShape) el.append(shapeInspector(oneShape));
  else if (nN + nL + nT > 1) el.append(multiInspector());
  else el.append(overviewInspector());
  hydrateIcons(el);
}

function statusPill(st) {
  const cls = st === 'running' ? 'ok' : st === 'booting' ? 'warn' : 'bad';
  return `<span class="pill ${cls}"><span class="dot ${st}"></span>${st[0].toUpperCase() + st.slice(1)}</span>`;
}

function nodeInspector(id) {
  const d = byId.get(id);
  const t = tpl(tplOf(id));
  const st = nodeState(id);
  const isExt = d.kind === 'ext', isHost = d.kind === 'host', isSw = d.kind === 'switch';
  const root = h(`<div>
    <div class="insp-head"><div class="art">${iconSVG(t.id)}</div>
      <div><input class="name" value="${esc(d.name)}" spellcheck="false" aria-label="Node name" />
      <div class="sub">${esc(t.label)} ${statusPill(st)}</div></div></div>
    <div class="insp-actions">
      ${st === 'stopped' ? `<button class="btn sm primary" data-a="start">${ico('play')}Start</button>` : `<button class="btn sm" data-a="stop">${ico('stop')}Stop</button>`}
      ${!isExt ? `<button class="btn sm" data-a="reload" ${st !== 'running' ? 'disabled' : ''}>${ico('reload')}Reload</button>` : ''}
      <button class="btn sm" data-a="console">${ico('term')}Console</button>
      <span class="spacer"></span>
      <button class="icon-btn sm" data-a="delete" title="Delete">${UI.trash}</button>
    </div>
  </div>`);
  const name = $('.name', root);
  name.onkeydown = e => { if (e.key === 'Enter') name.blur(); if (e.key === 'Escape') { name.value = d.name; name.blur(); } };
  name.onchange = () => rename(id, name.value);
  $$('[data-a]', root).forEach(b => b.onclick = () => ({
    start: () => startNode(id), stop: () => stopNode(id), reload: () => reloadNode(id),
    console: () => openConsole(id), delete: () => { selectOnly('node', id); deleteSelection(); },
  })[b.dataset.a]());

  if (isVM(id)) root.append(vmSettings(id, d, st));
  if (isExt && !isVM(id)) {
    const m = meta.get(id);
    const img = m.image;
    const nPorts = portCount(id);
    const csw = isIOSSwitch(id);
    const plat = m.iosPlatform || (img && /3725/.test(img) ? '3725' : img && /3745/.test(img) ? '3745' : img && /2691/.test(img) ? '2691' : csw ? '3725 / 7200' : '7200');
    const adapters = csw
      ? (plat === '7200' ? `${nPorts <= 8 ? '1×' : '2×'} PA-8E (${nPorts}× Ethernet)` : `NM-16ESW (${nPorts}× FastEthernet L2/L3)`)
      : (plat !== '7200' && plat !== '3725 / 7200'
        ? `GT96100-FE${nPorts > 2 ? ` + NM-16ESW (${nPorts} ports)` : ''}`
        : `${nPorts >= 8 ? 'C7200-IO-2FE' : 'C7200-IO-FE'}${nPorts > 1 ? ` + ${nPorts >= 8 ? 6 : nPorts - 1}× PA-FE-TX` : ''}`);
    const s = h(`<div class="sect"><h4>Image</h4>
      <div class="row"><span class="mono" style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${img ? esc(img) : '<span class="muted">none selected</span>'}</span>
      <button class="btn sm" data-img>${ico('image')}${img ? 'Change' : 'Choose'}</button></div>
      <dl class="kv" style="margin-top:10px"><dt>Platform</dt><dd>c${esc(plat)}${plat === '7200' ? ' · NPE-400' : ''}</dd><dt>RAM</dt><dd>256 MB</dd>
      <dt>Adapters</dt><dd>${esc(adapters)}</dd></dl>
      ${csw ? `<p class="muted" style="margin:8px 0 0;font-size:12px"><b>NM-16ESW (c3725)</b>: <span class="mono">vlan database</span> → <span class="mono">vlan 10</span>, <span class="mono">interface fa1/0</span> → <span class="mono">switchport mode access</span> / <span class="mono">trunk</span>, <span class="mono">spanning-tree vlan 1</span>.<br><b>PA-8E (c7200)</b>: <span class="mono">bridge irb</span>, <span class="mono">bridge 1 protocol ieee</span>, <span class="mono">interface e1/0</span> → <span class="mono">bridge-group 1</span>.</p>` : ''}
      ${crossOriginIsolated ? '' : '<p class="muted" style="margin-top:8px">This page is not cross-origin isolated, so IOS cannot boot here. Serve with COOP/COEP (<span class="mono">make serve</span>).</p>'}</div>`);
    $('[data-img]', s).onclick = () => imageManager(id);
    root.append(s);

    const when = m.nvSaved ? new Date(m.nvSaved).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }) : null;
    const cfg = h(`<div class="sect"><h4>Startup config</h4>
      <p style="margin:0 0 8px">${when
        ? `<span class="pill ok"><span class="dot running"></span>Saved in this browser</span> <span class="muted" style="font-size:12px">${esc(when)}</span>`
        : '<span class="muted">Nothing saved yet. Run <span class="mono">write memory</span> in the console — NVRAM is kept in this browser and restored on the next start.</span>'}</p>
      ${when ? `<button class="btn sm danger" data-erase ${st !== 'stopped' ? 'disabled title="Stop the node first"' : ''}>${ico('trash')}Erase NVRAM</button>` : ''}</div>`);
    const er = $('[data-erase]', cfg);
    if (er) er.onclick = async () => {
      if (!(await confirmBox('Erase NVRAM', `Delete the saved startup-config of ${d.name}? It boots with a blank config next time.`, 'Erase', true))) return;
      await nvram.remove(m.uid);
      m.nvSaved = null;
      toast(`${d.name}: NVRAM erased`, 'ok');
      renderInspector(true);
    };
    root.append(cfg);
  }

  root.append(startupSection(id, d, st));

  // interfaces
  const n = portCount(id);
  const maxP = Math.min(d.ports.length, t.maxPorts ?? d.ports.length);
  const portOpts = (isSw || isIOSSwitch(id)) ? [4, 8, 12, 16] : isIOS(id) ? [1, 2, 3, 4, 5, 6, 7, 8] : [1, 2, 3, 4, 6, 8];
  const validOpts = [...new Set([...portOpts.filter(v => v <= maxP), n])].sort((a, b) => a - b);
  const lockPorts = isExt && st !== 'stopped';
  const ifs = h(`<div class="sect"><h4>Interfaces<span class="spacer"></span>
    <select class="field" data-ports style="width:auto;padding:2px 6px;height:22px;font-size:11px;text-transform:none;letter-spacing:0" ${lockPorts ? 'disabled title="Stop the node to change NIC count"' : ''}>
      ${validOpts.map(v => `<option value="${v}" ${v === n ? 'selected' : ''}>${v} port${v === 1 ? '' : 's'}</option>`).join('')}
    </select></h4><table class="iftable"><thead><tr><th>Name</th><th>${isSw ? 'Connected to' : 'IPv4 / prefix'}</th>${isSw ? '' : '<th>Peer</th>'}</tr></thead><tbody></tbody></table>
    ${isExt ? `<p class="muted" style="margin:8px 0 0">Addresses &amp; VLANs are configured inside the ${isVM(id) ? 'guest (console)' : 'IOS console'}.</p>` : ''}</div>`);
  $('[data-ports]', ifs).onchange = e => setNodePorts(id, +e.target.value);
  const tb = $('tbody', ifs);
  for (let i = 0; i < n; i++) {
    const p = d.ports[i];
    const l = p.link != null ? linkById.get(p.link) : null;
    const peer = l ? peerOf(l, id) : null;
    const peerTxt = l ? `${byId.get(peer.dev)?.name} ${ifName(peer.dev, peer.port)}` : '—';
    const dotc = !l ? 'idle' : l.up ? 'running' : 'stopped';
    const tr = h(`<tr><td class="ifn"><span class="dot ${dotc}"></span>${esc(ifName(id, i))}</td>
      ${isSw ? `<td class="peer" style="max-width:none">${esc(peerTxt)}</td>` :
        isExt ? `<td class="mono" style="color:var(--muted)">${isVM(id) ? 'in guest' : 'via IOS'}</td><td class="peer" title="${esc(peerTxt)}">${esc(peerTxt)}</td>` :
        `<td><input class="field" value="${p.ip ? `${p.ip}/${maskLen(p.mask)}` : ''}" placeholder="unassigned" spellcheck="false" /></td><td class="peer" title="${esc(peerTxt)}">${esc(peerTxt)}</td>`}</tr>`);
    const inp = $('input', tr);
    if (inp) {
      inp.onkeydown = e => { if (e.key === 'Enter') inp.blur(); };
      inp.onchange = () => {
        const v = inp.value.trim();
        if (!v) { checkpoint(); lab.clear_ip(id, i); inp.classList.remove('bad'); }
        else {
          const m = v.match(/^(\d+\.\d+\.\d+\.\d+)(?:\/(\d+))?$/);
          if (!m) { inp.classList.add('bad'); return toast(`"${v}" is not an address like 10.0.0.1/24`, 'err'); }
          checkpoint();
          if (!lab.set_ip(id, i, m[1], lenMask(m[2] ? +m[2] : 24))) { inp.classList.add('bad'); return toast(`"${v}" is not an address like 10.0.0.1/24`, 'err'); }
          inp.classList.remove('bad');
        }
        refresh(); dirty(); renderInspector(true);
      };
    }
    tb.append(tr);
  }
  root.append(ifs);
  if (isSw && !isExt) root.append(switchportSection(id, d));

  if (isHost) {
    const gw = d.routes.find(r => r.prefix === '0.0.0.0')?.next_hop ?? '';
    const s = h(`<div class="sect"><h4>Default gateway</h4><input class="field" value="${esc(gw)}" placeholder="e.g. 192.168.10.1" spellcheck="false" /></div>`);
    const inp = $('input', s);
    inp.onkeydown = e => { if (e.key === 'Enter') inp.blur(); };
    inp.onchange = () => {
      const v = inp.value.trim();
      if (v && !/^\d+\.\d+\.\d+\.\d+$/.test(v)) { inp.classList.add('bad'); return toast(`"${v}" is not an IPv4 address`, 'err'); }
      checkpoint();
      if (!v) { const idx = d.routes.findIndex(r => r.prefix === '0.0.0.0'); if (idx >= 0) lab.remove_route(id, idx); }
      else if (!lab.set_gateway(id, v)) { inp.classList.add('bad'); return toast(`"${v}" is not an IPv4 address`, 'err'); }
      refresh(); dirty(); renderInspector(true);
    };
    root.append(s);
    root.append(servicesSection(id, d));
  }

  if (d.kind === 'router') {
    const s = h(`<div class="sect"><h4>Static routes</h4><ul class="routes"></ul>
      <div class="row"><input class="field" placeholder="0.0.0.0/0" data-p /><input class="field" placeholder="next hop" data-n /><button class="icon-btn sm" data-add title="Add route">${UI.plus}</button></div></div>`);
    const ul = $('ul', s);
    if (!d.routes.length) ul.append(h('<li class="empty-note" style="border:0">Connected networks route automatically.</li>'));
    d.routes.forEach((r, i) => {
      const code = r.proto === 'O' ? 'O' : 'S';
      const li = h(`<li><span>${code} ${esc(r.prefix)}/${maskLen(r.mask)} → ${esc(r.next_hop)} <span class="muted">${esc(ifName(id, r.out_port))}</span></span>${r.proto === 'O' ? '' : `<button class="icon-btn sm" title="Remove">${UI.x}</button>`}</li>`);
      const btn = $('button', li);
      if (btn) btn.onclick = () => { checkpoint(); lab.remove_route(id, i); refresh(); dirty(); renderInspector(true); };
      ul.append(li);
    });
    const add = () => {
      const pv = $('[data-p]', s).value.trim(), nh = $('[data-n]', s).value.trim();
      const m = pv.match(/^(\d+\.\d+\.\d+\.\d+)\/(\d+)$/);
      if (!m) return toast('Prefix must look like 10.1.0.0/16', 'err');
      const port = outPortFor(d, nh);
      if (port < 0) return toast(`Next hop ${nh} is not on any connected network`, 'err');
      checkpoint();
      if (!lab.add_route(id, m[1], lenMask(+m[2]), nh, port)) return toast('Invalid route', 'err');
      refresh(); dirty(); renderInspector(true);
    };
    $('[data-add]', s).onclick = add;
    $('[data-n]', s).onkeydown = e => { if (e.key === 'Enter') add(); };
    root.append(s);
    root.append(natSection(id, d));
    root.append(relaySection(id, d));
    root.append(aclSection(id, d));
    root.append(ospfSection(id, d));
  }

  // quick ping
  if (!isExt && !isSw) {
    const targets = snap.devices.filter(x => x.id !== id).flatMap(x => x.ports.filter(p => p.ip).map(p => ({ name: x.name, ip: p.ip })));
    const s = h(`<div class="sect"><h4>Ping</h4><div class="row">
      <input class="field" list="ping-${id}" placeholder="destination IP" value="${esc(targets[0]?.ip ?? '')}" spellcheck="false" />
      <datalist id="ping-${id}">${targets.map(t => `<option value="${esc(t.ip)}">${esc(t.name)}</option>`).join('')}</datalist>
      <button class="btn sm primary">Send</button></div></div>`);
    const go = () => {
      const ip = $('input', s).value.trim();
      if (!/^\d+\.\d+\.\d+\.\d+$/.test(ip)) return toast('Enter an IPv4 address to ping', 'err');
      openConsole(id);
      consoles.get(id)?.shell?.run(`ping ${ip} -c 3`);
    };
    $('button', s).onclick = go;
    $('input', s).onkeydown = e => { if (e.key === 'Enter') go(); };
    if (!d.ports.some(p => p.ip)) $('button', s).disabled = true;
    root.append(s);
  }

  // tables
  if (!isExt && (d.arp.length || isSw)) {
    const rows = isSw ? d.macs.map(([m, p, v]) => [m, `${ifName(id, p)}  vlan ${v ?? ''}`]) : d.arp;
    const s = h(`<div class="sect"><h4>${isSw ? 'MAC address table' : 'ARP cache'}<span class="spacer"></span><span class="muted mono" style="text-transform:none;letter-spacing:0">${rows.length}</span></h4>
      ${rows.length ? `<table class="tbl-mini">${rows.map(([a, b]) => `<tr><td>${esc(a)}</td><td>${esc(b)}</td></tr>`).join('')}</table>` : '<div class="empty-note">Empty — send some traffic.</div>'}</div>`);
    root.append(s);
  }
  return root;
}

function vmSettings(id, d, st) {
  const m = meta.get(id), running = st !== 'stopped';
  const ros = isRos(id);
  const msw = tplOf(id) === 'mikrotik-sw';
  const osw = tplOf(id) === 'ovs-sw';
  const frr = tplOf(id) === 'frr';
  const fw = tplOf(id) === 'firewall';
  const nics = portCount(id);
  const when = m.diskSaved ? new Date(m.diskSaved).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }) : null;
  const root = h(`<div>
    <div class="sect"><h4>Image</h4>
      <div class="row"><span class="mono" style="flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${m.image ? esc(m.image) : '<span class="muted">none selected</span>'}</span>
      <button class="btn sm" data-img>${ico('image')}${m.image ? 'Change' : 'Choose'}</button></div>
      ${ros ? `<p class="muted" style="margin:8px 0 0;font-size:12px">First start boots the RouterOS v6 x86 ISO on the <b>Screen</b> tab: press <span class="mono">a</span> then <span class="mono">i</span>, answer <span class="mono">n</span>, <span class="mono">y</span>, then Enter to reboot into the installed system. Login <span class="mono">admin</span>, empty password. The Serial tab is the console after that.</p>` : ''}
      ${msw ? `<p class="muted" style="margin:6px 0 0;font-size:12px"><b>CRS Bridge VLANs &amp; RSTP</b>: <span class="mono">/interface bridge add name=br0 vlan-filtering=yes protocol-mode=rstp</span> and add <span class="mono">ether1..ether8</span> under <span class="mono">/interface bridge port</span>.</p>` : ''}
      ${osw ? `<p class="muted" style="margin:8px 0 0;font-size:12px"><b>Open vSwitch / Linux Bridge</b>: configure L2 switching, 802.1Q VLAN trunks, STP/RSTP, or LACP bonding across <span class="mono">eth0..eth7</span> using <span class="mono">ovs-vsctl</span> or <span class="mono">ip link add br0 type bridge vlan_filtering 1</span>.</p>` : ''}
      ${frr ? `<p class="muted" style="margin:8px 0 0;font-size:12px"><b>FRRouting</b>: 32-bit Linux with <span class="mono">frr</span>. Ports <span class="mono">eth0</span>–<span class="mono">eth7</span>. <span class="mono">vtysh</span> for OSPFv2/v3, IS-IS, BGP EVPN and BFD. A starter script is in Startup script.</p>` : ''}
      ${fw ? `<p class="muted" style="margin:8px 0 0;font-size:12px"><b>Firewall</b>: 32-bit Linux. <span class="mono">wan0</span> outside, <span class="mono">dmz0</span>, <span class="mono">lan0</span>–<span class="mono">lan5</span> inside. The startup script installs a stateful <span class="mono">nftables</span> forward chain.</p>` : ''}
    </div>
    <div class="sect"><h4>Hardware</h4>
      <div class="kv">
        <dt>RAM</dt><dd><select class="field" data-ram ${running ? 'disabled' : ''}>${[64, 128, 256, 512].map(v => `<option value="${v}" ${v === m.ram ? 'selected' : ''}>${v} MB</option>`).join('')}</select></dd>
        <dt>New disk</dt><dd><select class="field" data-disk ${running || m.diskSaved ? 'disabled' : ''}>${[64, 128, 256].map(v => `<option value="${v}" ${v === m.diskMB ? 'selected' : ''}>${v} MB</option>`).join('')}</select></dd>
        <dt>NIC model</dt><dd><select class="field" data-nic ${running ? 'disabled' : ''}>
          <option value="ne2k" ${m.nic === 'ne2k' ? 'selected' : ''}>NE2000 (works with RouterOS v6)</option>
          <option value="virtio" ${m.nic === 'virtio' ? 'selected' : ''}>virtio-net (modern Linux)</option></select></dd>
        <dt>NICs</dt><dd><select class="field" data-nics ${running ? 'disabled' : ''}>${[1, 2, 3, 4, 6, 8].map(v => `<option value="${v}" ${v === nics ? 'selected' : ''}>${v} × Ethernet</option>`).join('')}</select></dd>
      </div>
      ${running ? '<p class="muted" style="margin:8px 0 0;font-size:12px">Stop the VM to change hardware.</p>' : ''}
    </div>
    <div class="sect"><h4>Disk</h4>
      <p style="margin:0 0 8px">${when
        ? `<span class="pill ok"><span class="dot running"></span>Saved in this browser</span> <span class="muted" style="font-size:12px">${(m.diskSize / 1048576).toFixed(0)} MB · ${esc(when)}</span>`
        : '<span class="muted">No disk yet — created on first start.</span>'}</p>
      <p class="muted" style="margin:0 0 8px;font-size:12px">Changes are saved a few seconds after the guest writes, and on Stop. VM disks are not included in lab files.</p>
      ${when ? `<button class="btn sm danger" data-reset ${running ? 'disabled title="Stop the VM first"' : ''}>${ico('trash')}Reset disk</button>` : ''}
    </div></div>`);
  $('[data-img]', root).onclick = () => imageManager(id);
  const set = (k, v) => { m[k] = v; dirty(); };
  $('[data-ram]', root).onchange = e => set('ram', +e.target.value);
  $('[data-disk]', root).onchange = e => set('diskMB', +e.target.value);
  $('[data-nic]', root).onchange = e => set('nic', e.target.value);
  $('[data-nics]', root).onchange = e => setNodePorts(id, +e.target.value);
  const rs = $('[data-reset]', root);
  if (rs) rs.onclick = async () => {
    if (!(await confirmBox('Reset disk', `Delete the disk of ${d.name}? Everything installed or configured inside the VM is lost; it starts from the image again.`, 'Reset', true))) return;
    await disks.remove(m.uid);
    m.diskSaved = null; m.diskSize = 0;
    toast(`${d.name}: disk reset`, 'ok');
    renderInspector(true);
  };
  return root;
}

function startupSection(id, d, st) {
  const m = meta.get(id);
  const ext = d.kind === 'ext';
  const s = h(`<div class="sect"><h4>Startup script</h4>
    <p class="muted" style="margin:0 0 8px;font-size:12px">${ext
      ? 'Plain text pasted into the guest console. IOS NVRAM above is separate. Apply once the prompt is up.'
      : 'Cisco-like lines, a MikroTik .rsc, or a shell script. Lines starting with ! or # are skipped on simulated nodes.'}</p>
    <textarea class="field area" spellcheck="false" aria-label="Startup script">${esc(m.startup || '')}</textarea>
    <div class="row" style="margin-top:8px;flex-wrap:wrap">
      ${ext ? '' : `<label class="check"><input type="checkbox" data-boot ${m.startupBoot ? 'checked' : ''}/>Apply on start</label>`}
      <span class="spacer"></span>
      <button class="btn sm primary" data-apply ${st === 'running' ? '' : 'disabled'}>Apply</button>
    </div></div>`);
  const ta = $('textarea', s);
  let startT = null;
  ta.oninput = () => {
    m.startup = ta.value;
    clearTimeout(startT);
    startT = setTimeout(() => setStartupText(id, m.startup ?? null), 300);   // mirror trails the keystrokes — "" is a real seat (Some("")), not absent
    dirty();
  };
  const boot = $('[data-boot]', s);
  if (boot) boot.onchange = () => { checkpoint(); m.startupBoot = boot.checked; dirty(); };
  $('[data-apply]', s).onclick = () => { m.startup = ta.value; pasteStartup(id); };
  return s;
}

function servicesSection(id, d) {
  const s = h(`<div class="sect"><h4>Services</h4>
    <div class="row" style="flex-wrap:wrap;gap:12px">
      <label class="check"><input type="checkbox" data-k="dhcp" ${d.svc_dhcp ? 'checked' : ''}/>DHCP server</label>
      <label class="check"><input type="checkbox" data-k="dns" ${d.svc_dns ? 'checked' : ''}/>DNS</label>
      <label class="check"><input type="checkbox" data-k="http" ${d.svc_http ? 'checked' : ''}/>HTTP :80</label>
    </div>
    <p class="muted" style="margin:8px 0 0;font-size:12px">DHCP answers Discover on a connected interface. DNS answers its A records, or this host's address for any other name. HTTP replies <span class="mono">netlab</span>.</p>
    ${d.svc_dns ? `<div class="row" style="margin-top:8px"><input class="field" data-n placeholder="lab.local" /><input class="field" data-ip placeholder="10.1.0.9" /><button class="btn sm" data-add>A record</button></div>
      <ul class="routes">${(d.dns_records || []).map(([n, ip]) => `<li><span>${esc(n)} → ${esc(ip)}</span></li>`).join('') || ''}</ul>` : ''}
    ${d.dhcp ? `<p style="margin:8px 0 0">DHCP client: <span class="mono">${esc(d.dhcp)}</span>${d.resolver ? ` · DNS ${esc(d.resolver)}` : ''}</p>` : ''}
  </div>`);
  const apply = () => {
    checkpoint();
    const on = k => $(`[data-k="${k}"]`, s).checked;
    lab.set_services(id, on('dhcp'), on('dns'), on('http'));
    refresh(); dirty(); renderInspector(true);
  };
  $$('[data-k]', s).forEach(c => c.onchange = apply);
  const add = $('[data-add]', s);
  if (add) add.onclick = () => {
    checkpoint();
    const name = $('[data-n]', s).value.trim(), ip = $('[data-ip]', s).value.trim();
    if (!name || !/^\d+\.\d+\.\d+\.\d+$/.test(ip) || !lab.add_dns_record(id, name, ip)) return toast('A record needs a name and an IPv4 address', 'err');
    refresh(); dirty(); renderInspector(true);
  };
  return s;
}

function natProtoName(proto) {
  return proto === 6 || proto === 'tcp' ? 'tcp' : proto === 17 || proto === 'udp' ? 'udp' : 'ip';
}

function natSection(id, d) {
  const n = portCount(id);
  const maps = d.static_nat || [];
  const s = h(`<div class="sect"><h4>NAT</h4>
    <p class="muted" style="margin:0 0 8px;font-size:12px">Inside hosts are masqueraded out of the outside interface. One outside port. A 1:1 map sends every new packet for that outside address to one inside host. A port forward does not.</p>
    <table class="iftable"><tbody></tbody></table>
    <div class="row" style="margin-top:8px"><input class="field" data-static placeholder="10.0.0.2" spellcheck="false" /><button class="btn sm" data-add-static>1:1</button></div>
    <div class="row" style="margin-top:6px">
      <select class="field" data-l4><option>tcp</option><option>udp</option></select>
      <input class="field" data-iip placeholder="inside ip" spellcheck="false" />
      <input class="field" data-inn placeholder="80" spellcheck="false" style="width:64px" />
      <input class="field" data-out placeholder="80" spellcheck="false" style="width:64px" />
      <button class="btn sm" data-add-pf>forward</button>
    </div>
    <ul class="routes"></ul>
  </div>`);
  const tb = $('tbody', s);
  for (let i = 0; i < n; i++) {
    const role = d.ports[i].nat || '';
    const tr = h(`<tr><td class="ifn">${esc(ifName(id, i))}</td><td><select class="field" data-p="${i}">
      <option value="" ${role ? '' : 'selected'}>off</option>
      <option value="inside" ${role === 'inside' ? 'selected' : ''}>inside</option>
      <option value="outside" ${role === 'outside' ? 'selected' : ''}>outside</option>
    </select></td></tr>`);
    $('select', tr).onchange = e => {
      checkpoint();
      lab.set_nat(id, i, e.target.value || 'off');
      refresh(); dirty(); renderInspector(true);
    };
    tb.append(tr);
  }
  const list = $('ul', s);
  for (const m of maps) {
    const label = !m.proto ? `1:1 ${m.inside}` : `${natProtoName(m.proto)} ${m.inside}:${m.inside_port} → outside:${m.outside_port}`;
    const li = h(`<li><span>${esc(label)}</span><button class="btn sm" data-x>remove</button></li>`);
    $('[data-x]', li).onclick = () => {
      checkpoint();
      lab.nat_static_clear(id, m.inside || '', natProtoName(m.proto), m.outside_port || 0);
      refresh(); dirty(); renderInspector(true);
    };
    list.append(li);
  }
  $('[data-add-static]', s).onclick = () => {
    const ip = $('[data-static]', s).value.trim();
    if (!/^\d+\.\d+\.\d+\.\d+$/.test(ip)) return toast('1:1 needs an inside IPv4 address', 'err');
    checkpoint();
    if (!lab.nat_static(id, ip, 'ip', 0, 0)) return toast('static map was not added', 'err');
    refresh(); dirty(); renderInspector(true);
  };
  $('[data-add-pf]', s).onclick = () => {
    const proto = $('[data-l4]', s).value;
    const ip = $('[data-iip]', s).value.trim();
    const inn = +$('[data-inn]', s).value;
    const out = +$('[data-out]', s).value;
    if (!/^\d+\.\d+\.\d+\.\d+$/.test(ip) || !(inn > 0 && inn < 65536) || !(out > 0 && out < 65536)) {
      return toast('port forward needs an inside address and two ports', 'err');
    }
    checkpoint();
    if (!lab.nat_static(id, ip, proto, inn, out)) return toast('port forward was not added', 'err');
    refresh(); dirty(); renderInspector(true);
  };
  return s;
}

function switchportSection(id, d) {
  const n = portCount(id);
  const s = h(`<div class="sect"><h4>VLANs</h4>
    <p class="muted" style="margin:0 0 8px;font-size:12px">Access sends one VLAN untagged. A trunk tags every VLAN except the native one. Allowed blank means all.</p>
    <table class="iftable"><thead><tr><th>Port</th><th>Mode</th><th>VLAN</th><th>Allowed</th></tr></thead><tbody></tbody></table></div>`);
  const tb = $('tbody', s);
  for (let i = 0; i < n; i++) {
    const p = d.ports[i];
    const tr = h(`<tr>
      <td class="ifn">${esc(ifName(id, i))}</td>
      <td><select class="field" data-mode><option value="access" ${p.trunk ? '' : 'selected'}>access</option><option value="trunk" ${p.trunk ? 'selected' : ''}>trunk</option></select></td>
      <td><input class="field" data-vid value="${p.trunk ? (p.native_vlan || 1) : (p.access_vlan || 1)}" spellcheck="false" style="width:64px" /></td>
      <td><input class="field" data-allow value="${esc(p.allowed || '')}" placeholder="all" spellcheck="false" ${p.trunk ? '' : 'disabled'} /></td>
    </tr>`);
    const apply = () => {
      const mode = $('[data-mode]', tr).value;
      const vid = +$('[data-vid]', tr).value || 1;
      if (vid < 1 || vid > 4094) return toast('VLAN id is 1–4094', 'err');
      const allowed = $('[data-allow]', tr).value.trim();
      checkpoint();
      lab.set_switchport(id, i, mode, mode === 'access' ? vid : (p.access_vlan || 1), mode === 'trunk' ? vid : (p.native_vlan || 1), mode === 'trunk' && allowed && allowed !== 'all' ? allowed : '');
      refresh(); dirty(); renderInspector(true);
    };
    $('[data-mode]', tr).onchange = apply;
    $('[data-vid]', tr).onchange = apply;
    $('[data-allow]', tr).onchange = apply;
    tb.append(tr);
  }
  return s;
}

function relaySection(id, d) {
  const n = portCount(id);
  const s = h(`<div class="sect"><h4>DHCP relay</h4>
    <p class="muted" style="margin:0 0 8px;font-size:12px">Helper address forwards Discover and Request. The relay address is this interface. With no helper, a limited broadcast stays on the link.</p>
    <table class="iftable"><tbody></tbody></table></div>`);
  const tb = $('tbody', s);
  for (let i = 0; i < n; i++) {
    const tr = h(`<tr><td class="ifn">${esc(ifName(id, i))}</td><td><input class="field" value="${esc(d.ports[i].helper || '')}" placeholder="DHCP server" spellcheck="false" /></td></tr>`);
    $('input', tr).onchange = e => {
      const v = e.target.value.trim();
      if (v && !/^\d+\.\d+\.\d+\.\d+$/.test(v)) return toast('Helper is an IPv4 address', 'err');
      checkpoint();
      if (!lab.set_helper(id, i, v)) return toast('Helper was not set', 'err');
      refresh(); dirty(); renderInspector(true);
    };
    tb.append(tr);
  }
  return s;
}

function aclSection(id, d) {
  const n = portCount(id);
  const lines = [];
  for (const a of d.acls ?? []) {
    for (const e of a.entries ?? []) {
      const proto = PROTO_NAME[e.proto] || 'ip';
      lines.push(`${a.num} ${e.permit ? 'permit' : 'deny'} ${proto} any any${e.dport ? ' eq ' + e.dport : ''}`);
    }
  }
  const s = h(`<div class="sect"><h4>Extended ACL</h4>
    <p class="muted" style="margin:0 0 8px;font-size:12px">A list applied to a port ends with an implicit deny. This form matches any address; wildcards stay on the CLI.</p>
    ${lines.length ? `<ul class="routes">${lines.map(l => `<li><span class="mono">${esc(l)}</span></li>`).join('')}</ul>` : '<div class="empty-note">No ACL entries.</div>'}
    <div class="row" style="flex-wrap:wrap">
      <input class="field" data-num placeholder="100" style="max-width:70px" />
      <select class="field" data-act style="width:auto"><option value="permit">permit</option><option value="deny">deny</option></select>
      <select class="field" data-proto style="width:auto"><option>ip</option><option>icmp</option><option>tcp</option><option>udp</option></select>
      <input class="field" data-port placeholder="port" style="max-width:70px" />
      <button class="btn sm" data-add>Add</button>
    </div>
    <div class="row" style="margin-top:8px;flex-wrap:wrap">
      <select class="field" data-if style="width:auto">${Array.from({ length: n }, (_, i) => `<option value="${i}">${esc(ifName(id, i))}</option>`).join('')}</select>
      <select class="field" data-dir style="width:auto"><option value="in">in</option><option value="out">out</option></select>
      <input class="field" data-gnum placeholder="list" style="max-width:70px" />
      <button class="btn sm" data-grp>Apply</button>
      <button class="btn sm" data-off>Remove</button>
      <button class="btn sm" data-clr>Clear list</button>
    </div></div>`);
  $('[data-add]', s).onclick = () => {
    const num = +$('[data-num]', s).value;
    const proto = $('[data-proto]', s).value;
    const port = +$('[data-port]', s).value || 0;
    if (!(num >= 1 && num <= 199)) return toast('ACL number is 1–199', 'err');
    checkpoint();
    if (!lab.acl_add(id, { num, permit: $('[data-act]', s).value === 'permit', proto, src: '0.0.0.0', src_wild: '255.255.255.255', dst: '0.0.0.0', dst_wild: '255.255.255.255', sport: 0, dport: port })) return toast('ACL entry was not added', 'err');
    refresh(); dirty(); renderInspector(true);
  };
  $('[data-grp]', s).onclick = () => {
    const num = +$('[data-gnum]', s).value;
    if (!(num >= 1)) return toast('Name the list number to apply', 'err');
    checkpoint();
    lab.set_access_group(id, +$('[data-if]', s).value, num, $('[data-dir]', s).value);
    refresh(); dirty(); renderInspector(true);
  };
  $('[data-off]', s).onclick = () => {
    const dir = $('[data-dir]', s).value;
    checkpoint();
    lab.set_access_group(id, +$('[data-if]', s).value, 0, dir === 'out' ? 'off-out' : 'off-in');
    refresh(); dirty(); renderInspector(true);
  };
  $('[data-clr]', s).onclick = () => {
    const num = +$('[data-num]', s).value || +$('[data-gnum]', s).value;
    if (!(num >= 1)) return toast('Name the list number to clear', 'err');
    checkpoint();
    lab.acl_clear(id, num);
    refresh(); dirty(); renderInspector(true);
  };
  return s;
}

function ospfSection(id, d) {
  const nets = d.ospf_nets ?? [];
  const s = h(`<div class="sect"><h4>OSPFv2</h4>
    <p class="muted" style="margin:0 0 8px;font-size:12px">${d.ospf_id ? `Router ID ${esc(d.ospf_id)}. Hellos stay on the link. A static route of the same length wins.` : 'Off. A network statement or an interface area turns it on. This is not Cisco OSPF on the wire.'}</p>
    ${(d.ospf_nbrs ?? []).length ? `<ul class="routes">${d.ospf_nbrs.map(n => `<li><span>neighbor ${esc(n)}</span></li>`).join('')}</ul>` : ''}
    ${nets.length ? `<ul class="routes">${nets.map(n => `<li><span class="mono">${esc(n.prefix)} ${esc(n.wild)} area ${n.area}</span></li>`).join('')}</ul>` : ''}
    <div class="row" style="flex-wrap:wrap">
      <input class="field" data-p placeholder="10.0.0.0" spellcheck="false" />
      <input class="field" data-w placeholder="0.255.255.255" spellcheck="false" />
      <input class="field" data-a placeholder="area" value="0" style="max-width:64px" />
      <button class="btn sm" data-add>Network</button>
      <button class="btn sm" data-clr ${d.ospf_id ? '' : 'disabled'}>Clear</button>
    </div></div>`);
  $('[data-add]', s).onclick = () => {
    const prefix = $('[data-p]', s).value.trim(), wild = $('[data-w]', s).value.trim();
    if (!/^\d+\.\d+\.\d+\.\d+$/.test(prefix) || !/^\d+\.\d+\.\d+\.\d+$/.test(wild)) return toast('OSPF needs a prefix and a wildcard', 'err');
    checkpoint();
    if (!lab.ospf_network(id, prefix, wild, +$('[data-a]', s).value || 0)) return toast('OSPF network was not added', 'err');
    refresh(); dirty(); renderInspector(true);
  };
  $('[data-clr]', s).onclick = () => { checkpoint(); lab.ospf_clear(id); refresh(); dirty(); renderInspector(true); };
  return s;
}

function outPortFor(d, ip) {
  const N = s => s.split('.').reduce((a, o) => ((a << 8) | +o) >>> 0, 0);
  if (!/^\d+\.\d+\.\d+\.\d+$/.test(ip)) return -1;
  return d.ports.findIndex(p => p.ip && ((N(p.ip) & N(p.mask)) >>> 0) === ((N(ip) & N(p.mask)) >>> 0));
}

function linkInspector(lid) {
  const l = linkById.get(lid);
  const a = byId.get(l.a), b = byId.get(l.b);
  const ok = l.up && nodeState(l.a) === 'running' && nodeState(l.b) === 'running';
  const root = h(`<div>
    <div class="cable-head"><svg viewBox="0 0 48 28" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M3 22c10 0 10-16 21-16s11 16 21 16"/><circle cx="3" cy="22" r="2.5" fill="currentColor"/><circle cx="45" cy="22" r="2.5" fill="currentColor"/></svg>
      <div><b style="font-size:15px">Ethernet link</b><div class="muted" style="font-size:12px">${esc(l.speed || '1G')} · ${l.delay} ms${l.jitter ? ` ±${l.jitter}` : ''}${l.loss ? ` · ${l.loss}% loss` : ''}${l.mtu ? ` · MTU ${l.mtu}` : ''}</div></div></div>
    <div class="ends">
      <div class="end"><b>${esc(a.name)}</b><span>${esc(ifName(l.a, l.a_port))}</span></div>
      <div class="wire ${l.up ? '' : 'down'}"></div>
      <div class="end"><b>${esc(b.name)}</b><span>${esc(ifName(l.b, l.b_port))}</span></div>
    </div>
    <div class="sect"><dl class="kv">
      <dt>Cable</dt><dd>${l.up ? '<span class="pill ok"><span class="dot running"></span>Plugged in</span>' : '<span class="pill bad"><span class="dot stopped"></span>Unplugged</span>'}</dd>
      <dt>Link state</dt><dd>${ok ? '<span class="pill ok"><span class="dot running"></span>Up</span>' : '<span class="pill bad"><span class="dot stopped"></span>Down</span>'}</dd>
      <dt>In flight</dt><dd>${l.frames.length} frame${l.frames.length === 1 ? '' : 's'}</dd>
      <dt>Capture</dt><dd>${l.capture ? `<span class="pill bad"><span class="dot stopped"></span>Recording · ${caps.get(lid)?.packets.length ?? 0}</span>` : 'off'}</dd>
    </dl></div>
    <div class="sect"><h4>Link quality</h4>
      <div class="kv">
        <dt>Delay</dt><dd><input class="field" data-delay type="number" min="1" max="60000" value="${l.delay}" /> <span class="muted">ms</span></dd>
        <dt>Jitter</dt><dd><input class="field" data-jitter type="number" min="0" max="60000" value="${l.jitter || 0}" /> <span class="muted">±ms</span></dd>
        <dt>Loss</dt><dd><input class="field" data-loss type="number" min="0" max="100" value="${l.loss || 0}" /> <span class="muted">%</span></dd>
        <dt>MTU</dt><dd><input class="field" data-mtu type="number" min="0" max="9000" value="${l.mtu || 0}" /> <span class="muted">0 = none</span></dd>
        <dt>Speed</dt><dd><select class="field" data-speed>${[['10M', '10M'], ['100M', '100M'], ['1G', '1G'], ['10G', '10G'], ['WAN', 'WAN Serial']].map(([v, label]) => `<option value="${v}" ${v === (l.speed || '1G') ? 'selected' : ''}>${label}</option>`).join('')}</select></dd>
      </div>
      <p class="muted" style="margin:8px 0 0;font-size:12px">Delay and jitter are milliseconds in Realtime. 10M, 100M and WAN Serial add serialization delay. Loss uses a fixed sequence, so 0% never drops and 100% always does.</p>
    </div>
    <div class="insp-actions" style="padding-top:12px">
      <button class="btn sm ${l.up ? '' : 'primary'}" data-a="plug">${ico(l.up ? 'unplug' : 'plug')}${l.up ? 'Unplug' : 'Plug in'}</button>
      <button class="btn sm" data-a="cap">${ico('capture')}${l.capture ? 'Stop capture' : 'Start capture'}</button>
      <span class="spacer"></span>
      <button class="btn sm danger" data-a="del">${ico('trash')}Remove</button>
    </div></div>`);
  const commit = () => {
    checkpoint();
    const num = (sel, d, lo, hi) => {
      const v = +$(sel, root).value;
      return Number.isFinite(v) ? Math.max(lo, Math.min(hi, Math.round(v))) : d;
    };
    const speed = $('select[data-speed]', root).value || '1G';
    lab.set_link_impair(lid, num('[data-delay]', 4, 1, 60000), num('[data-jitter]', 0, 0, 60000), num('[data-loss]', 0, 0, 100), num('[data-mtu]', 0, 0, 9000), speed);
    refresh(); dirty(); renderInspector(true);
  };
  $$('[data-delay],[data-jitter],[data-loss],[data-mtu],[data-speed]', root).forEach(el => { el.onchange = commit; });
  $$('[data-a]', root).forEach(btn => btn.onclick = () => ({
    plug: () => toggleLink(lid), cap: () => setCapture(lid, !l.capture), del: () => { selectOnly('link', lid); deleteSelection(); },
  })[btn.dataset.a]());
  return root;
}

// style used for the next shape drawn (sticky, like most drawing tools)
const shapeStyle = { stroke: SHAPE_COLORS[0], fill: true, dash: false, width: 2 };

function shapeInspector(sh) {
  const name = { rect: 'Rectangle', ellipse: 'Ellipse', line: 'Line' }[sh.kind];
  const b = norm(sh);
  const root = h(`<div>
    <div class="insp-head"><div class="art" style="color:${sh.stroke};display:grid;place-items:center">${UI[sh.kind].replace('<svg', '<svg width="34" height="34"')}</div>
      <div><b style="font-size:15px">${name}</b><div class="sub">${Math.round(b.w)} × ${Math.round(b.h)} at ${Math.round(b.x)}, ${Math.round(b.y)}</div></div></div>
    <div class="sect"><h4>Label</h4><input class="field" data-text value="${esc(sh.text ?? '')}" placeholder="e.g. DMZ, VLAN 10" spellcheck="false" /></div>
    <div class="sect"><h4>Colour</h4><div class="swatches">${SHAPE_COLORS.map(c => `<button class="swatch ${c === sh.stroke ? 'on' : ''}" style="--c:${c}" data-c="${c}" title="${c}"></button>`).join('')}</div></div>
    <div class="sect"><h4>Stroke</h4>
      <div class="row" style="gap:12px;flex-wrap:wrap">
        <div class="seg sm">${[1, 2, 3, 4].map(w => `<button data-w="${w}" class="${sh.width === w ? 'on' : ''}">${w}px</button>`).join('')}</div>
        <label class="check"><input type="checkbox" data-dash ${sh.dash ? 'checked' : ''}/>Dashed</label>
        ${sh.kind !== 'line' ? `<label class="check"><input type="checkbox" data-fill ${sh.fill ? 'checked' : ''}/>Tinted fill</label>` : ''}
      </div></div>
    <div class="insp-actions" style="padding-top:12px">
      <button class="btn sm" data-a="front">${ico('front')}To front</button>
      <button class="btn sm" data-a="back">${ico('back')}To back</button>
      <span class="spacer"></span>
      <button class="btn sm danger" data-a="del">${ico('trash')}Delete</button>
    </div></div>`);
  const apply = patch => {
    Object.assign(sh, patch);
    for (const k of ['stroke', 'fill', 'dash', 'width']) if (k in patch) shapeStyle[k] = patch[k];
    dirty(); renderInspector(true);
  };
  const t = $('[data-text]', root);
  t.onkeydown = e => { if (e.key === 'Enter') t.blur(); };
  t.onchange = () => { sh.text = t.value.trim(); dirty(); };
  $$('[data-c]', root).forEach(b => b.onclick = () => apply({ stroke: b.dataset.c }));
  $$('[data-w]', root).forEach(b => b.onclick = () => apply({ width: +b.dataset.w }));
  $('[data-dash]', root).onchange = e => apply({ dash: e.target.checked });
  const f = $('[data-fill]', root); if (f) f.onchange = e => apply({ fill: e.target.checked });
  $$('[data-a]', root).forEach(b => b.onclick = () => ({
    front: () => reorderShape(sh, true), back: () => reorderShape(sh, false), del: deleteSelection,
  })[b.dataset.a]());
  return root;
}

function multiInspector() {
  const ids = [...sel.nodes];
  const root = h(`<div><div class="sect"><h4>Selection</h4>
    <p>${sel.nodes.size} device${sel.nodes.size === 1 ? '' : 's'}, ${sel.links.size} link${sel.links.size === 1 ? '' : 's'}, ${sel.notes.size} note${sel.notes.size === 1 ? '' : 's'}, ${sel.shapes.size} shape${sel.shapes.size === 1 ? '' : 's'}</p>
    <div class="row" style="flex-wrap:wrap">
      <button class="btn sm" data-a="start" ${ids.length ? '' : 'disabled'}>${ico('play')}Start</button>
      <button class="btn sm" data-a="stop" ${ids.length ? '' : 'disabled'}>${ico('stop')}Stop</button>
      <button class="btn sm" data-a="dup" ${ids.length ? '' : 'disabled'}>${ico('plus')}Duplicate</button>
      <button class="btn sm danger" data-a="del">${ico('trash')}Delete</button>
    </div></div></div>`);
  $$('[data-a]', root).forEach(b => b.onclick = () => ({
    start: () => ids.forEach(startNode), stop: () => ids.forEach(stopNode), dup: duplicateSelection, del: deleteSelection,
  })[b.dataset.a]());
  return root;
}

function overviewInspector() {
  const run = snap.devices.filter(d => nodeState(d.id) === 'running').length;
  const root = h(`<div class="overview">
    <div class="sect"><h4>Lab</h4><div class="stat">
      <div><b>${snap.devices.length}</b><span>devices</span></div>
      <div><b>${snap.links.length}</b><span>links</span></div>
      <div><b>${run}</b><span>running</span></div></div>
      <div class="row" style="margin-top:8px">
        <button class="btn sm" data-act="templates" style="flex:1">${ico('templates')}Browse topology templates</button>
      </div>
    </div>
    <div class="sect"><h4>Getting started</h4><ol class="steps">
      <li>Browse <b>Templates</b> (<kbd>T</kbd>) for ready-to-run topologies from <b>Beginner</b> to <b>Expert</b>, or drag devices from the left onto the canvas.</li>
      <li>Press <kbd>C</kbd>, click a device and pick an interface, then click the other device.</li>
      <li>Select a device to set addresses here, or double-click it for a shell (<span class="mono">ip 10.0.0.2/24 10.0.0.1</span>).</li>
      <li>Right-click a link → <b>Start capture</b> to see every frame decoded, and save it as .pcap for Wireshark.</li>
      <li>Switch to <b>Simulation</b> to step packet by packet.</li>
    </ol></div>
  </div>`);
  return root;
}

/* ════════════════════════════ dock ════════════════════════════ */

function showDock(tab) {
  dockTab = tab;
  if (tab === 'capture' && P.dockH < 380 && innerHeight > 760) {
    P.dockH = 380;
    document.documentElement.style.setProperty('--dock-h', P.dockH + 'px');
  }
  if (P.dockMin) toggleDock();
  renderDockTabs();
  renderDockBody();
}

function renderDockTabs() {
  const el = $('#dock-tabs');
  const tabs = [
    { id: 'events', label: 'Events', n: events.length },
    { id: 'capture', label: 'Capture', n: [...caps.values()].reduce((s, c) => s + c.packets.length, 0), icon: 'capture' },
    ...[...conPlace].filter(([id, p]) => p === 'dock' && byId.has(id)).map(([id]) => ({ id: 'con:' + id, label: byId.get(id).name, art: tplOf(id), dev: id })),
  ];
  el.innerHTML = '';
  for (const t of tabs) {
    const st = t.dev != null ? nodeState(t.dev) : null;
    const b = h(`<button class="dtab ${dockTab === t.id ? 'on' : ''}" role="tab">
      ${t.art ? `<span class="art">${iconSVG(t.art)}</span>` : ''}${st ? `<span class="dot ${st}" style="width:6px;height:6px;box-shadow:none"></span>` : ''}
      <span>${esc(t.label)}</span>${t.n != null ? `<span class="n">${t.n > 9999 ? '9999+' : t.n}</span>` : ''}
      ${t.dev != null ? `<span class="pop" title="Pop out to window">${ico('fit')}</span><span class="x" title="Close console">${ico('x')}</span>` : ''}</button>`);
    b.onclick = e => {
      if (e.target.closest('.x')) return closeConsole(t.dev);
      if (e.target.closest('.pop')) return floatConsole(t.dev);
      showDock(t.id);
    };
    el.append(b);
  }
  el.append(h('<span class="spacer"></span>'));
  const tog = h(`<button class="icon-btn sm" title="Toggle panel (\`)" style="align-self:center">${P.dockMin ? UI.chev.replace('<svg', '<svg style="transform:rotate(-90deg)"') : UI.chev.replace('<svg', '<svg style="transform:rotate(90deg)"')}</button>`);
  tog.onclick = toggleDock;
  el.append(tog);
  hydrateIcons(el);
}

function renderDockBody() {
  const body = $('#dock-body');
  body.innerHTML = '';
  if (dockTab === 'events') body.append(eventsPane());
  else if (dockTab === 'capture') body.append(capturePane());
  else if (dockTab.startsWith('con:')) {
    const id = +dockTab.slice(4), c = consoles.get(id);
    if (c && conPlace.get(id) === 'dock') { body.append(c.el); c.focus?.(); c.scroll?.(); }
    else { dockTab = 'events'; renderDockBody(); return; }
  }
  hydrateIcons(body);
}

function toggleDock() {
  P.dockMin = !P.dockMin;
  prefs.set('dockMin', P.dockMin);
  $('#app').classList.toggle('dock-min', P.dockMin);
  renderDockTabs();
}

// resize handle
$('#dock-resize').addEventListener('pointerdown', e => {
  const startY = e.clientY, startH = P.dockMin ? 34 : P.dockH, handle = e.currentTarget;
  handle.setPointerCapture(e.pointerId);
  handle.classList.add('drag');
  handle.onpointermove = ev => {
    const nh = clamp(startH - (ev.clientY - startY), 34, innerHeight * .7);
    if (P.dockMin && nh > 60) { P.dockMin = false; $('#app').classList.remove('dock-min'); }
    P.dockH = Math.max(120, nh);
    document.documentElement.style.setProperty('--dock-h', P.dockH + 'px');
  };
  handle.onpointerup = () => { handle.onpointermove = null; handle.classList.remove('drag'); prefs.set('dockH', P.dockH); renderDockTabs(); };
});

/* ── events pane ── */
let evTbody = null, evWrap = null, evRendered = 0;
function eventsPane() {
  const pane = h(`<div class="pane">
    <div class="pane-bar">
      ${['ARP', 'ICMP', 'L2', 'L4', 'EXT', 'SYS'].map(c => `<button class="chip ${evFilter.cats.has(c) ? '' : 'off'}" data-cat="${c}" style="--c:var(--${CAT_COLOR[c]})">${c}</button>`).join('')}
      <button class="chip ${evFilter.warnOnly ? '' : 'off'}" data-drops style="--c:var(--bad)">Warnings only</button>
      <span class="spacer"></span>
      <label class="search">${ico('search')}<input placeholder="Filter text" value="${esc(evFilter.q)}" /></label>
      <button class="btn sm ghost" data-clear>${ico('trash')}Clear</button>
    </div>
    <div class="tbl-wrap"><table class="ev"><colgroup><col style="width:56px"><col style="width:84px"><col style="width:110px"><col style="width:110px"><col style="width:64px"><col></colgroup>
      <thead><tr><th>#</th><th>Time</th><th>Device</th><th>Via link to</th><th>Type</th><th>What happened</th></tr></thead><tbody></tbody></table>
      <div class="pane-empty" hidden><div><b>No events yet</b>Ping something — link changes, probe results and syslog notices show up here.</div></div></div>
  </div>`);
  $$('[data-cat]', pane).forEach(b => b.onclick = () => {
    const c = b.dataset.cat;
    evFilter.cats.has(c) ? evFilter.cats.delete(c) : evFilter.cats.add(c);
    b.classList.toggle('off');
    renderEvents(true);
  });
  $('[data-drops]', pane).onclick = e => { evFilter.warnOnly = !evFilter.warnOnly; e.currentTarget.classList.toggle('off'); renderEvents(true); };
  $('.search input', pane).oninput = e => { evFilter.q = e.target.value.toLowerCase(); renderEvents(true); };
  $('[data-clear]', pane).onclick = () => { events.length = 0; renderEvents(true); renderDockTabs(); };
  evTbody = $('tbody', pane);
  evWrap = $('.tbl-wrap', pane);
  evRendered = 0;
  queueMicrotask(() => renderEvents(true));
  return pane;
}
// Row class comes from the adapter's own parse table — warn → the red row,
// a delivered ping glyph → the green one. Nothing invents a second mapping.
const evCls = ev => parse_event(ev.kind, ev.msg).cls;
const evVisible = ev => evFilter.cats.has(ev.cat) && (!evFilter.warnOnly || evCls(ev) === 'warn') &&
  (!evFilter.q || `${ev.msg} ${ev.node} ${ev.via}`.toLowerCase().includes(evFilter.q));

function evRow(ev) {
  const cls = evCls(ev);
  const tr = document.createElement('tr');
  tr.className = (cls === 'warn' ? 'drop' : cls === 'echo' && ev.msg === '!' ? 'deliver' : '') + (evSelected === ev.n ? ' sel' : '');
  const color = cls === 'warn' ? 'var(--bad)' : `var(--${CAT_COLOR[ev.cat]})`;
  const label = ev.cat;
  tr.innerHTML = `<td class="n">${ev.n}</td><td class="t">${(ev.tick * TICK_MS).toFixed(1)} ms</td><td>${esc(ev.node)}</td><td>${esc(ev.via)}</td>
    <td><span class="badge" style="--c:${color}">${label}</span></td><td class="msg" title="${esc(ev.msg)}">${esc(ev.msg.replace(/^[^:]+:\s*/, ''))}</td>`;
  tr.onclick = () => {
    evSelected = ev.n;
    $$('tr.sel', evTbody).forEach(r => r.classList.remove('sel'));
    tr.classList.add('sel');
    if (ev.device != null && byId.has(ev.device)) {
      selectOnly('node', ev.device);
      flashes.push({ dev: ev.device, color: C.accent, t0: performance.now() });
    }
  };
  return tr;
}

function renderEvents(full) {
  if (!evTbody || !evTbody.isConnected) return;
  const atBottom = evWrap.scrollHeight - evWrap.scrollTop - evWrap.clientHeight < 30;
  if (full || evRendered > events.length || (events.length && evRendered && events[0].n > 1 && !evTbody.firstChild)) {
    evTbody.innerHTML = '';
    evRendered = 0;
  }
  // events may have been trimmed from the front; render whatever is newer than the last row
  const lastN = +evTbody.lastChild?.firstChild?.textContent || 0;
  const frag = document.createDocumentFragment();
  const fresh = events.filter(ev => ev.n > lastN && evVisible(ev));
  for (const ev of fresh.slice(-1500)) frag.append(evRow(ev));
  evTbody.append(frag);
  while (evTbody.children.length > 1500) evTbody.firstChild.remove();
  evRendered = events.length;
  $('.pane-empty', evWrap.parentElement).hidden = !!evTbody.children.length;
  if (atBottom || full) evWrap.scrollTop = evWrap.scrollHeight;
}

/* ── capture pane ── */
function capturePane() {
  if (!caps.size) {
    return h(`<div class="pane"><div class="pane-empty"><div><b>No captures running</b>Right-click a link → Start capture, or double-click a link.<br>Frames are decoded here and can be saved as .pcap for Wireshark.</div></div></div>`);
  }
  if (capLink == null || !caps.has(capLink)) capLink = caps.keys().next().value;
  const s = caps.get(capLink);
  const live = linkById.get(capLink)?.capture;
  const pane = h(`<div class="pane">
    <div class="pane-bar">
      <b style="font-weight:600">${esc(s.name)}</b>
      <span class="pill ${live && !s.paused ? 'bad' : ''}">${live ? (s.paused ? 'Paused' : '● Recording') : 'Stopped'}</span>
      <span class="spacer"></span>
      ${live ? `<button class="btn sm" data-a="pause">${ico(s.paused ? 'play' : 'pause')}${s.paused ? 'Resume' : 'Pause'}</button>
      <button class="btn sm" data-a="stop">${ico('stop')}Stop</button>` : `<button class="btn sm" data-a="restart">${ico('play')}Restart</button>`}
      <button class="btn sm" data-a="clear">${ico('trash')}Clear</button>
      <button class="btn sm" data-a="replay">Replay</button>
      <button class="btn sm" data-a="follow">Follow TCP</button>
      <button class="btn sm primary" data-a="pcap" ${s.packets.length ? '' : 'disabled'}>${ico('download')}Save .pcap</button>
    </div>
    <div class="pane-bar cap-filter">
      <label class="search" style="flex:1">${ico('search')}<input data-filt placeholder="stp · vlan == 10 · arp · icmp · ip == 10.0.0.1" value="${esc(s.filter || '')}" spellcheck="false" /></label>
    </div>
    <details class="cap-craft">
      <summary>Craft frame</summary>
      <div class="row" style="flex-wrap:wrap;padding:6px 8px;gap:6px">
        <select class="field" data-from style="width:auto"></select>
        <select class="field" data-kind style="width:auto"><option value="arp">ARP who-has</option><option value="icmp">ICMP echo</option></select>
        <input class="field" data-dst placeholder="dst MAC" value="ff:ff:ff:ff:ff:ff" spellcheck="false" />
        <input class="field" data-srcm placeholder="src MAC" spellcheck="false" />
        <input class="field" data-vlan placeholder="VLAN" spellcheck="false" style="max-width:70px" />
        <input class="field" data-aip placeholder="sender / src IP" spellcheck="false" />
        <input class="field" data-bip placeholder="target IP" spellcheck="false" />
        <button class="btn sm" data-a="craft">Inject</button>
      </div>
    </details>
    <div class="cap">
      <div class="cap-sessions"></div>
      <div class="cap-main">
        <div class="tbl-wrap"><table class="ev"><colgroup><col style="width:56px"><col style="width:80px"><col style="width:140px"><col style="width:140px"><col style="width:70px"><col style="width:60px"><col></colgroup>
          <thead><tr><th>No.</th><th>Time</th><th>Source</th><th>Destination</th><th>Protocol</th><th>Length</th><th>Info</th></tr></thead><tbody></tbody></table></div>
        <div class="cap-lower"><div class="cap-tree"></div><div class="cap-hex"></div></div>
      </div>
    </div></div>`);
  const sess = $('.cap-sessions', pane);
  for (const [lid, c] of caps) {
    const on = linkById.get(lid)?.capture;
    const row = h(`<div class="cap-sess ${lid === capLink ? 'on' : ''}"><span class="rec ${on && !c.paused ? '' : 'paused'}"></span><span>${esc(c.name)}</span><span class="n">${c.packets.length}</span></div>`);
    row.onclick = () => { capLink = lid; capPkt = null; renderDockBody(); };
    sess.append(row);
  }
  const link = linkById.get(capLink);
  const fromSel = $('[data-from]', pane);
  if (link && fromSel) {
    for (const [dev, port] of [[link.a, link.a_port], [link.b, link.b_port]]) {
      const opt = document.createElement('option');
      opt.value = String(dev);
      opt.textContent = `${byId.get(dev)?.name ?? '?'} ${ifName(dev, port)}`;
      fromSel.append(opt);
    }
    const fillMac = () => {
      const dev = +fromSel.value;
      const port = link.a === dev ? link.a_port : link.b_port;
      const mac = byId.get(dev)?.ports?.[port]?.mac;
      if (mac) $('[data-srcm]', pane).value = mac;
    };
    fromSel.onchange = fillMac;
    fillMac();
  }
  const filt = $('[data-filt]', pane);
  if (filt) filt.oninput = e => {
    s.filter = e.target.value;
    if (capTbody) { capTbody.innerHTML = ''; delete capTbody.dataset.filt; }
    renderCapture();
  };
  $$('[data-a]', pane).forEach(b => b.onclick = () => {
    const a = b.dataset.a;
    if (a === 'craft') { injectCraft(pane, capLink); return; }
    if (a === 'replay') {
      const p = s.packets.find(x => x.n === capPkt);
      if (!p) return toast('Select a frame to replay', 'err');
      if (!lab.inject_link(capLink, p.from, p.bytes)) return toast('Could not replay that frame', 'err');
      toast('Frame replayed onto the link');
      return;
    }
    if (a === 'follow') { followTcp(); return; }
    if (a === 'pause') s.paused = !s.paused;
    if (a === 'stop') { lab.set_capture(capLink, false); refresh(); }
    if (a === 'restart') { lab.set_capture(capLink, true); s.paused = false; refresh(); }
    if (a === 'clear') { s.packets = []; capPkt = null; }
    if (a === 'pcap') download(toPcap(s.packets), `${s.name.replace(/[^\w.-]+/g, '_')}.pcap`);
    renderDockBody(); renderDockTabs();
  });
  capTbody = $('tbody', pane);
  capWrap = $('.tbl-wrap', pane);
  capTree = $('.cap-tree', pane);
  capHex = $('.cap-hex', pane);
  queueMicrotask(() => { renderCapture(); renderPacket(); });
  return pane;
}
let capTbody, capWrap, capTree, capHex;
function renderCapture() {
  if (!capTbody?.isConnected) { if (dockTab === 'capture') renderDockBody(); return; }
  const s = caps.get(capLink);
  if (!s) return;
  const atBottom = capWrap.scrollHeight - capWrap.scrollTop - capWrap.clientHeight < 30;
  const filt = (s.filter || '').trim();
  if (capTbody.dataset.filt !== filt) {
    capTbody.innerHTML = '';
    capTbody.dataset.filt = filt;
  }
  const lastN = +capTbody.lastChild?.firstChild?.textContent || 0;
  const frag = document.createDocumentFragment();
  for (const p of s.packets) {
    if (!matchFilter(p.d, filt)) continue;
    if (p.n <= lastN) continue;
    const tr = document.createElement('tr');
    tr.className = 'p-' + protoClass(p.d) + (capPkt === p.n ? ' sel' : '');
    tr.innerHTML = `<td class="n">${p.n}</td><td class="t">${(p.tick * TICK_MS / 1000).toFixed(4)}</td><td class="mono">${esc(p.d.src)}</td><td class="mono">${esc(p.d.dst)}</td>
      <td class="proto mono">${esc(p.d.proto)}</td><td class="t">${p.bytes.length}</td><td title="${esc(p.d.info)}">${esc(p.d.info)}</td>`;
    tr.onclick = () => { capPkt = p.n; $$('tr.sel', capTbody).forEach(r => r.classList.remove('sel')); tr.classList.add('sel'); renderPacket(); };
    frag.append(tr);
  }
  capTbody.append(frag);
  while (capTbody.children.length > 3000) capTbody.firstChild.remove();
  if (atBottom) capWrap.scrollTop = capWrap.scrollHeight;
  $('.cap-sess.on .n')?.replaceChildren(String(s.packets.length));
}
function renderPacket() {
  const s = caps.get(capLink);
  const p = s?.packets.find(x => x.n === capPkt);
  if (!p) { capTree.innerHTML = '<div class="muted" style="padding:6px 8px">Select a packet to see its fields.</div>'; capHex.innerHTML = ''; return; }
  capTree.innerHTML = '';
  const from = byId.get(p.from)?.name ?? '?';
  capTree.append(h(`<div class="fld" style="padding-left:6px">Frame ${p.n}: ${p.bytes.length} bytes on wire, sent by ${esc(from)}</div>`));
  for (const layer of p.d.tree) {
    const det = h(`<details open><summary>${esc(layer.title)}</summary></details>`);
    $('summary', det).onmouseenter = () => hl(layer.range);
    for (const f of layer.fields) {
      const fl = h(`<div class="fld"><b>${esc(f.k)}:</b> ${esc(f.v)}</div>`);
      fl.onmouseenter = () => hl(f.range);
      det.append(fl);
    }
    capTree.append(det);
  }
  capTree.onmouseleave = () => hl(null);
  capHex.innerHTML = hexRows(p.bytes).map(r =>
    `<div><span class="o">${r.off}</span>${r.bytes.map(b => `<span class="b" data-i="${b.i}">${b.h}</span>`).join(' ')}${'   '.repeat(16 - r.bytes.length)}<span class="a">${r.ascii.map(a => `<span data-i="${a.i}">${esc(a.c)}</span>`).join('')}</span></div>`).join('');
}
function injectCraft(pane, lid) {
  const from = +$('[data-from]', pane).value;
  const kind = $('[data-kind]', pane).value;
  const aip = $('[data-aip]', pane).value.trim();
  const bip = $('[data-bip]', pane).value.trim();
  const bytes = craftFrame({
    dst: $('[data-dst]', pane).value.trim(),
    src: $('[data-srcm]', pane).value.trim(),
    vlan: +$('[data-vlan]', pane).value || 0,
    kind,
    spa: aip, tpa: bip, icmpSrc: aip, icmpDst: bip,
  });
  if (!bytes) return toast('Craft needs a source and destination MAC, plus two IPv4 addresses', 'err');
  if (!lab.inject_link(lid, from, bytes)) return toast('Could not inject on this link', 'err');
  toast(kind === 'arp' ? 'ARP injected' : 'ICMP injected', 'ok');
}

function followTcp() {
  const s = caps.get(capLink);
  const p = s?.packets.find(x => x.n === capPkt);
  const flow = p && tcpFlow(p.bytes);
  if (!flow) return toast('Select a TCP packet to follow', 'err');
  const key = `${flow.src}:${flow.sp}>${flow.dst}:${flow.dp}`;
  const rev = `${flow.dst}:${flow.dp}>${flow.src}:${flow.sp}`;
  const chunks = [];
  let last = '';
  for (const pkt of s.packets) {
    const f = tcpFlow(pkt.bytes);
    if (!f?.data?.length) continue;
    const k = `${f.src}:${f.sp}>${f.dst}:${f.dp}`;
    if (k !== key && k !== rev) continue;
    const dir = `${f.src}:${f.sp} → ${f.dst}:${f.dp}`;
    if (dir !== last) { chunks.push(`\n— ${dir}\n`); last = dir; }
    chunks.push(new TextDecoder('latin1').decode(f.data));
  }
  capTree.replaceChildren();
  const pre = document.createElement('pre');
  pre.className = 'stream';
  pre.textContent = chunks.join('').trim() || '(no TCP payload in this flow)';
  capTree.append(pre);
  capHex.innerHTML = '';
}

function hl(range) {
  $$('.hl', capHex).forEach(e => e.classList.remove('hl'));
  if (!range) return;
  for (const e of $$('[data-i]', capHex)) { const i = +e.dataset.i; if (i >= range[0] && i < range[1]) e.classList.add('hl'); }
}

/* ── consoles ── */
function openConsole(id) {
  const d = byId.get(id);
  if (!d) return;
  if (!consoles.has(id)) consoles.set(id, isVM(id) ? vmConsole(id) : d.kind === 'ext' ? iosConsole(id) : simConsole(id));
  const key = 'con:' + id;
  if (conPlace.get(id) === 'dock') return showDock(key);
  if (hasTab(key)) return activateTab(key);
  addTab(key, consoleTab(id));
  consoles.get(id).scroll?.();
}
/** A console as a floating-window tab; its element moves between the window and the bottom panel. */
function consoleTab(id) {
  const d = byId.get(id), c = consoles.get(id);
  return {
    label: d.name,
    art: `<span class="art">${iconSVG(tplOf(id))}</span>`,
    state: () => nodeState(id),
    el: c.el,
    focus: () => c.focus?.(),   // runs after the element is in the window, so the VM terminal can measure itself
    onClose: () => closeConsole(id),
    onDock: () => dockConsole(id),
  };
}
function dockConsole(id) {
  conPlace.set(id, 'dock');
  showDock('con:' + id);
}
function floatConsole(id) {
  conPlace.delete(id);
  if (dockTab === 'con:' + id) dockTab = 'events';
  renderDockTabs(); renderDockBody();
  addTab('con:' + id, consoleTab(id));
}
function closeConsole(id) {
  removeTab('con:' + id);
  const wasDocked = conPlace.delete(id);
  consoles.delete(id);
  if (wasDocked) { if (dockTab === 'con:' + id) dockTab = 'events'; renderDockTabs(); renderDockBody(); }
}

function termShell() {
  const el = h(`<div class="pane"><div class="term"><div class="term-bar"></div><div class="term-out" tabindex="0"></div><div class="term-in"><span class="pr"></span><input spellcheck="false" autocomplete="off" aria-label="Console input" /></div></div></div>`);
  const out = $('.term-out', el);
  const write = (text, cls = '') => {
    if (text === '\x0c') { out.innerHTML = ''; return; }
    write.batch([{ text, cls }]);
  };
  // One append + one scroll read per batch — the adapter's multi-line
  // renders land as a single fragment, not a layout pass per line.
  write.batch = spans => {
    const atBottom = out.scrollHeight - out.scrollTop - out.clientHeight < 40;
    const frag = document.createDocumentFragment();
    for (const { text, cls } of spans) {
      const sp = document.createElement('span');
      if (cls) sp.className = cls;
      sp.textContent = text;
      frag.append(sp);
    }
    out.append(frag);
    while (out.childNodes.length > 4000) out.firstChild.remove();
    if (atBottom) out.scrollTop = out.scrollHeight;
  };
  return { el, out, write, bar: $('.term-bar', el), input: $('input', el), pr: $('.pr', el) };
}

function simConsole(id) {
  const t = termShell();
  // host facade {app, meta, lab} — the adapter reaches lab per call so a
  // restore() swap stays live; refresh and dirty are split channels:
  // CliResult.mutated gates refresh+dirty together, intents mark dirty alone.
  const app = {
    dev: i => byId.get(i), snap: () => snap, ifName, ifNames, portCount,
    refresh: () => { refresh(); renderInspector(true); },
    dirty,
    realtime: () => mode === 'realtime',
    rename: (i, n) => rename(i, n),
    checkpoint,
  };
  const shell = new SimShell({ app, meta, get lab() { return lab; } }, id, t.write);
  t.bar.innerHTML = `<span>${esc(byId.get(id).name)} · ${esc(tpl(tplOf(id)).label)} shell</span><span class="spacer"></span><button class="btn sm" data-clear>${ico('trash')}Clear</button>`;
  $('[data-clear]', t.bar).onclick = () => t.write('\x0c');
  // the adapter owns the cmdInput keymap (Enter/`?`/Tab/chords/arrows/paste)
  // — keydown routes inside cli.js: Ctrl+Z→cli_key('ctrlz'), Ctrl+C→cli_key('ctrlc')
  // + pingJob abort, `?`/Tab→value.slice(0, selectionStart), paste→cli_script batch
  shell.attach({
    shell: t.el.querySelector('.term'), input: t.input, pr: t.pr,
    flash: () => { const el = t.el.querySelector('.term'); el.style.filter = 'brightness(1.35)'; setTimeout(() => { el.style.filter = ''; }, 90); },
  });
  shell.mount();
  return {
    shell, el: t.el,
    focus: () => t.input.focus(),
    scroll: () => { t.out.scrollTop = t.out.scrollHeight; },
  };
}

/* ── VM console: Serial (xterm.js) + Screen (VGA) ── */
let xtermP = null;
function loadXterm() {
  const add = (tag, attrs) => new Promise((res, fail) => { const e = document.createElement(tag); Object.assign(e, attrs); e.onload = res; e.onerror = fail; document.head.append(e); });
  return xtermP ??= Promise.all([
    add('link', { rel: 'stylesheet', href: 'vendor/xterm/xterm.css' }),
    add('script', { src: 'vendor/xterm/xterm.js' }).then(() => add('script', { src: 'vendor/xterm/addon-fit.js' })),
  ]);
}
function vmConsoleOf(id) {
  if (!consoles.has(id)) consoles.set(id, vmConsole(id));
  return consoles.get(id);
}
function vmConsole(id) {
  const node = emus.get(id);
  const el = h(`<div class="pane"><div class="term vm-term">
    <div class="term-bar">
      <div class="seg sm vm-views"><button data-v="serial">Serial</button><button data-v="screen">Screen</button></div>
      <span class="vm-title"></span><span class="spacer"></span><span data-ctl></span>
    </div>
    <div class="vm-body"><div class="vm-serial"></div><div class="vm-screen"></div></div></div></div>`);
  const serialEl = $('.vm-serial', el), screenEl = $('.vm-screen', el);
  screenEl.append(node.screen.el);
  let view = 'serial', term = null, fit = null, pending = '';
  const dec = new TextDecoder();
  const ensureTerm = async () => {
    if (term || !serialEl.isConnected) return;
    await loadXterm();
    if (term) return;
    term = new window.Terminal({
      fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--mono'), fontSize: 13, lineHeight: 1.15,
      cursorBlink: true, scrollback: 5000, convertEol: false,
      theme: { background: '#07090d', foreground: '#d7dde8', cursor: '#6fb3ff', selectionBackground: '#2a3a55' },
    });
    fit = new window.FitAddon.FitAddon();
    term.loadAddon(fit);
    term.open(serialEl);
    const doFit = () => { try { if (serialEl.offsetWidth) fit.fit(); } catch { /* hidden */ } };
    doFit();
    new ResizeObserver(doFit).observe(serialEl);
    term.onData(d => emus.get(id)?.sendSerial(new TextEncoder().encode(d)));
    if (pending) { term.write(pending); pending = ''; }
    if (view === 'serial') term.focus();
  };
  const write = str => { if (term) term.write(str); else pending = (pending + str).slice(-200000); };
  const renderBar = () => {
    const st = nodeState(id);
    $('.vm-title', el).textContent = `${byId.get(id)?.name ?? ''} · ${tpl(tplOf(id)).label}`;
    const ctl = $('[data-ctl]', el);
    ctl.innerHTML = st === 'stopped' ? `<button class="btn sm" data-s>${ico('play')}Start</button>` : `<button class="btn sm" data-x>${ico('stop')}Stop</button>`;
    $('[data-s]', ctl)?.addEventListener('click', () => startNode(id));
    $('[data-x]', ctl)?.addEventListener('click', () => stopNode(id));
    hydrateIcons(ctl);
  };
  const setView = v => {
    view = v;
    $$('.vm-views button', el).forEach(b => b.classList.toggle('on', b.dataset.v === v));
    serialEl.hidden = v !== 'serial';
    screenEl.hidden = v !== 'screen';
    if (v === 'serial') ensureTerm().then(() => term?.focus());
    else node.screen.el.focus();
  };
  $$('.vm-views button', el).forEach(b => b.onclick = () => setView(b.dataset.v));
  // keys typed on the VGA screen go to the guest's PS/2 keyboard and nowhere else
  node.screen.el.addEventListener('keydown', e => { if (node.screenKey(e)) { e.preventDefault(); e.stopPropagation(); } });
  node.screen.el.addEventListener('mousedown', () => node.screen.el.focus());
  renderBar();
  setView('serial');
  write(`\x1b[2m${byId.get(id)?.name}: ${nodeState(id) === 'stopped' ? 'stopped, press Start' : 'serial console'}\x1b[0m\r\n`);
  return {
    el,
    serial: bytes => write(dec.decode(bytes, { stream: true })),
    log: msg => write(`\x1b[2m[netlab] ${msg}\x1b[0m\r\n`),
    setView,
    onState: st => {
      renderBar();
      if (st === 'stopped') write('\r\n\x1b[2m[VM stopped]\x1b[0m\r\n');
      // after Start, the click's focus (the Start button) is usually gone; put the keyboard in the console
      const a = document.activeElement;
      if (st === 'running' && (!a || a === document.body || !a.isConnected)) setView(view);
    },
    focus: () => setView(view),
    scroll: () => {},
  };
}

const ANSI_RE = /\x1b\[[0-9;?]*[a-zA-Z]|\x1b[()][0-9A-B]/g;
function iosConsole(id) {
  const t = termShell();
  t.el.querySelector('.term-in').remove();
  const renderBar = () => {
    const st = nodeState(id), img = meta.get(id)?.image;
    t.bar.innerHTML = `<span>${esc(byId.get(id)?.name)} · console (9600 8N1) · ${img ? esc(img) : 'no image'}</span><span class="spacer"></span>
      ${st === 'stopped' ? `<button class="btn sm" data-s>${ico('play')}Start</button>` : `<button class="btn sm" data-x>${ico('stop')}Stop</button>`}`;
    $('[data-s]', t.bar)?.addEventListener('click', () => startNode(id));
    $('[data-x]', t.bar)?.addEventListener('click', () => stopNode(id));
    hydrateIcons(t.bar);
  };
  renderBar();
  if (nodeState(id) === 'stopped') t.write(`${byId.get(id)?.name} is stopped. Start it to boot IOS — the console streams here.\n`, 'dim');
  const onState = s => { renderBar(); if (s === 'stopped') t.write('\n[node stopped]\n', 'dim'); };
  let line = document.createElement('span');
  t.out.append(line);
  let pendingCon = '', conRaf = 0;
  const LOG_KEEP = 60000;
  let pendingLog = '', logRaf = 0;
  const flushLog = () => {
    logRaf = 0;
    if (!pendingLog) return;
    const sp = document.createElement('span');
    sp.className = 'dim';
    sp.textContent = pendingLog;
    pendingLog = '';
    t.out.insertBefore(sp, line);
    while (t.out.childNodes.length > 400) t.out.firstChild.remove();
    t.out.scrollTop = t.out.scrollHeight;
  };
  const cursor = h('<span class="cursor">&nbsp;</span>');
  t.out.append(cursor);
  const dec = new TextDecoder('latin1');
  const c = {
    el: t.el,
    write(bytes) {
      pendingCon += dec.decode(bytes);
      if (pendingCon.length > 200000) pendingCon = pendingCon.slice(-200000);
      if (!conRaf) conRaf = requestAnimationFrame(() => { conRaf = 0; c.flushCon(); });
    },
    flushCon() {
      const txt = pendingCon.replace(ANSI_RE, '');
      pendingCon = '';
      let s = line.textContent;
      const flush = () => { line.textContent = s; };
      for (const ch of txt) {
        if (ch === '\r' || ch === '\x00' || ch === '\x07') continue;
        if (ch === '\b' || ch === '\x7f') s = s.slice(0, -1);
        else if (ch === '\n') { s += '\n'; flush(); line = document.createElement('span'); t.out.insertBefore(line, cursor); s = ''; }
        else s += ch;
      }
      flush();
      while (t.out.childNodes.length > 3000) t.out.firstChild.remove();
      t.out.scrollTop = t.out.scrollHeight;
    },
    onState,
    // batched: render at most once per frame, keep the last LOG_KEEP chars
    log(text) {
      pendingLog += text.endsWith('\n') ? text : text + '\n';
      if (pendingLog.length > LOG_KEEP) pendingLog = pendingLog.slice(-LOG_KEEP);
      if (!logRaf) logRaf = requestAnimationFrame(flushLog);
    },
    focus: () => t.out.focus(),
    scroll: () => { t.out.scrollTop = t.out.scrollHeight; },
  };
  t.out.addEventListener('keydown', e => {
    const n = emus.get(id);
    if (!n) return;
    let b = null;
    if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) b = new TextEncoder().encode(e.key);
    else if (e.key === 'Enter') b = new Uint8Array([13]);
    else if (e.key === 'Backspace') b = new Uint8Array([127]);
    else if (e.key === 'Tab') b = new Uint8Array([9]);
    else if (e.key === 'Escape') b = new Uint8Array([27]);
    else if (e.ctrlKey && /^[a-z]$/i.test(e.key)) b = new Uint8Array([e.key.toLowerCase().charCodeAt(0) - 96]);
    else if (e.key === 'ArrowUp') b = new Uint8Array([16]);   // Ctrl-P: IOS history
    else if (e.key === 'ArrowDown') b = new Uint8Array([14]); // Ctrl-N
    if (b) { n.sendKeys(b); e.preventDefault(); }
  });
  t.out.addEventListener('paste', e => { emus.get(id)?.sendKeys(new TextEncoder().encode(e.clipboardData.getData('text').replace(/\n/g, '\r'))); e.preventDefault(); });
  return c;
}
function consoleWrite(id, bytes) {
  if (!consoles.has(id)) consoles.set(id, iosConsole(id));
  consoles.get(id).write(bytes);
}

/* ════════════════════════════ status + chrome ════════════════════════════ */

function renderStatus() {
  const m = $('#st-mode');
  m.textContent = mode === 'realtime' ? 'Realtime' : playing ? 'Simulation · playing' : 'Simulation · paused';
  m.classList.toggle('paused', mode === 'sim' && !playing);
  $('#st-tick').textContent = `tick ${snap.tick} · ${(snap.tick * TICK_MS).toFixed(1)} ms sim`;
  const run = snap.devices.filter(d => nodeState(d.id) === 'running').length;
  $('#st-counts').textContent = `${snap.devices.length} devices · ${snap.links.length} links · ${run} running`;
  $('#empty').hidden = snap.devices.length > 0;
}

function renderAll() {
  renderLibrary();
  renderTopo(true);
  renderInspector(true);
  renderDockTabs();
  renderDockBody();
  renderStatus();
  updateHint();
}

function togglePref(k) {
  P[k] = !P[k];
  prefs.set(k, P[k]);
  applyPrefs();
}
function applyPrefs() {
  $$('#toolbar [data-pref]').forEach(b => b.classList.toggle('on', !!P[b.dataset.pref]));
  $('#minimap').hidden = !P.minimap;
  document.documentElement.dataset.theme = P.theme;
  $('#theme').innerHTML = P.theme === 'dark' ? UI.sun : UI.moon;
  document.documentElement.style.setProperty('--dock-h', P.dockH + 'px');
  $('#app').classList.toggle('dock-min', P.dockMin);
  readColors();
}
function toggleTheme() { P.theme = P.theme === 'dark' ? 'light' : 'dark'; prefs.set('theme', P.theme); applyPrefs(); }

/* ════════════════════════════ wiring ════════════════════════════ */

$('#toolbar').addEventListener('click', e => {
  const b = e.target.closest('button');
  if (!b) return;
  if (b.dataset.tool) setTool(b.dataset.tool);
  if (b.dataset.pref) togglePref(b.dataset.pref);
  const a = b.dataset.act;
  if (a === 'zoom-in') zoomAt(W / 2, H / 2, 1.2);
  if (a === 'zoom-out') zoomAt(W / 2, H / 2, 1 / 1.2);
  if (a === 'zoom-reset') zoomAt(W / 2, H / 2, 1 / view.z);
  if (a === 'fit') fit();
});
document.addEventListener('click', e => {
  const a = e.target.closest('[data-act]')?.dataset.act;
  if (a === 'start-all') startAll();
  if (a === 'stop-all') stopAll();
  if (a === 'templates') templateMarketplace();
  if (a === 'images') imageManager();
  if (a === 'demo') { checkpoint(); restore(DEMO()); fit(); dirty(); }
});
$$('#mode button').forEach(b => b.onclick = () => setMode(b.dataset.mode));
$('#sim-play').onclick = () => { playing = !playing; updatePlay(); renderStatus(); };
$('#sim-step').onclick = stepOnce;
$('#speed').oninput = e => { speed = SPEEDS[+e.target.value]; $('#speed-out').textContent = speed + '×'; prefs.set('speed', +e.target.value); };
$('#theme').onclick = toggleTheme;
$('#lab-name').onchange = e => { labName = e.target.value.trim() || 'Untitled lab'; dirty(); };
$('#lab-name').onkeydown = e => { if (e.key === 'Enter') e.target.blur(); };

addEventListener('keydown', e => {
  if (e.key === 'Escape' && !ctxEl.hidden) return closeMenu();
  if ($('.modal-bg')) return;
  if (isTyping(e)) return;
  const k = e.key, mod = e.ctrlKey || e.metaKey;
  if (k === ' ' && e.shiftKey) { if (mode !== 'sim') setMode('sim'); playing = !playing; updatePlay(); e.preventDefault(); return; }
  if (k === ' ') { if (!spaceDown) { spaceDown = true; cv.style.cursor = 'grab'; } e.preventDefault(); return; }
  if (mod && !e.shiftKey && k.toLowerCase() === 'z') { undo(); e.preventDefault(); return; }
  if (mod && (k.toLowerCase() === 'y' || (e.shiftKey && k.toLowerCase() === 'z'))) { redo(); e.preventDefault(); return; }
  if (mod && k.toLowerCase() === 's') { saveFile(); e.preventDefault(); return; }
  if (mod && k.toLowerCase() === 'o') { openFile(); e.preventDefault(); return; }
  if (mod && k.toLowerCase() === 'a') { selectAll(); e.preventDefault(); return; }
  if (mod && k.toLowerCase() === 'd') { duplicateSelection(); e.preventDefault(); return; }
  if (mod) return;
  switch (k) {
    case 'v': case 'V': setTool('select'); break;
    case 'c': case 'C': setTool('cable'); break;
    case 'n': case 'N': setTool('note'); break;
    case 'r': case 'R': setTool('rect'); break;
    case 'o': case 'O': setTool('ellipse'); break;
    case '/': setTool('line'); e.preventDefault(); break;
    case 't': case 'T': templateMarketplace(); break;
    case 'Escape': if (armed || cable || tool !== 'select') setTool('select'); else selectOnly(null); break;
    case 'Delete': case 'Backspace': deleteSelection(); e.preventDefault(); break;
    case 'F2': focusRename(); e.preventDefault(); break;
    case '+': case '=': zoomAt(W / 2, H / 2, 1.2); break;
    case '-': case '_': zoomAt(W / 2, H / 2, 1 / 1.2); break;
    case '0': zoomAt(W / 2, H / 2, 1 / view.z); break;
    case 'f': case 'F': fit(); break;
    case 'l': case 'L': togglePref('labels'); break;
    case 'g': case 'G': togglePref('grid'); break;
    case 'm': case 'M': togglePref('minimap'); break;
    case '`': toggleDock(); break;
    case '.': stepOnce(); break;
    case '?': shortcuts(); break;
  }
});
addEventListener('keyup', e => { if (e.key === ' ') { spaceDown = false; cv.style.cursor = tool === 'select' ? 'default' : 'crosshair'; } });
addEventListener('blur', () => { spaceDown = false; });

/* ════════════════════════════ demo ════════════════════════════ */

function DEMO() {
  return allTemplates().find(t => t.id === 'branch-office').build();
}

/* ════════════════════════════ start ════════════════════════════ */

// best effort: flush running VM disks when the tab goes away (normally they save ~8 s after writes)
// F-22: a pending dirty() debounce must not outlive the page — flush the autosave too
addEventListener('pagehide', () => {
  for (const n of emus.values()) if (n.saveDisk && n.state === 'running') n.saveDisk();
  if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; autosave.save(serialize()); }
});
addEventListener('netlab-storage-blocked', () => toast('Close other netlab tabs — browser storage is locked by an older version. Images and configs are kept in memory until reload.', 'err'));
applyPrefs();
const sp = prefs.get('speed', 2);
$('#speed').value = sp; speed = SPEEDS[sp]; $('#speed-out').textContent = speed + '×';
$('#st-iso').textContent = crossOriginIsolated ? '' : 'Cisco IOS unavailable on this page';
hydrateIcons();
setMode('realtime');
setTool('select');
const saved = autosave.load();
restore(saved?.format === 'netlab' ? saved : DEMO());
$('#save-state').textContent = saved ? 'Restored' : '';
requestAnimationFrame(() => { resize(); fit(); });
requestAnimationFrame(frame);
