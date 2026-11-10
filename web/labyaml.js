// Containerlab-style lab file for the subset netlab writes.
// One document: name, nodes (template, position, ports, startup block), links.

const scalar = s => {
  const t = String(s ?? '');
  if (t === '' || /[:#\n,"'&*!|>%@`]|^\s|\s$/.test(t) || /^(true|false|null|~)$/i.test(t) || /^-?\d/.test(t)) {
    return JSON.stringify(t);
  }
  return t;
};

export function emitLab(data) {
  const out = [`name: ${scalar(data.name || 'lab')}`];
  out.push('nodes:');
  for (const n of data.nodes ?? []) {
    out.push(`  - id: ${n.id}`);
    out.push(`    template: ${scalar(n.tpl || 'pc')}`);
    out.push(`    name: ${scalar(n.name)}`);
    out.push(`    x: ${n.x ?? 0}`);
    out.push(`    y: ${n.y ?? 0}`);
    if (n.powered === false) out.push('    powered: false');
    if (n.startup) {
      out.push('    startup: |');
      for (const line of String(n.startup).split('\n')) out.push(`      ${line}`);
    }
    if (n.startupBoot) out.push('    startup_boot: true');
    const svc = [];
    if (n.svc?.dhcp) svc.push('dhcp');
    if (n.svc?.dns) svc.push('dns');
    if (n.svc?.http) svc.push('http');
    if (svc.length) out.push(`    services: [${svc.join(', ')}]`);
    if (n.resolver) out.push(`    resolver: ${n.resolver}`);
    if ((n.ports ?? []).some(Boolean)) {
      out.push('    ports:');
      for (const p of n.ports) {
        if (!p) { out.push('      - ~'); continue; }
        const bits = [];
        if (p.ip) bits.push(`ip: ${p.ip}`, `mask: ${p.mask}`);
        if (p.nat) bits.push(`nat: ${p.nat}`);
        if (p.trunk) bits.push('mode: trunk');
        if (p.access_vlan) bits.push(`access_vlan: ${p.access_vlan}`);
        if (p.native_vlan) bits.push(`native_vlan: ${p.native_vlan}`);
        if (p.allowed) bits.push(`allowed: ${scalar(p.allowed)}`);
        if (p.helper) bits.push(`helper: ${p.helper}`);
        if (p.in_acl) bits.push(`in_acl: ${p.in_acl}`);
        if (p.out_acl) bits.push(`out_acl: ${p.out_acl}`);
        if (p.ospf_area != null) bits.push(`ospf_area: ${p.ospf_area}`);
        out.push(`      - { ${bits.join(', ')} }`);
      }
    }
    const routes = (n.routes ?? []).filter(r => r.proto !== 'O');
    if (routes.length) {
      out.push('    routes:');
      for (const r of routes) out.push(`      - { prefix: ${r.prefix}, mask: ${r.mask}, next_hop: ${r.next_hop}, out_port: ${r.out_port} }`);
    }
    if (n.ospf?.nets?.length) {
      out.push('    ospf:');
      for (const net of n.ospf.nets) out.push(`      - { prefix: ${net.prefix}, wild: ${net.wild}, area: ${net.area} }`);
    }
    if (n.static_nat?.length) {
      out.push('    static_nat:');
      for (const s of n.static_nat) {
        out.push(`      - { inside: ${s.inside}, proto: ${s.proto || 0}, inside_port: ${s.inside_port || 0}, outside_port: ${s.outside_port || 0} }`);
      }
    }
    if (n.acls?.length) {
      out.push('    acls:');
      for (const a of n.acls) {
        out.push(`      - num: ${a.num}`);
        out.push('        entries:');
        for (const e of a.entries ?? []) {
          out.push(`          - { permit: ${!!e.permit}, proto: ${e.proto}, src: ${e.src}, src_wild: ${e.src_wild}, dst: ${e.dst}, dst_wild: ${e.dst_wild}, sport: ${e.sport || 0}, dport: ${e.dport || 0} }`);
        }
      }
    }
  }
  out.push('links:');
  for (const l of data.links ?? []) {
    const bits = [`a: ${l.a}`, `ap: ${l.ap}`, `b: ${l.b}`, `bp: ${l.bp}`, `delay: ${l.delay ?? 4}`, `jitter: ${l.jitter ?? 0}`, `loss: ${l.loss ?? 0}`, `mtu: ${l.mtu ?? 0}`, `speed: ${l.speed || '1G'}`];
    if (l.up === false) bits.push('up: false');
    out.push(`  - { ${bits.join(', ')} }`);
  }
  return out.join('\n') + '\n';
}

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

/** Parse a `{ k: v, k: v }` line into an object. */
function brace(s) {
  const body = s.trim().replace(/^\{/, '').replace(/\}$/, '').trim();
  const o = {};
  if (!body) return o;
  const parts = [];
  let cur = '', q = '';
  for (const ch of body) {
    if (q) {
      cur += ch;
      if (ch === q) q = '';
      continue;
    }
    if (ch === '"' || ch === "'") { q = ch; cur += ch; continue; }
    if (ch === ',') { parts.push(cur); cur = ''; continue; }
    cur += ch;
  }
  if (cur.trim()) parts.push(cur);
  for (const part of parts) {
    const i = part.indexOf(':');
    if (i < 0) continue;
    o[part.slice(0, i).trim()] = unquote(part.slice(i + 1));
  }
  return o;
}

function flowList(s) {
  return s.replace(/^\[/, '').replace(/\]$/, '').split(',').map(x => unquote(x)).filter(x => x != null && x !== '');
}

/**
 * Indent parser for the document emitLab writes.
 * Returns the same shape as a .netlab.json lab (format netlab).
 */
export function parseLab(text) {
  const lines = String(text).replace(/\r/g, '').split('\n');
  const lab = { format: 'netlab', version: 1, name: 'lab', nodes: [], links: [] };
  let block = null;
  let node = null;
  let acl = null;
  const takeBlock = () => {
    if (!block) return;
    if (node && block.key === 'startup') node.startup = block.lines.join('\n').replace(/\s+$/, '');
    block = null;
  };
  for (const raw of lines) {
    if (block) {
      const ind = raw.match(/^ */)?.[0].length ?? 0;
      if (raw.trim() === '' || ind > block.indent) {
        block.lines.push(raw.slice(Math.min(raw.length, block.content)));
        continue;
      }
      takeBlock();
    }
    if (!raw.trim() || raw.trim().startsWith('#')) continue;
    const s = raw.trim();
    if (!node && s.startsWith('name:')) { lab.name = unquote(s.slice(5)) || 'lab'; continue; }
    if (s === 'nodes:' || s === 'links:') { node = null; acl = null; continue; }
    if (s.startsWith('- id:')) {
      node = { id: unquote(s.slice(5)), ports: [], routes: [], svc: {} };
      lab.nodes.push(node);
      acl = null;
      continue;
    }
    if (!node && s.startsWith('- {') && s.includes('a:')) {
      const l = brace(s.slice(1));
      lab.links.push({ a: +l.a, ap: +l.ap, b: +l.b, bp: +l.bp, up: l.up !== false, delay: +l.delay || 4, jitter: +l.jitter || 0, loss: +l.loss || 0, mtu: +l.mtu || 0, speed: l.speed || '1G' });
      continue;
    }
    if (!node) continue;
    const kv = s.match(/^([A-Za-z0-9_]+):\s*(.*)$/);
    if (kv && !s.startsWith('-')) {
      const [, k, rest] = kv;
      if (rest === '|') {
        const indent = raw.match(/^ */)?.[0].length ?? 0;
        block = { key: k, indent, content: indent + 2, lines: [] };
        continue;
      }
      if (k === 'template') node.tpl = unquote(rest);
      else if (k === 'name') node.name = unquote(rest);
      else if (k === 'x') node.x = +rest;
      else if (k === 'y') node.y = +rest;
      else if (k === 'powered') node.powered = unquote(rest) !== false;
      else if (k === 'startup_boot') node.startupBoot = unquote(rest) === true;
      else if (k === 'resolver') node.resolver = unquote(rest);
      else if (k === 'services') {
        for (const name of flowList(rest)) node.svc[name] = true;
      } else if (k === 'ports' || k === 'routes' || k === 'ospf' || k === 'acls' || k === 'entries' || k === 'static_nat') {
        node._sec = k;
      }
      continue;
    }
    if (s === '- ~') { (node.ports ??= []).push(null); continue; }
    if (s.startsWith('- {')) {
      const o = brace(s.slice(1));
      if (o.prefix && o.wild != null && node._sec === 'ospf') {
        (node.ospf ??= { nets: [] }).nets.push({ prefix: o.prefix, wild: o.wild, area: +o.area || 0 });
      } else if (node._sec === 'static_nat' && o.inside) {
        (node.static_nat ??= []).push({ inside: o.inside, proto: +o.proto || 0, inside_port: +o.inside_port || 0, outside_port: +o.outside_port || 0 });
      } else if (o.prefix && o.next_hop) {
        node.routes.push({ prefix: o.prefix, mask: o.mask, next_hop: o.next_hop, out_port: +o.out_port || 0, proto: 'S' });
      } else if (o.permit != null && o.src) {
        const list = node.acls?.at(-1);
        list?.entries.push({ permit: o.permit === true || o.permit === 'true', proto: +o.proto || 0, src: o.src, src_wild: o.src_wild, dst: o.dst, dst_wild: o.dst_wild, sport: +o.sport || 0, dport: +o.dport || 0 });
      } else {
        const p = {};
        if (o.ip) { p.ip = o.ip; p.mask = o.mask; }
        if (o.nat) p.nat = o.nat;
        if (o.mode === 'trunk') p.trunk = true;
        if (o.access_vlan) p.access_vlan = +o.access_vlan;
        if (o.native_vlan) p.native_vlan = +o.native_vlan;
        if (o.allowed) p.allowed = o.allowed;
        if (o.helper) p.helper = o.helper;
        if (o.in_acl) p.in_acl = +o.in_acl;
        if (o.out_acl) p.out_acl = +o.out_acl;
        if (o.ospf_area != null && o.ospf_area !== '') p.ospf_area = +o.ospf_area;
        node.ports.push(Object.keys(p).length ? p : null);
      }
      continue;
    }
    if (s.startsWith('- num:')) {
      acl = { num: +s.slice(6), entries: [] };
      (node.acls ??= []).push(acl);
      node._sec = 'acls';
    }
  }
  takeBlock();
  return lab;
}
