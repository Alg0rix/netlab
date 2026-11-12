// Lab document codec — the ONLY decode site for the lab document.
// Four places: decodeLab (load), encodeLab (save), parseLab (yaml-import),
// emitLab (yaml-export). Keys are snake_case matching engine names; a
// pre-existing alias is a decode-side alias, never a second write site.
// acl ACEs, ip access-group bindings, nat statics, ip dhcp excluded ranges
// and service dhcp persist through meta.startup replay only — decode seats,
// never emitted. Never persisted: sessions/Cli fields, snaps, errors[],
// OSPF nbr FSM state, derived ospf.router_id, transient uids.

import { tpl } from './icons.js';

/* ── scalar helpers ──────────────────────────────────────────────── */

const scalar = s => {
  const t = String(s ?? '');
  if (t === '' || /[:#\n,"'&*!|>%@`]|^\s|\s$/.test(t) || /^(true|false|null|~)$/i.test(t) || /^-?\d/.test(t)) {
    return JSON.stringify(t);
  }
  return t;
};

/** Emit a bool as bare true/false (F-28), anything else through scalar(). */
const word = v => (typeof v === 'boolean' ? String(v) : scalar(v));

/** Emit an id raw when numeric; scalar() would quote it into a string (F-23). */
const emitId = v => (typeof v === 'number' ? v : scalar(v));

function unquote(s) {
  s = s.trim();
  if (s === '~' || s === 'null' || s === '') return null;
  if (s === 'true') return true;
  if (s === 'false') return false;
  if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) {
    try { return JSON.parse(s.startsWith("'") ? `"${s.slice(1, -1)}"` : s); } catch { return s.slice(1, -1); }
  }
  if (/^-?\d+$/.test(s)) return Number(s);
  return s;
}

// Dotted-path segments that must never be walked — `cur[p] ??= {}` on one
// resolves to Object.prototype and writes onto it. A key carrying one drops
// the whole path: unknown keys are already ignored at the edge.
const DENY_SEGS = new Set(['__proto__', 'constructor', 'prototype']);

// Descend only through OWN members — `cur[p]` on an inherited member
// (`toString`, `hasOwnProperty`, `push`) reaches a prototype and the write
// lands on it. A missing own member gets a fresh {} — and so does a scalar:
// a path that walks through a string has no object to hold it.
const ownStep = (cur, p) =>
  (Object.hasOwn(cur, p) && cur[p] !== null && typeof cur[p] === 'object' ? cur[p] : (cur[p] = {}));

const setPath = (o, key, v) => {
  const parts = String(key).split('.');
  if (parts.some(p => DENY_SEGS.has(p))) return;
  let cur = o;
  for (const p of parts.slice(0, -1)) cur = ownStep(cur, p);
  cur[parts.at(-1)] = v;
};

/** `{` → flow map, `[` → flow list, else scalar. */
const inlineVal = s => {
  s = s.trim();
  return s.startsWith('{') ? brace(s) : s.startsWith('[') ? flowList(s) : unquote(s);
};

/** Parse a `{ k: v, k: v }` line into an object. Depth- and quote-aware. */
function brace(s) {
  const body = s.trim().replace(/^\{/, '').replace(/\}$/, '').trim();
  const o = {};
  if (!body) return o;
  const parts = [];
  let cur = '', q = '', esc = false, depth = 0;
  for (const ch of body) {
    if (q) {
      cur += ch;
      // `\` escapes the next char — a \" inside a quoted scalar is content,
      // not the closing quote.
      if (esc) esc = false;
      else if (ch === '\\') esc = true;
      else if (ch === q) q = '';
      continue;
    }
    if (ch === '"' || ch === "'") { q = ch; cur += ch; continue; }
    if (ch === '{' || ch === '[') depth++;
    if (ch === '}' || ch === ']') depth--;
    if (ch === ',' && depth === 0) { parts.push(cur); cur = ''; continue; }
    cur += ch;
  }
  if (cur.trim()) parts.push(cur);
  for (const part of parts) {
    const i = part.indexOf(':');
    if (i < 0) continue;
    setPath(o, part.slice(0, i).trim(), inlineVal(part.slice(i + 1)));
  }
  return o;
}

function flowList(s) {
  const body = s.trim().replace(/^\[/, '').replace(/\]$/, '');
  const out = [];
  let cur = '', q = '', esc = false, depth = 0;
  for (const ch of body) {
    if (q) {
      cur += ch;
      if (esc) esc = false;
      else if (ch === '\\') esc = true;
      else if (ch === q) q = '';
      continue;
    }
    if (ch === '"' || ch === "'") { q = ch; cur += ch; continue; }
    if (ch === '{' || ch === '[') depth++;
    if (ch === '}' || ch === ']') depth--;
    if (ch === ',' && depth === 0) { out.push(inlineVal(cur)); cur = ''; continue; }
    cur += ch;
  }
  if (cur.trim()) out.push(inlineVal(cur));
  return out;
}

/* ── codec tables — the §6 field table is the contract ───────────── */

const kindOf = n => {
  const k = tpl(n?.tpl)?.kind ?? n?.kind ?? 'host';
  return k === 'router' ? 'router' : k === 'switch' ? 'switch' : 'host';
};

