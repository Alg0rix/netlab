// Interactive shell for simulated nodes. Hosts get a VPCS-flavoured shell,
// routers/switches a Cisco-flavoured one (with abbreviations: "sh ip int br").

import { TICK_MS } from './capture.js';

const WORDS = ['show', 'interface', 'brief', 'route', 'arp', 'address', 'address-table', 'mac',
  'hostname', 'ping', 'clear', 'help', 'history', 'running-config', 'gateway',
  'dhcp', 'dns', 'nat', 'inside', 'outside', 'server', 'nslookup', 'curl'];

function expand(tok) {
  if (WORDS.includes(tok)) return tok;
  const m = WORDS.filter(w => w.startsWith(tok));
  return m.length === 1 ? m[0] : tok;
}

const isIp = s => /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.test(s) && s.split('.').every(n => +n <= 255);
const lenToMask = n => [0, 1, 2, 3].map(i => { const b = Math.max(0, Math.min(8, n - i * 8)); return (0xff << (8 - b)) & 0xff; }).join('.');
const maskToLen = m => m.split('.').reduce((s, o) => s + ((+o).toString(2).match(/1/g)?.length ?? 0), 0);
const pad = (s, n) => String(s).padEnd(n);

/** parse "1.2.3.4/24" or "1.2.3.4 255.255.255.0" from tokens → {ip, mask, used} */
function parseAddr(toks) {
  const [a, b] = toks;
  if (!a) return null;
  if (a.includes('/')) {
    const [ip, len] = a.split('/');
    if (isIp(ip) && +len >= 0 && +len <= 32) return { ip, mask: lenToMask(+len), used: 1 };
    return null;
  }
  if (isIp(a) && b && isIp(b)) return { ip: a, mask: b, used: 2 };
  if (isIp(a)) return { ip: a, mask: '255.255.255.0', used: 1 };
  return null;
}

// ── command grammar: drives Tab completion and contextual `?` help ──
// key → { d: description, n: next level, cr: command may end here }
// '<if>' expands to this node's interface names; other '<…>' are value slots.
const ARG = (d, n, cr = true) => ({ d, n, cr });
const COMMON = {
  ping: ARG('Send ICMP echo requests', { '<ip>': ARG('Destination address', { '-c': ARG('Number of pings', { '<n>': ARG('Count 1-50') }, false) }) }, false),
  hostname: ARG('Set the node name', { '<name>': ARG('New name') }, false),
  clear: ARG('Clear the screen'),
  history: ARG('Show command history'),
  reload: ARG('Restart the node (clears ARP/MAC tables)'),
  help: ARG('List commands'),
};
const HOST_TREE = {
  ip: ARG('Show or set an IPv4 address', {
    '<addr/len>': ARG('Address on eth0, e.g. 10.0.0.5/24', { '<gateway>': ARG('Default gateway') }),
    '<if>': ARG('Interface, e.g. eth1', { '<addr/len>': ARG('Address, e.g. 10.0.1.5/24', { '<gateway>': ARG('Default gateway') }, false) }, false),
    address: ARG('Set an interface address', { '<if>': ARG('Interface', { '<addr/len>': ARG('Address, e.g. 10.0.1.5/24') }, false) }, false),
    dhcp: ARG('Lease an address on this link', { '<if>': ARG('Interface (default eth0)') }),
    dns: ARG('DNS server used by nslookup', { '<ip>': ARG('Resolver address') }, false),
  }),
  nslookup: ARG('Resolve a name', { '<name>': ARG('Hostname, e.g. lab.local') }, false),
  curl: ARG('HTTP GET', { '<ip>': ARG('Server address', { '<port>': ARG('TCP port (default 80)') }) }, false),
  nc: ARG('Probe a TCP or UDP port', { '-u': ARG('UDP instead of TCP', { '<ip>': ARG('Destination', { '<port>': ARG('Port') }, false) }, false), '<ip>': ARG('Destination', { '<port>': ARG('Port') }, false) }, false),
  server: ARG('Built-in daemons', {
    dhcp: ARG('DHCP server', { on: ARG('Enable'), off: ARG('Disable') }),
    dns: ARG('DNS server', { on: ARG('Enable'), off: ARG('Disable'), a: ARG('Add an A record', { '<name>': ARG('Name', { '<ip>': ARG('Address') }, false) }, false) }),
    http: ARG('HTTP server on port 80', { on: ARG('Enable'), off: ARG('Disable') }),
  }, false),
  show: ARG('Show information', {
    ip: ARG('Addresses, gateway, MACs', { interface: ARG('Interface table', { brief: ARG('Brief summary') }) }),
    interface: ARG('Interface summary'),
    arp: ARG('ARP cache'),
    route: ARG('Routing table'),
  }),
  arp: ARG('ARP cache'),
  set: ARG('Set a property', { pcname: ARG('Node name', { '<name>': ARG('New name') }, false) }, false),
  ...COMMON,
  clear: ARG('Clear the screen', { ip: ARG('Remove an IPv4 address', { '<if>': ARG('Interface (default eth0)') }) }),
};
const ROUTER_TREE = (isSwitch) => ({
  show: ARG('Show running system information', {
    ip: ARG('IP information', {
      interface: ARG('Interface status', { brief: ARG('Brief summary') }),
      route: ARG('Routing table'),
      arp: ARG('ARP cache'),
      nat: ARG('NAT roles'),
    }, false),
    interface: ARG('Interface status'),
    arp: ARG('ARP cache'),
    ...(isSwitch ? { mac: ARG('MAC information', { 'address-table': ARG('Learned MAC addresses') }, false) } : {}),
    'running-config': ARG('Current configuration as commands'),
  }, false),
  ip: ARG('Configure IP', {
    address: ARG('Set an interface address', { '<if>': ARG('Interface', { '<addr/len>': ARG('Address, e.g. 10.1.1.1/24') }, false) }, false),
    route: ARG('Add a static route', { '<prefix/len>': ARG('Destination, e.g. 0.0.0.0/0', { '<next-hop>': ARG('Next-hop address', { '<if>': ARG('Exit interface') }) }, false) }, false),
    nat: ARG('Source NAT role', {
      inside: ARG('Inside (hosts to hide)', { '<if>': ARG('Interface') }, false),
      outside: ARG('Outside (masquerade address)', { '<if>': ARG('Interface') }, false),
    }, false),
  }, false),
  no: ARG('Negate a command', {
    ip: ARG('IP', {
      address: ARG('Remove an interface address', { '<if>': ARG('Interface') }, false),
      route: ARG('Remove a static route', { '<n>': ARG('Route number from "show ip route"') }, false),
      nat: ARG('Clear a NAT role', { '<if>': ARG('Interface') }, false),
    }, false),
  }, false),
  ...COMMON,
});

export class SimShell {
  /** app: { lab, dev(id), meta(id), ifName(id,i), portCount(id), rename(id,name), refresh() } */
  constructor(app, devId, write) {
    this.app = app;
    this.id = devId;
    this.write = write;
    this.history = [];
    this.pings = [];      // outstanding {seq, ip, sent}
    this.pingJob = null;  // {ip, left, seq, next}
  }

  get dev() { return this.app.dev(this.id); }
  get isHost() { return this.dev?.kind === 'host'; }
  ports() {
    const d = this.dev;
    if (!d) return [];
    const n = this.app.portCount ? this.app.portCount(this.id) : d.ports.length;
    return d.ports.slice(0, n);
  }
  prompt() {
    const d = this.dev;
    if (!d) return '> ';
    return this.isHost ? `${d.name}> ` : `${d.name}# `;
  }

  banner() {
    const d = this.dev;
    if (this.isHost) {
      this.write(`netlab VPCS shell — ${d.name}\nType ? for commands. Try:  ip 10.0.0.5/24 10.0.0.254   then   ping 10.0.0.254\n`, 'dim');
    } else {
      this.write(`${d.name} — netlab ${d.kind} CLI\nType ? for commands. Abbreviations work: sh ip int br, sh ip ro, sh run\n`, 'dim');
    }
  }