const TOGGLE_KEYS = ['domain_lookup', 'proxy_arp', 'directed_broadcast', 'ip_routing', 'routing_enabled'];
const TOGGLE_DEFAULT = {
  router: { domain_lookup: true, proxy_arp: true, directed_broadcast: false, ip_routing: true, routing_enabled: true },
  switch: { domain_lookup: true, proxy_arp: true, directed_broadcast: false, ip_routing: false, routing_enabled: true },
  host: { domain_lookup: true, proxy_arp: false, directed_broadcast: false, ip_routing: false, routing_enabled: false },
};
const STATS_KEYS = ['rx', 'tx', 'rx_err', 'tx_err', 'drops', 'cleared_tick'];
const AD_DEFAULT = { connected: 0, static: 1, ospf: 110 };
const PROTO_WORD = { c: 'connected', connected: 'connected', s: 'static', static: 'static', o: 'ospf', ospf: 'ospf' };
const METHODS = ['none', 'manual', 'dhcp'];
const VLAN_SEED = { 1: 'default', 1002: 'fddi-default', 1003: 'token-ring', 1004: 'fddinet', 1005: 'trnet' };
const vlanNameDefault = id => VLAN_SEED[id] ?? `VLAN${String(id).padStart(4, '0')}`;

const u = v => { const n = +v; return Number.isFinite(n) && n > 0 ? Math.floor(n) : 0; };
const boolOf = (v, d) => (v === true || v === 'true' ? true : v === false || v === 'false' ? false : d);
const arr = v => (Array.isArray(v) ? v : []);

/** Fold dotted keys (`meta.startup`, `stats.rx`, `vlans.10.name`) into nested seats. */
const undot = o => {
  const out = {};
  for (const [k, v] of Object.entries(o ?? {})) {
    const parts = k.split('.');
    if (parts.some(p => DENY_SEGS.has(p))) continue;
    if (parts.length === 1) { out[k] = v; continue; }
    let cur = out;
    for (const p of parts.slice(0, -1)) cur = ownStep(cur, p);
    cur[parts.at(-1)] = v;
  }
  return out;
};

/* ── decode — raw document seat → canonical resolved seat ────────── */

const decodeMeta = n => {
  const m = n.meta && typeof n.meta === 'object' ? n.meta : {};
  const st = m.startup ?? n.startup;
  const startup = st == null || typeof st === 'object' ? undefined : typeof st === 'string' ? st : String(st);
  return { startup, startupBoot: boolOf(m.startupBoot ?? m.startup_boot ?? n.startupBoot ?? n.startup_boot, false) };
};

const decodeSvc = n => {
  let s = n.svc ?? n.services;
  if (Array.isArray(s)) { const o = {}; for (const x of s) o[x] = true; s = o; }
  if (!s || typeof s !== 'object') return null;
  const svc = { dhcp: boolOf(s.dhcp, false), dns: boolOf(s.dns, false), http: boolOf(s.http, false) };
  return svc.dhcp || svc.dns || svc.http ? svc : null;
};

const decodeDns = list => arr(list).map(r => {
  const name = Array.isArray(r) ? r[0] : r?.name;
  const ip = Array.isArray(r) ? r[1] : r?.ip;
  return name != null && ip != null ? [String(name), String(ip)] : null;
}).filter(Boolean);

const decodePort = (p, kind) => {
  if (p == null || typeof p !== 'object') return null;
  p = undot(p);
  const st = p.stats && typeof p.stats === 'object' ? p.stats : {};
  const port = {};
  if (p.ip != null && p.ip !== '') { port.ip = String(p.ip); if (p.mask != null) port.mask = String(p.mask); }
  if (p.nat != null && p.nat !== '') port.nat = p.nat;
  if (p.trunk === true || p.trunk === 'true' || p.mode === 'trunk') port.trunk = true;
  if (p.access_vlan != null) port.access_vlan = u(p.access_vlan) || 1;
  if (p.native_vlan != null) port.native_vlan = u(p.native_vlan) || 1;
  if (p.allowed != null && p.allowed !== '') port.allowed = Array.isArray(p.allowed) ? p.allowed.join(',') : String(p.allowed);
  if (p.helper != null && p.helper !== '') port.helper = String(p.helper);
  if (p.in_acl != null) port.in_acl = u(p.in_acl);
  if (p.out_acl != null) port.out_acl = u(p.out_acl);
  if (p.ospf_area != null && p.ospf_area !== '') port.ospf_area = u(p.ospf_area);
  port.admin_up = boolOf(p.admin_up ?? p.adminUp, true);
  port.method = METHODS.includes(p.method) ? p.method : 'none';
  port.stats = {};
  for (const k of STATS_KEYS) port.stats[k] = u(st[k] ?? p[k]);
  return port;
};

const decodeRoute = r => {
  if (!r || r.prefix == null || r.next_hop == null) return null;
  const proto = PROTO_WORD[String(r.proto ?? 'static').toLowerCase()] ?? 'static';
  return {
    prefix: String(r.prefix), mask: r.mask != null ? String(r.mask) : r.mask,
    next_hop: String(r.next_hop), out_port: u(r.out_port),
    proto, ad: r.ad == null ? AD_DEFAULT[proto] : u(r.ad),
    metric: u(r.metric), learned_tick: u(r.learned_tick),
  };
};

const decodeMac = r => (r && r.mac != null ? {
  mac: String(r.mac), port: u(r.port), vlan: u(r.vlan) || 1,
  created_tick: u(r.created_tick), etype: r.etype === 'static' ? 'static' : 'dynamic',
} : null);

const decodeArp = r => (r && r.ip != null && r.mac != null ? {
  ip: String(r.ip), mac: String(r.mac), port: u(r.port),
  created_tick: u(r.created_tick), etype: r.etype === 'static' ? 'static' : 'dynamic',
} : null);

const decodeVlans = raw => {
  const rows = {};
  for (const [id, name] of Object.entries(VLAN_SEED)) rows[id] = { name, state: 'active' };
  const put = (id, v) => {
    const vid = u(id);
    if (vid < 1 || vid > 4094) return;
    rows[vid] = {
      name: v && v.name != null ? String(v.name) : vlanNameDefault(vid),
      state: v && v.state === 'suspend' ? 'suspend' : 'active',
    };
  };
  if (Array.isArray(raw)) for (const v of raw) v && put(v.id ?? v.vid, v);
  else if (raw && typeof raw === 'object') for (const [id, v] of Object.entries(raw)) put(id, v);
  return rows;
};

const decodeToggles = (raw, kind) => {
  const d = TOGGLE_DEFAULT[kind], t = {};
  for (const k of TOGGLE_KEYS) t[k] = boolOf(raw?.[k], d[k]);
  return t;
};

const decodeOspf = raw => {
  if (!raw || typeof raw !== 'object') return null;
  const pid0 = raw.pid ?? raw.id;
  if (pid0 == null) return null;
  const rid = raw.router_id;
  const router_id = typeof rid === 'string' || typeof rid === 'number'
    ? { value: String(rid), explicit: true }
    : { value: String(rid?.value ?? '0.0.0.0'), explicit: rid?.explicit === true };
  const networks = arr(raw.networks ?? raw.nets)
    .map(r => (r && (r.net ?? r.prefix) != null && r.wild != null
      ? { net: String(r.net ?? r.prefix), wild: String(r.wild), area: u(r.area) }
      : null))
    .filter(Boolean);
  const pid = u(pid0);
  return {
    pid, router_id, networks,
    id: pid,                                                    // legacy mirror for restore()
    nets: networks.map(r => ({ prefix: r.net, wild: r.wild, area: r.area })),
  };
};

const decodeAcls = list => arr(list).map(a => (a ? {
  num: u(a.num),
  entries: arr(a.entries).map(e => ({
    permit: e?.permit === true || e?.permit === 'true' || e?.permit === 'permit',
    proto: e?.proto ?? 0,
    src: e?.src, src_wild: e?.src_wild, dst: e?.dst, dst_wild: e?.dst_wild,
    sport: u(e?.sport), dport: u(e?.dport),
  })),
} : null)).filter(Boolean);

const decodeNat = list => arr(list).map(s => (s && s.inside != null ? {
  inside: s.inside, proto: s.proto ?? 0, inside_port: u(s.inside_port), outside_port: u(s.outside_port),
} : null)).filter(Boolean);

const decodeLink = l => (l ? {
  a: l.a, ap: u(l.ap ?? l.a_port), b: l.b, bp: u(l.bp ?? l.b_port),
  up: boolOf(l.up, true),
  delay: l.delay == null ? 4 : u(l.delay), jitter: u(l.jitter), loss: u(l.loss), mtu: u(l.mtu),
  speed: l.speed || '1G',
} : null);

const decodeNote = n => (n ? { id: u(n.id), x: +n.x || 0, y: +n.y || 0, text: n.text ?? '' } : null);

const decodeShape = sh => {
  if (!sh) return null;
  const o = { id: u(sh.id), kind: sh.kind, x: +sh.x || 0, y: +sh.y || 0, w: +sh.w || 0, h: +sh.h || 0 };
  if (sh.stroke != null) o.stroke = sh.stroke;
  if (sh.fill != null) o.fill = sh.fill;
  if (sh.dash != null) o.dash = sh.dash;
  if (sh.width) o.width = +sh.width || 0;
  if (sh.text != null) o.text = sh.text;
  return o;
};

const decodeView = v => ({ x: +v.x || 0, y: +v.y || 0, z: +v.z || 1 });

const decodeNode = n0 => {
  const n = undot(n0 ?? {});
  const kind = kindOf(n);
  const meta = decodeMeta(n);
  const node = {
    id: n.id, tpl: n.tpl ?? n.template, name: n.name ?? '', x: +n.x || 0, y: +n.y || 0,
    powered: boolOf(n.powered, true),
    meta,
    startup: meta.startup, startupBoot: meta.startupBoot,
  };
  if (n.uid != null) node.uid = n.uid;
  if (n.image != null) node.image = n.image;
  if (n.portCount != null) node.portCount = u(n.portCount);
  if (n.vm != null) node.vm = n.vm;
  if (n.nvram != null) node.nvram = n.nvram;
  const svc = decodeSvc(n);
  if (svc) node.svc = svc;
  if (n.resolver != null && n.resolver !== '') node.resolver = String(n.resolver);
  const dns = decodeDns(n.dns);
  if (dns.length) node.dns = dns;
  node.ports = arr(n.ports).map(p => decodePort(p, kind));
  node.routes = arr(n.routes).map(decodeRoute).filter(Boolean);
  node.mac = arr(n.mac ?? n.mac_table).map(decodeMac).filter(Boolean);
  node.arp = arr(n.arp ?? n.arp_table ?? n.arp_cache).map(decodeArp).filter(Boolean);
  if (kind === 'switch') node.vlans = decodeVlans(n.vlans);
  node.toggles = decodeToggles(n.toggles, kind);
  const ospf = decodeOspf(n.ospf);
  if (ospf) node.ospf = ospf;
  if (n.acls != null) node.acls = decodeAcls(n.acls);
  if (n.static_nat != null) node.static_nat = decodeNat(n.static_nat);
  return node;
};