  run(line) {
    const raw = line.trim();
    this.write(this.prompt() + line + '\n', 'echo');
    if (!raw) return;
    this.history.push(raw);
    if (!this.dev) return this.write('% node was deleted\n', 'err');
    const toks = raw.split(/\s+/).map((t, i) => (i < 4 ? expand(t.toLowerCase()) : t));
    try {
      this.exec(toks, raw);
    } catch (e) {
      this.write(`% ${e.message}\n`, 'err');
    }
    this.app.refresh();
  }

  exec(t, raw) {
    const [c0, c1, c2] = t;
    const d = this.dev;
    if (c0 === '?' || c0 === 'help') return this.help();
    if (c0 === 'clear' && !c1) return this.write('\x0c');
    if (c0 === 'history') return this.write(this.history.map((h, i) => `${pad(i + 1, 4)}${h}`).join('\n') + '\n');
    if (c0 === 'hostname' || (c0 === 'set' && c1 === 'pcname')) {
      const n = c0 === 'set' ? raw.split(/\s+/)[2] : raw.split(/\s+/)[1];
      if (!n) throw new Error('usage: hostname <name>');
      this.app.rename(this.id, n);
      return;
    }
    if (c0 === 'ping') return this.ping(t.slice(1));
    if (c0 === 'reload') { this.app.lab.set_power(this.id, false); this.app.lab.set_power(this.id, true); return this.write('Reloading… tables cleared.\n', 'dim'); }

    if (this.isHost) {
      if (c0 === 'show' && (c1 === 'interface' || (c1 === 'ip' && c2 === 'interface'))) return this.showIntBrief();
      if (c0 === 'show' && (!c1 || c1 === 'ip')) return this.showHostIp();
      if (c0 === 'ip' && !c1) return this.showHostIp();
      if (c0 === 'ip' && c1 === 'dhcp') {
        const port = c2 ? this.port(c2) : 0;
        if (!d.powered) throw new Error('node is stopped — start it first');
        if (!this.app.lab.dhcp_client(this.id, port)) throw new Error('DHCP could not start on this interface');
        return this.write(`DHCP discover on ${this.app.ifName(this.id, port)}\n`, 'dim');
      }
      if (c0 === 'ip' && c1 === 'dns') {
        if (!c2) return this.write(`DNS server : ${d.resolver || 'none'}\n`);
        if (!isIp(c2) || !this.app.lab.set_dns(this.id, c2)) throw new Error('usage: ip dns <ip>');
        return this.write(`DNS server ${c2}\n`);
      }
      if (c0 === 'clear' && c1 === 'ip' && c2 === 'dns') {
        this.app.lab.set_dns(this.id, '');
        return this.write('DNS server cleared\n');
      }
      if (c0 === 'nslookup') return this.lookup(t[1]);
      if (c0 === 'curl') return this.curl(t.slice(1));
      if (c0 === 'nc') return this.nc(t.slice(1));
      if (c0 === 'server') return this.server(t.slice(1));
      if (c0 === 'ip') {
        let port = 0, rest = t.slice(1);
        if (c1 === 'address') {
          port = this.port(t[2]);
          rest = t.slice(3);
        } else if (c1 && !parseAddr([c1])) {
          port = this.port(c1);
          rest = t.slice(2);
        }
        const a = parseAddr(rest);
        if (!a) throw new Error('usage: ip [if] <addr>/<len> [gateway]');
        this.app.lab.set_ip(this.id, port, a.ip, a.mask);
        const gw = rest[a.used];
        if (gw) {
          if (!isIp(gw)) throw new Error(`bad gateway "${gw}"`);
          this.app.lab.set_gateway(this.id, gw);
        }
        return this.write(`${d.name} (${this.app.ifName(this.id, port)}) : ${a.ip} ${a.mask}${gw ? ` gateway ${gw}` : ''}\n`);
      }
      if (c0 === 'clear' && c1 === 'ip') {
        const port = c2 ? this.port(c2) : 0;
        this.app.lab.clear_ip(this.id, port);
        return this.write(`IPv4 address cleared on ${this.app.ifName(this.id, port)}\n`);
      }
      if (c0 === 'arp' || (c0 === 'show' && c1 === 'arp')) return this.showArp();
      if (c0 === 'show' && c1 === 'route') return this.showRoute();
      throw new Error(`unknown command "${t[0]}" — type ?`);
    }

    // router / switch
    if (c0 === 'show') {
      if (c1 === 'ip' && c2 === 'interface') return this.showIntBrief();
      if (c1 === 'interface') return this.showIntBrief();
      if (c1 === 'ip' && c2 === 'route') return this.showRoute();
      if (c1 === 'arp' || (c1 === 'ip' && c2 === 'arp')) return this.showArp();
      if (c1 === 'ip' && c2 === 'nat') return this.showNat();
      if (c1 === 'mac') return this.showMac();
      if (c1 === 'running-config') return this.showRun();
      throw new Error('show what? interface | ip route | arp | mac address-table | running-config');
    }
    if (c0 === 'ip' && c1 === 'address') {
      const port = this.port(t[2]);
      const a = parseAddr(t.slice(3));
      if (!a) throw new Error('usage: ip address <if> <addr>/<len>');
      this.app.lab.set_ip(this.id, port, a.ip, a.mask);
      return;
    }
    if (c0 === 'no' && c1 === 'ip' && c2 === 'address') {
      this.app.lab.clear_ip(this.id, this.port(t[3]));
      return;
    }
    if (c0 === 'ip' && c1 === 'nat') {
      const role = c2;
      if (role !== 'inside' && role !== 'outside') throw new Error('usage: ip nat inside|outside <if>');
      const port = this.port(t[3]);
      if (!this.app.lab.set_nat(this.id, port, role)) throw new Error('NAT role was not set');
      return this.write(`${this.app.ifName(this.id, port)} is NAT ${role}\n`);
    }
    if (c0 === 'no' && c1 === 'ip' && c2 === 'nat') {
      const port = this.port(t[3]);
      this.app.lab.set_nat(this.id, port, 'off');
      return this.write(`${this.app.ifName(this.id, port)} NAT cleared\n`);
    }
    if (c0 === 'ip' && c1 === 'route') {
      const a = parseAddr(t.slice(2));
      if (!a) throw new Error('usage: ip route <prefix>/<len> <next-hop> [if]');
      const nh = t[2 + a.used];
      if (!isIp(nh ?? '')) throw new Error('missing next-hop');
      const port = t[3 + a.used] ? this.port(t[3 + a.used]) : this.portFor(nh);
      if (port < 0) throw new Error(`next-hop ${nh} is not on a connected network — name the interface`);
      this.app.lab.add_route(this.id, a.ip, a.mask, nh, port);
      return;
    }
    if (c0 === 'no' && c1 === 'ip' && c2 === 'route') {
      const n = +t[3];
      if (!(n >= 1)) throw new Error('usage: no ip route <index from "show ip route">');
      this.app.lab.remove_route(this.id, n - 1);
      return;
    }
    throw new Error(`invalid input "${raw}" — type ?`);
  }

  port(name) {
    const ps = this.ports();
    if (name == null) throw new Error('interface required');
    const i = ps.findIndex((_, i) => this.app.ifName(this.id, i) === name.toLowerCase());
    if (i >= 0) return i;
    if (/^\d+$/.test(name) && +name < ps.length) return +name;
    throw new Error(`no interface "${name}" — have ${ps.map((_, i) => this.app.ifName(this.id, i)).join(', ')}`);
  }

  portFor(ipStr) {
    const toN = s => s.split('.').reduce((a, o) => (a << 8 | +o) >>> 0, 0);
    return this.ports().findIndex(p => p.ip && ((toN(p.ip) & toN(p.mask)) >>> 0) === ((toN(ipStr) & toN(p.mask)) >>> 0));
  }

  tree() { return this.isHost ? HOST_TREE : ROUTER_TREE(this.dev?.kind === 'switch'); }
  ifNames() { return this.ports().map((_, i) => this.app.ifName(this.id, i)); }

  /** words valid at `level` that start with `part` ([{ w, d, slot }]) */
  options(level, part) {
    const out = [];
    for (const [k, v] of Object.entries(level)) {
      if (k === '<if>') { for (const n of this.ifNames()) if (n.startsWith(part)) out.push({ w: n, d: v.d, node: v }); }
      else if (k.startsWith('<')) out.push({ w: k, d: v.d, node: v, slot: true });
      else if (k.startsWith(part)) out.push({ w: k, d: v.d, node: v });
    }
    return out;
  }