/** load — raw document object → canonical lab document. */
export function decodeLab(data) {
  if (typeof data === 'string') { try { data = JSON.parse(data); } catch { data = null; } }
  const lab = {
    format: data?.format ?? 'netlab',
    version: data?.version ?? 1,
    name: data?.name ?? 'lab',
    nodes: arr(data?.nodes).map(decodeNode),
    links: arr(data?.links).map(decodeLink).filter(Boolean),
    notes: arr(data?.notes).map(decodeNote).filter(Boolean),
    shapes: arr(data?.shapes).map(decodeShape).filter(Boolean),
  };
  if (data?.view) lab.view = decodeView(data.view);
  return lab;
}

// Canonical decoded node → the apply_device_fields seat object: every codec
// seat the bypass setters cannot reach, in one grouped call. Routes carry
// static rows only — connected routes re-derive, OSPF routes relearn.
export const applySeats = n => ({
  ports: arr(n.ports).map((p, index) => p ? {
    index, admin_up: p.admin_up, method: p.method, stats: p.stats,
  } : null).filter(Boolean),
  routes: arr(n.routes).filter(r => r.proto === 'static'),
  mac: arr(n.mac),
  arp: arr(n.arp),
  vlans: n.vlans ?? {},
  toggles: n.toggles ?? {},
  ospf: n.ospf ? {
    pid: n.ospf.pid,
    router_id: n.ospf.router_id?.explicit === true ? n.ospf.router_id.value : null,
    networks: arr(n.ospf.networks),
  } : null,
});

/* ── encode — canonical seat → persisted seat (defaults suppressed) ── */

const encodePort = (p, kind) => {
  const d = decodePort(p, kind);
  if (d == null) return null;
  const o = {};
  if (d.ip != null) { o.ip = d.ip; if (d.mask != null) o.mask = d.mask; }
  if (d.nat) o.nat = d.nat;
  if (d.trunk) o.trunk = true;
  if (d.access_vlan != null && d.access_vlan !== 1) o.access_vlan = d.access_vlan;
  if (d.native_vlan != null && d.native_vlan !== 1) o.native_vlan = d.native_vlan;
  if (d.allowed != null) o.allowed = d.allowed;
  if (d.helper != null) o.helper = d.helper;
  if (d.ospf_area != null) o.ospf_area = d.ospf_area;
  if (d.admin_up === false) o.admin_up = false;
  if (d.method !== 'none') o.method = d.method;
  const st = {};
  for (const k of STATS_KEYS) if (d.stats[k] !== 0) st[k] = d.stats[k];
  if (Object.keys(st).length) o.stats = st;
  return o;   // {} is a live seat — every member rode to its default
};

const encodeVlans = raw => {
  const decoded = decodeVlans(raw);
  const out = {};
  for (const id of Object.keys(decoded).sort((a, b) => +a - +b)) {
    const v = decoded[id];
    if (VLAN_SEED[id] != null && v.name === VLAN_SEED[id] && v.state === 'active') continue;
    const row = {};
    if (v.name !== vlanNameDefault(+id)) row.name = v.name;
    if (v.state !== 'active') row.state = v.state;
    out[id] = row;   // {} marks the seat — vlan exists at defaults
  }
  return Object.keys(out).length ? out : undefined;
};

// Exported for main.js serialize(): the same seat emit + suppression rules
// drive .netlab.json, so a saved document never drifts from the YAML form.
export const encodeNode = n0 => {
  const n = undot(n0 ?? {});
  const kind = kindOf(n);
  const out = { id: n.id, tpl: n.tpl ?? n.template, name: n.name ?? '', x: +n.x || 0, y: +n.y || 0 };
  if (n.powered === false || n.powered === 'false') out.powered = false;
  if (n.uid != null && n.uid !== '') out.uid = n.uid;
  if (n.image != null && n.image !== '') out.image = n.image;
  if (n.portCount != null) out.portCount = u(n.portCount);
  if (n.vm != null) out.vm = n.vm;
  if (n.nvram != null && n.nvram !== '') out.nvram = n.nvram;
  const meta = decodeMeta(n);
  const m = {};
  if (typeof meta.startup === 'string' && meta.startup !== '') m.startup = meta.startup;
  if (meta.startupBoot === true) m.startupBoot = true;
  if (Object.keys(m).length) out.meta = m;
  const svc = decodeSvc(n);
  if (svc) {
    const s = {};
    if (svc.dns) s.dns = true;
    if (svc.http) s.http = true;
    if (Object.keys(s).length) out.svc = s;   // service dhcp persists via meta.startup only
  }
  if (n.resolver != null && n.resolver !== '') out.resolver = String(n.resolver);
  const dns = decodeDns(n.dns);
  if (dns.length) out.dns = dns;
  const ports = arr(n.ports).map(p => encodePort(p, kind));
  if (ports.some(p => p != null)) out.ports = ports;
  const routes = arr(n.routes).map(decodeRoute).filter(r => r?.proto === 'static').map(r => {
    const o = { prefix: r.prefix };
    if (r.mask != null) o.mask = r.mask;
    o.next_hop = r.next_hop; o.out_port = r.out_port; o.ad = r.ad;
    if (r.metric) o.metric = r.metric;
    if (r.learned_tick) o.learned_tick = r.learned_tick;
    return o;
  });
  if (routes.length) out.routes = routes;
  const mac = arr(n.mac ?? n.mac_table).map(decodeMac).filter(Boolean).map(r => {
    const o = { mac: r.mac, port: r.port, vlan: r.vlan };
    if (r.created_tick) o.created_tick = r.created_tick;
    if (r.etype !== 'dynamic') o.etype = r.etype;
    return o;
  });
  if (mac.length) out.mac = mac;
  const arp = arr(n.arp ?? n.arp_table ?? n.arp_cache).map(decodeArp).filter(Boolean).map(r => {
    const o = { ip: r.ip, mac: r.mac, port: r.port };
    if (r.created_tick) o.created_tick = r.created_tick;
    if (r.etype !== 'dynamic') o.etype = r.etype;
    return o;
  });
  if (arp.length) out.arp = arp;
  if (kind === 'switch') {
    const vlans = encodeVlans(n.vlans);
    if (vlans) out.vlans = vlans;
  }
  const td = decodeToggles(n.toggles, kind), tg = {};
  for (const k of TOGGLE_KEYS) if (td[k] !== TOGGLE_DEFAULT[kind][k]) tg[k] = td[k];
  if (Object.keys(tg).length) out.toggles = tg;
  const ospf = decodeOspf(n.ospf);
  if (ospf) {
    const o = { pid: ospf.pid };
    if (ospf.router_id.explicit === true) o.router_id = ospf.router_id.value;
    if (ospf.networks.length) o.networks = ospf.networks.map(r => ({ net: r.net, wild: r.wild, area: r.area }));
    out.ospf = o;
  }
  return out;
};

const encodeLink = d => {
  const o = { a: d.a, ap: d.ap, b: d.b, bp: d.bp };
  if (d.up === false) o.up = false;
  o.delay = d.delay; o.jitter = d.jitter; o.loss = d.loss; o.mtu = d.mtu; o.speed = d.speed;
  return o;
};

/** save — document → canonical persisted document (stable key order). */
export function encodeLab(data) {
  const doc = {
    format: 'netlab', version: 1, name: data?.name || 'lab',
    nodes: arr(data?.nodes).map(encodeNode),
    links: arr(data?.links).map(decodeLink).filter(Boolean).map(encodeLink),
    notes: arr(data?.notes).map(decodeNote).filter(Boolean),
    shapes: arr(data?.shapes).map(decodeShape).filter(Boolean),
  };
  if (data?.view) doc.view = decodeView(data.view);
  return doc;
}

/* ── yaml-export ─────────────────────────────────────────────────── */