  /** walk the grammar along completed tokens → { level, cr } or null if the line is off-grammar */
  walk(tokens) {
    let level = this.tree(), cr = true;
    for (const t of tokens) {
      const lc = t.toLowerCase();
      const exact = this.options(level, lc).filter(o => !o.slot && o.w === lc);
      const pre = exact.length ? exact : this.options(level, lc).filter(o => !o.slot);
      const slot = Object.entries(level).find(([k]) => k.startsWith('<') && k !== '<if>');
      let node;
      if (pre.length === 1) node = pre[0].node;
      else if (!pre.length && slot) node = slot[1];
      else return null;
      level = node.n ?? {};
      cr = node.cr !== false;
    }
    return { level, cr };
  }

  /** Tab: complete the word under the cursor. Returns { line, list? } */
  complete(line) {
    const endsSpace = /\s$/.test(line) || line === '';
    const toks = line.trim() ? line.trim().split(/\s+/) : [];
    const part = endsSpace ? '' : (toks.pop() ?? '');
    const at = this.walk(toks);
    if (!at) return { line };
    const opts = this.options(at.level, part.toLowerCase()).filter(o => !o.slot);
    if (!opts.length) return { line };
    const base = toks.length ? toks.join(' ') + ' ' : '';
    if (opts.length === 1) return { line: base + opts[0].w + ' ' };
    // extend to the longest common prefix; if that adds nothing, list the choices
    let cp = opts[0].w;
    for (const o of opts) while (!o.w.startsWith(cp)) cp = cp.slice(0, -1);
    if (cp.length > part.length) return { line: base + cp };
    return { line, list: opts.map(o => o.w) };
  }

  /** `?`: Cisco-style contextual help for the current line (not executed) */
  contextHelp(line) {
    this.write(this.prompt() + line + '?\n', 'echo');
    const endsSpace = /\s$/.test(line) || line === '';
    const toks = line.trim() ? line.trim().split(/\s+/) : [];
    if (!endsSpace) {
      // "sh?" → words that start with "sh"
      const part = toks.pop();
      const at = this.walk(toks);
      const opts = at ? this.options(at.level, part.toLowerCase()).filter(o => !o.slot) : [];
      this.write(opts.length ? opts.map(o => o.w).join('  ') + '\n' : '% Unrecognized command\n', opts.length ? '' : 'err');
      return;
    }
    const at = this.walk(toks);
    if (!at) return this.write('% Unrecognized command\n', 'err');
    const opts = this.options(at.level, '');
    const rows = opts.map(o => `  ${pad(o.w, 18)}${o.d}`);
    if (at.cr && toks.length) rows.push(`  ${pad('<cr>', 18)}`);
    this.write((rows.length ? rows.join('\n') : '  <cr>') + '\n');
  }