export function emitLab(data) {
  const doc = encodeLab(data);
  const out = [`name: ${scalar(doc.name || 'lab')}`];
  out.push('nodes:');
  for (const n of doc.nodes) {
    out.push(`  - id: ${emitId(n.id)}`);
    out.push(`    template: ${scalar(n.tpl || 'pc')}`);
    out.push(`    name: ${scalar(n.name)}`);
    out.push(`    x: ${n.x ?? 0}`);
    out.push(`    y: ${n.y ?? 0}`);
    if (n.powered === false) out.push('    powered: false');
    if (n.uid) out.push(`    uid: ${scalar(n.uid)}`);
    if (n.image) out.push(`    image: ${scalar(n.image)}`);
    if (n.portCount != null) out.push(`    portCount: ${emitId(n.portCount)}`);
    if (n.vm) out.push(`    vm: { ram: ${emitId(n.vm.ram)}, diskMB: ${emitId(n.vm.diskMB)}, nic: ${scalar(n.vm.nic)}, nics: ${emitId(n.vm.nics)} }`);
    if (n.nvram) out.push(`    nvram: ${scalar(n.nvram)}`);
    if (n.meta?.startup) {
      const st = String(n.meta.startup);
      // |- strips the final newline; a text that ends in \n keeps it via |+.
      const keep = st.endsWith('\n');
      out.push(keep ? '    meta.startup: |+' : '    meta.startup: |-');
      for (const line of (keep ? st.slice(0, -1) : st).split('\n')) out.push(`      ${line}`);
    }
    if (n.meta?.startupBoot === true) out.push('    meta.startupBoot: true');
    const svc = ['dns', 'http'].filter(k => n.svc?.[k]);
    if (svc.length) out.push(`    services: [${svc.join(', ')}]`);
    if (n.resolver) out.push(`    resolver: ${scalar(n.resolver)}`);
    if (n.dns?.length) {
      out.push('    dns:');
      for (const [nm, ip] of n.dns) out.push(`      - { name: ${scalar(nm)}, ip: ${scalar(ip)} }`);
    }
    if (n.ports?.length) {
      out.push('    ports:');
      for (const p of n.ports) {
        if (p == null) { out.push('      - ~'); continue; }
        const bits = [];
        if (p.ip) { bits.push(`ip: ${scalar(p.ip)}`); if (p.mask != null) bits.push(`mask: ${scalar(p.mask)}`); }
        if (p.nat) bits.push(`nat: ${scalar(p.nat)}`);
        if (p.trunk) bits.push('mode: trunk');
        if (p.access_vlan != null) bits.push(`access_vlan: ${emitId(p.access_vlan)}`);
        if (p.native_vlan != null) bits.push(`native_vlan: ${emitId(p.native_vlan)}`);
        if (p.allowed != null) bits.push(`allowed: ${scalar(p.allowed)}`);
        if (p.helper != null) bits.push(`helper: ${scalar(p.helper)}`);
        if (p.ospf_area != null) bits.push(`ospf_area: ${emitId(p.ospf_area)}`);
        if (p.admin_up != null) bits.push(`admin_up: ${word(p.admin_up)}`);
        if (p.method != null) bits.push(`method: ${scalar(p.method)}`);
        for (const k of STATS_KEYS) if (p.stats?.[k] != null) bits.push(`stats.${k}: ${emitId(p.stats[k])}`);
        out.push(`      - {${bits.length ? ` ${bits.join(', ')} ` : ''}}`);
      }
    }
    if (n.routes?.length) {
      out.push('    routes:');
      for (const r of n.routes) {
        const b = [`prefix: ${scalar(r.prefix)}`];
        if (r.mask != null) b.push(`mask: ${scalar(r.mask)}`);
        b.push(`next_hop: ${scalar(r.next_hop)}`, `out_port: ${emitId(r.out_port)}`, `ad: ${emitId(r.ad)}`);
        if (r.metric) b.push(`metric: ${emitId(r.metric)}`);
        if (r.learned_tick) b.push(`learned_tick: ${emitId(r.learned_tick)}`);
        out.push(`      - { ${b.join(', ')} }`);
      }
    }
    if (n.mac?.length) {
      out.push('    mac:');
      for (const r of n.mac) {
        const b = [`mac: ${scalar(r.mac)}`, `port: ${emitId(r.port)}`, `vlan: ${emitId(r.vlan)}`];
        if (r.created_tick) b.push(`created_tick: ${emitId(r.created_tick)}`);
        if (r.etype !== 'dynamic') b.push(`etype: ${scalar(r.etype)}`);
        out.push(`      - { ${b.join(', ')} }`);
      }
    }
    if (n.arp?.length) {
      out.push('    arp:');
      for (const r of n.arp) {
        const b = [`ip: ${scalar(r.ip)}`, `mac: ${scalar(r.mac)}`, `port: ${emitId(r.port)}`];
        if (r.created_tick) b.push(`created_tick: ${emitId(r.created_tick)}`);
        if (r.etype !== 'dynamic') b.push(`etype: ${scalar(r.etype)}`);
        out.push(`      - { ${b.join(', ')} }`);
      }
    }
    if (n.vlans) {
      for (const id of Object.keys(n.vlans).sort((a, b) => +a - +b)) {
        const v = n.vlans[id];
        if (!Object.keys(v).length) { out.push(`    vlans.${id}: {}`); continue; }
        for (const k of ['name', 'state']) if (v[k] != null) out.push(`    vlans.${id}.${k}: ${word(v[k])}`);
      }
    }
    if (n.toggles) for (const k of TOGGLE_KEYS) if (n.toggles[k] != null) out.push(`    toggles.${k}: ${n.toggles[k]}`);
    if (n.ospf) {
      out.push(`    ospf.pid: ${emitId(n.ospf.pid)}`);
      if (n.ospf.router_id != null) out.push(`    ospf.router_id: ${scalar(n.ospf.router_id)}`);
      if (n.ospf.networks?.length) {
        out.push('    ospf.networks:');
        for (const r of n.ospf.networks) out.push(`      - { net: ${scalar(r.net)}, wild: ${scalar(r.wild)}, area: ${emitId(r.area)} }`);
      }
    }
  }
  out.push('links:');
  for (const l of doc.links ?? []) {
    const bits = [`a: ${emitId(l.a)}`, `ap: ${emitId(l.ap)}`, `b: ${emitId(l.b)}`, `bp: ${emitId(l.bp)}`, `delay: ${emitId(l.delay ?? 4)}`, `jitter: ${emitId(l.jitter ?? 0)}`, `loss: ${emitId(l.loss ?? 0)}`, `mtu: ${emitId(l.mtu ?? 0)}`, `speed: ${scalar(l.speed || '1G')}`];
    if (l.up === false) bits.push('up: false');
    out.push(`  - { ${bits.join(', ')} }`);
  }
  if (doc.view) out.push(`view: { x: ${doc.view.x ?? 0}, y: ${doc.view.y ?? 0}, z: ${doc.view.z ?? 1} }`);
  if (doc.notes?.length) {
    out.push('notes:');
    for (const n of doc.notes) out.push(`  - { id: ${n.id}, x: ${n.x ?? 0}, y: ${n.y ?? 0}, text: ${scalar(n.text)} }`);
  }
  if (doc.shapes?.length) {
    out.push('shapes:');
    for (const sh of doc.shapes) {
      const bits = [`id: ${sh.id}`, `kind: ${scalar(sh.kind)}`, `x: ${sh.x ?? 0}`, `y: ${sh.y ?? 0}`, `w: ${sh.w ?? 0}`, `h: ${sh.h ?? 0}`];
      if (sh.stroke) bits.push(`stroke: ${scalar(sh.stroke)}`);
      if (sh.fill) bits.push(`fill: ${word(sh.fill)}`);
      if (sh.dash) bits.push(`dash: ${word(sh.dash)}`);
      if (sh.width) bits.push(`width: ${sh.width}`);
      if (sh.text) bits.push(`text: ${scalar(sh.text)}`);
      out.push(`  - { ${bits.join(', ')} }`);
    }
  }
  return out.join('\n') + '\n';
}