  help() {
    const rows = this.isHost ? [
      ['ip [if] <addr>/<len> [gw]', 'set address (on eth0 or <if>) and gateway'],
      ['ip dhcp [if]', 'lease an address from a DHCP server on this link'],
      ['ip dns <ip>', 'set the resolver used by nslookup'],
      ['nslookup <name>', 'DNS query'],
      ['curl <ip> [port]', 'HTTP GET (default port 80)'],
      ['nc [-u] <ip> <port>', 'TCP connect, or UDP probe with -u'],
      ['server dhcp|dns|http on|off', 'toggle a built-in daemon'],
      ['server dns a <name> <ip>', 'publish an A record'],
      ['show ip | show interface', 'show addresses, gateway, MACs'],
      ['clear ip [if]', 'remove address on eth0 or <if>'],
      ['ping <ip> [-c n]', 'send ICMP echo requests'],
      ['arp | show route', 'show ARP cache or routing table'],
      ['hostname <name>', 'rename this node'],
      ['clear | history | reload', ''],
    ] : [
      ['show ip interface brief', 'interfaces, addresses, link state'],
      ['show ip route', 'routing table'],
      ['show arp', 'ARP cache'],
      ...(this.dev.kind === 'switch' ? [['show mac address-table', 'learned MACs']] : []),
      ['show running-config', 'config as commands'],
      ['ip address <if> <addr>/<len>', 'address an interface'],
      ['no ip address <if>', ''],
      ['ip route <net>/<len> <nh> [if]', 'add static route (0.0.0.0/0 = default)'],
      ['no ip route <n>', 'remove route n'],
      ['ip nat inside|outside <if>', 'source NAT (masquerade on outside)'],
      ['show ip nat', 'NAT role of each interface'],
      ['no ip nat <if>', 'clear a NAT role'],
      ['ping <ip> [-c n]', ''],
      ['hostname <name> | clear | reload', ''],
    ];
    this.write(rows.map(([a, b]) => `  ${pad(a, 32)}${b}`).join('\n') + '\n');
  }

  links() {
    const s = this.app.snap();
    return s.links;
  }

  showHostIp() {
    const d = this.dev, ps = this.ports();
    const gw = d.routes.find(r => r.prefix === '0.0.0.0')?.next_hop;
    if (ps.length <= 1) {
      const p = ps[0] ?? d.ports[0];
      this.write([
        `NAME        : ${d.name}`,
        `IP/MASK     : ${p.ip ? `${p.ip}/${maskToLen(p.mask)}` : '0.0.0.0/0'}`,
        `GATEWAY     : ${gw ?? '0.0.0.0'}`,
        `MAC         : ${p.mac}`,
        `DNS         : ${d.resolver || '0.0.0.0'}`,
        `DHCP        : ${d.dhcp || 'off'}`,
        `LINK        : ${this.linkState(0)}`,
      ].join('\n') + '\n');
      return;
    }
    const rows = [`NAME        : ${d.name}`, `GATEWAY     : ${gw ?? '0.0.0.0'}`, `DNS         : ${d.resolver || '0.0.0.0'}`, `DHCP        : ${d.dhcp || 'off'}`];
    ps.forEach((p, i) => {
      rows.push(`${pad(this.app.ifName(this.id, i).toUpperCase(), 12)}: ${pad(p.ip ? `${p.ip}/${maskToLen(p.mask)}` : '0.0.0.0/0', 18)} ${p.mac}  (${this.linkState(i)})`);
    });
    this.write(rows.join('\n') + '\n');
  }

  linkState(i) {
    const p = this.dev.ports[i];
    if (p.link == null) return 'down (no cable)';
    const l = this.links().find(l => l.id === p.link);
    return l?.up ? 'up' : 'down (unplugged)';
  }

  showIntBrief() {
    const ps = this.ports();
    const out = [`${pad('Interface', 12)}${pad('IP-Address', 18)}${pad('Status', 24)}MAC`];
    ps.forEach((p, i) => out.push(
      `${pad(this.app.ifName(this.id, i), 12)}${pad(p.ip ? `${p.ip}/${maskToLen(p.mask)}` : 'unassigned', 18)}${pad(this.linkState(i), 24)}${p.mac}`));
    this.write(out.join('\n') + '\n');
  }

  showRoute() {
    const d = this.dev, ps = this.ports();
    const out = ['Codes: C - connected, S - static, S* - default\n'];
    const toN = s => s.split('.').reduce((a, o) => (a << 8 | +o) >>> 0, 0);
    const fromN = n => [24, 16, 8, 0].map(s => (n >>> s) & 255).join('.');
    ps.forEach((p, i) => {
      if (!p.ip) return;
      const net = fromN((toN(p.ip) & toN(p.mask)) >>> 0);
      out.push(`C     ${net}/${maskToLen(p.mask)} is directly connected, ${this.app.ifName(this.id, i)}`);
    });
    d.routes.forEach((r, i) => {
      const code = r.prefix === '0.0.0.0' && r.mask === '0.0.0.0' ? 'S*' : 'S ';
      out.push(`${code}    ${r.prefix}/${maskToLen(r.mask)} via ${r.next_hop}, ${this.app.ifName(this.id, r.out_port)}   [${i + 1}]`);
    });
    if (out.length === 1) out.push('(empty — no addresses configured)');
    this.write(out.join('\n') + '\n');
  }

  showArp() {
    const a = this.dev.arp;
    if (!a.length) return this.write('ARP cache is empty\n', 'dim');
    this.write([`${pad('Address', 18)}Hardware Addr`, ...a.map(([ip, m]) => `${pad(ip, 18)}${m}`)].join('\n') + '\n');
  }

  showMac() {
    const m = this.dev.macs;
    if (!m.length) return this.write('MAC table is empty\n', 'dim');
    this.write([`${pad('Mac Address', 20)}Port`, ...m.sort((a, b) => a[1] - b[1]).map(([mac, p]) => `${pad(mac, 20)}${this.app.ifName(this.id, p)}`)].join('\n') + '\n');
  }

  showRun() {
    const d = this.dev, ps = this.ports();
    const out = ['!', `hostname ${d.name}`, '!'];
    ps.forEach((p, i) => {
      out.push(`interface ${this.app.ifName(this.id, i)}`);
      out.push(p.ip ? ` ip address ${p.ip} ${p.mask}` : ' no ip address');
      if (p.nat) out.push(` ip nat ${p.nat}`);
      if (p.link == null) out.push(' shutdown');
      out.push('!');
    });
    d.routes.forEach(r => out.push(`ip route ${r.prefix} ${r.mask} ${r.next_hop}`));
    out.push('end');
    this.write(out.join('\n') + '\n');
  }

  showNat() {
    const rows = this.ports().map((p, i) => `${pad(this.app.ifName(this.id, i), 12)}${p.nat || 'off'}`);
    this.write((rows.length ? rows.join('\n') : '(no interfaces)') + '\n');
  }

  lookup(name) {
    if (!name) throw new Error('usage: nslookup <name>');
    if (!this.dev.powered) throw new Error('node is stopped — start it first');
    if (!this.app.lab.dns_query(this.id, name)) throw new Error('no address or DNS server — ip dhcp, or ip <addr> and ip dns <ip>');
    this.write(`Server: ${this.dev.resolver || '?'}\nName: ${name}\n`, 'dim');
  }

  curl(args) {
    let host = args[0], port = 80;
    if (!host) throw new Error('usage: curl <ip> [port]');
    if (host.includes(':')) { const [h, p] = host.split(':'); host = h; port = +p || 80; }
    else if (args[1] && /^\d+$/.test(args[1])) port = +args[1];
    if (!isIp(host)) throw new Error('usage: curl <ip> [port]');
    if (!this.dev.powered) throw new Error('node is stopped — start it first');
    if (!this.app.lab.http_get(this.id, host, port)) throw new Error('could not send the GET');
    this.write(`GET / HTTP/1.0  →  ${host}:${port}\n`, 'dim');
  }

  nc(args) {
    let i = 0, udp = false;
    if (args[0] === '-u') { udp = true; i = 1; }
    const host = args[i], port = +args[i + 1];
    if (!isIp(host ?? '') || !(port > 0 && port < 65536)) throw new Error('usage: nc [-u] <ip> <port> [payload]');
    if (!this.dev.powered) throw new Error('node is stopped — start it first');
    const ok = udp
      ? this.app.lab.probe_udp(this.id, host, port, args.slice(i + 2).join(' '))
      : this.app.lab.probe_tcp(this.id, host, port);
    if (!ok) throw new Error('could not send the probe');
    this.write(`${udp ? 'UDP' : 'TCP'} ${host}:${port}\n`, 'dim');
  }