/* ── yaml-import ─────────────────────────────────────────────────── */

function blockText(b) {
  const ls = b.lines.slice();
  let trail = 0;
  while (ls.length && ls.at(-1).trim() === '') { ls.pop(); trail++; }
  let body;
  if (b.style === '>') {
    body = '';
    let blank = 0;
    for (const l of ls) {
      if (l.trim() === '') { blank++; continue; }
      body += (body === '' ? '' : blank > 0 ? '\n'.repeat(blank + 1) : ' ') + l;
      blank = 0;
    }
    trail = blank;
  } else {
    body = ls.join('\n');
  }
  if (b.chomp === '-') return body;
  if (b.chomp === '+') return body + (body || trail ? '\n'.repeat(trail + (body ? 1 : 0)) : '');
  return body === '' ? '' : body + '\n';
}

/**
 * Indent parser for the document emitLab writes plus tolerant decode of
 * nested-map and flat-key aliases. Returns the canonical lab document.
 */
export function parseLab(text) {
  // Split lines, then drop a single trailing \r — a CRLF document still
  // parses — but a \r inside a line belongs to the payload: the startup
  // scalar round-trips byte-verbatim.
  const lines = String(text).split('\n').map(l => (l.endsWith('\r') ? l.slice(0, -1) : l));
  const lab = { format: 'netlab', version: 1, name: 'lab', nodes: [], links: [], notes: [], shapes: [] };
  let node = null, acl = null, topSec = null;
  let sec = null, sub = null, vrow = null, block = null;
  const setSeat = (path, v) => setPath(node, path, v);
  const closeBlock = () => {
    if (!block) return;
    setSeat(block.path, blockText(block));
    block = null;
  };
  const blockPath = k => (k === 'startup' ? 'meta.startup' : k);
  for (const raw of lines) {
    const ind = raw.match(/^ */)?.[0].length ?? 0;
    if (block) {
      if (raw.trim() === '' || ind > block.indent) {
        block.lines.push(raw.slice(Math.min(raw.length, block.indent + 2)));
        continue;
      }
      closeBlock();
    }
    if (!raw.trim() || raw.trim().startsWith('#')) continue;
    const s = raw.trim();
    if (!node && s.startsWith('name:')) { lab.name = unquote(s.slice(5)) || 'lab'; continue; }
    if (s === 'nodes:' || s === 'links:' || s === 'notes:' || s === 'shapes:') {
      node = null; acl = null; sec = sub = vrow = null; topSec = s.slice(0, -1); continue;
    }
    if (!node && s.startsWith('view:')) {
      lab.view = brace(s.slice(5));
      lab.view.x = +lab.view.x || 0; lab.view.y = +lab.view.y || 0; lab.view.z = +lab.view.z || 1;
      continue;
    }
    if (s.startsWith('- id:')) {
      node = { id: unquote(s.slice(5)), ports: [], routes: [] };
      lab.nodes.push(node);
      acl = null; sec = sub = vrow = null;
      continue;
    }
    if (!node) {
      if (s.startsWith('- {')) {
        const o = brace(s.slice(1));
        if (topSec === 'notes') lab.notes.push(o);
        else if (topSec === 'shapes') lab.shapes.push(o);
        else lab.links.push(o);
      }
      continue;
    }
    // ── node context ──
    const isRow = s.startsWith('- ');
    if (sec && (ind < sec.indent || (ind === sec.indent && !isRow))) { sec = null; sub = null; vrow = null; }
    if (sub && (ind < sub.indent || (ind === sub.indent && !isRow))) sub = null;
    if (vrow && ind <= vrow.indent) vrow = null;
    if (isRow) {
      const key = sub?.key ?? sec?.key;
      if (s === '- ~') { if (key === 'ports') node.ports.push(null); continue; }
      if (s.startsWith('- num:') && key === 'acls') {
        acl = { num: +s.slice(6), entries: [] };
        (node.acls ??= []).push(acl);
        sub = { key: 'acl-item', indent: ind };
        continue;
      }
      if (s.startsWith('- {')) {
        const o = brace(s.slice(1));
        switch (key) {
          case 'entries': acl?.entries.push(o); break;                 // F-26 — entries sit under the entries: gate only
          case 'acls': if (o.num != null) { acl = { num: +o.num || 0, entries: [] }; (node.acls ??= []).push(acl); for (const e of arr(o.entries)) acl.entries.push(e); } break;
          case 'ospf': case 'ospf.networks': case 'networks': case 'nets':
            ((node.ospf ??= {}).networks ??= []).push(o); break;
          case 'ports': node.ports.push(o); break;
          case 'routes': node.routes.push(o); break;
          case 'mac': case 'mac_table': (node.mac ??= []).push(o); break;
          case 'arp': case 'arp_table': case 'arp_cache': (node.arp ??= []).push(o); break;
          case 'dns': (node.dns ??= []).push(o); break;
          case 'static_nat': (node.static_nat ??= []).push(o); break;
          default: break;                                            // rows under unknown seats ignored — never leak into ports
        }
        continue;
      }
      if (s.startsWith('- [')) {
        const arr = flowList(s.slice(1));
        if (key === 'dns' && arr.length >= 2) (node.dns ??= []).push([arr[0], arr[1]]);
        continue;
      }
      if (key === 'services') { (node.svc ??= {})[unquote(s.slice(2))] = true; }
      continue;
    }
    const kv = s.match(/^([A-Za-z0-9_.]+):(?:[ \t]+(.*))?$/);
    if (!kv) continue;
    const [, k, rest0] = kv;
    const rest = (rest0 ?? '').trim();
    if (vrow && ind > vrow.indent) {
      if (k === 'name' || k === 'state') { vrow.row[k] = unquote(rest); }
      continue;
    }
    if (sec && ind > sec.indent) {
      if (sec.key === 'meta') {
        if (k === 'startup') {
          if (/^[|>][+-]?$/.test(rest)) block = { path: 'meta.startup', indent: ind, style: rest[0], chomp: rest[1] ?? '', lines: [] };
          else setSeat('meta.startup', unquote(rest));
        } else if (k === 'startupBoot' || k === 'startup_boot') setSeat('meta.startupBoot', unquote(rest));
      } else if (sec.key === 'toggles') {
        (node.toggles ??= {})[k] = unquote(rest);
      } else if (sec.key === 'ospf') {
        if (k === 'pid' || k === 'id') (node.ospf ??= {}).pid = +rest || 0;
        else if (k === 'router_id') (node.ospf ??= {}).router_id = unquote(rest);
        else if (k === 'networks' || k === 'nets') sub = { key: 'ospf.networks', indent: ind };
      } else if (sec.key === 'vlans') {
        if (/^\d+$/.test(k)) {
          const row = rest.startsWith('{') ? brace(rest) : {};
          vrow = { row, indent: ind };
          (node.vlans ??= {})[k] = row;
        }
      } else if (sec.key === 'acls' && k === 'entries') {
        sub = { key: 'entries', indent: ind };
      }
      continue;   // unknown children of a known section are ignored
    }
    if (/^[|>][+-]?$/.test(rest)) {
      block = { path: blockPath(k), indent: ind, style: rest[0], chomp: rest[1] ?? '', lines: [] };
      continue;
    }
    if (k.includes('.')) {
      const base = k.split('.')[0];
      if (base === 'meta' || base === 'toggles' || base === 'ospf' || base === 'vlans') {
        if (rest === '') {
          if (k === 'ospf.networks' || k === 'ospf.nets') sec = { key: 'ospf.networks', indent: ind };
          else if (base === 'vlans') setSeat(k, {});
        } else setSeat(k, inlineVal(rest));
      }
      continue;   // unknown dotted key — ignored at the edge
    }
    switch (k) {
      case 'uid': node.uid = unquote(rest); break;
      case 'image': node.image = unquote(rest); break;
      case 'nvram': node.nvram = unquote(rest); break;
      case 'portCount': node.portCount = +rest || 0; break;
      case 'vm': { const v = brace(rest); node.vm = { ram: +v.ram || 0, diskMB: +v.diskMB || 0, nic: v.nic, nics: +v.nics || 0 }; break; }
      case 'template': node.tpl = unquote(rest); break;
      case 'name': node.name = unquote(rest); break;
      case 'x': node.x = +rest; break;
      case 'y': node.y = +rest; break;
      case 'powered': node.powered = boolOf(unquote(rest), true); break;
      case 'startup': setSeat('meta.startup', unquote(rest)); break;
      case 'startupBoot': case 'startup_boot': setSeat('meta.startupBoot', unquote(rest)); break;
      case 'meta': case 'toggles': case 'ospf': case 'vlans':
        if (rest === '') sec = { key: k, indent: ind };
        else setSeat(k, inlineVal(rest));
        break;
      case 'services':
        if (rest === '') sec = { key: 'services', indent: ind };
        else for (const nm of rest.startsWith('[') ? flowList(rest) : [unquote(rest)]) (node.svc ??= {})[nm] = true;
        break;
      case 'svc':
        if (rest === '') sec = { key: 'services', indent: ind };
        else node.svc = inlineVal(rest);
        break;
      case 'resolver': node.resolver = unquote(rest); break;
      case 'ports': case 'routes': case 'mac': case 'mac_table': case 'arp': case 'arp_table':
      case 'arp_cache': case 'dns': case 'static_nat': case 'acls': case 'entries':
      case 'ospf.networks': case 'networks': case 'nets': case 'acl':
        if (rest === '') sec = { key: k, indent: ind };
        break;
      default: break;   // unknown key — ignored at the edge
    }
  }
  closeBlock();
  return decodeLab(lab);
}