  server(args) {
    const [what, a, name, ip] = args;
    const d = this.dev;
    if (what === 'dns' && a === 'a') {
      if (!name || !isIp(ip ?? '') || !this.app.lab.add_dns_record(this.id, name, ip)) throw new Error('usage: server dns a <name> <ip>');
      return this.write(`A ${name} → ${ip}\n`);
    }
    if (!['dhcp', 'dns', 'http'].includes(what)) throw new Error('usage: server dhcp|dns|http on|off');
    const on = a !== 'off';
    this.app.lab.set_services(this.id, what === 'dhcp' ? on : !!d.svc_dhcp, what === 'dns' ? on : !!d.svc_dns, what === 'http' ? on : !!d.svc_http);
    this.write(`${what} server ${on ? 'on' : 'off'}\n`);
  }

  ping(args) {
    const ip = args.find(isIp);
    if (!ip) throw new Error('usage: ping <ip> [-c count]');
    const ci = args.indexOf('-c');
    const count = ci >= 0 ? Math.max(1, Math.min(50, +args[ci + 1] || 5)) : 5;
    if (!this.dev.powered) throw new Error('node is stopped — start it first');
    if (!this.dev.ports.some(p => p.ip)) throw new Error('no IPv4 address configured');
    this.pings = [];
    this.pingJob = { ip, left: count, seq: 0, next: 0, sent: 0, ok: 0 };
    this.write(`PING ${ip}: 56 data bytes\n`, 'dim');
  }

  /** called every sim tick */
  onTick(tick) {
    const j = this.pingJob;
    if (!j) return;
    if (j.left > 0 && tick >= j.next) {
      j.seq++; j.left--; j.sent++;
      j.next = tick + (this.app.realtime?.() ? 400 : 30);   // ~0.4 s apart in realtime
      if (!this.app.lab.ping(this.id, j.ip)) this.write(`icmp_seq=${j.seq} could not send\n`, 'err');
      else this.pings.push({ seq: j.seq, sent: tick });
    }
    const timeout = this.app.realtime?.() ? 2000 : 200;   // 2 s in realtime (real guests can be slow)
    while (this.pings.length && tick - this.pings[0].sent > timeout) {
      const p = this.pings.shift();
      this.write(`Request timeout for icmp_seq ${p.seq}\n`, 'err');
    }
    if (j.left === 0 && !this.pings.length) {
      const loss = Math.round(100 * (1 - j.ok / j.sent));
      this.write(`--- ${j.ip} ping statistics ---\n${j.sent} packets transmitted, ${j.ok} received, ${loss}% packet loss\n`, loss ? 'warn' : 'ok');
      this.pingJob = null;
    }
  }

  /** called with every sim event */
  onEvent(ev) {
    if (ev.device !== this.id) return;
    if (ev.kind === 'dhcp' || ev.kind === 'dns' || ev.kind === 'tcp' || ev.kind === 'http' || ev.kind === 'udp') {
      const msg = ev.msg.replace(/^[^:]+:\s*/, '');
      const bad = /fail|timeout|closed|NXDOMAIN|unreachable/i.test(msg);
      this.write(msg + '\n', bad ? 'err' : 'ok');
      return;
    }
    if (!this.pingJob) return;
    if (ev.kind === 'deliver') {
      const m = ev.msg.match(/reply from ([\d.]+) ttl=(\d+)/);
      const p = this.pings.shift();
      if (!m || !p) return;
      this.pingJob.ok++;
      this.write(`64 bytes from ${m[1]}: icmp_seq=${p.seq} ttl=${m[2]} time=${((ev.tick - p.sent) * TICK_MS).toFixed(1)} ms\n`, 'ok');
    } else if (ev.kind === 'drop' && /no route|not connected|unplugged/.test(ev.msg)) {
      const p = this.pings.shift();
      if (p) this.write(`From ${this.dev.name} icmp_seq=${p.seq} Destination Host Unreachable (${ev.msg.replace(/^[^:]+:\s*/, '')})\n`, 'err');
    }
  }
}
