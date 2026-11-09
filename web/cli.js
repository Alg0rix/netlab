// Interactive shell for simulated nodes. Hosts get a VPCS-flavoured shell,
// routers/switches a Cisco-flavoured one (with abbreviations: "sh ip int br").

import { TICK_MS } from './capture.js';

const WORDS = ['show', 'interface', 'brief', 'route', 'arp', 'address', 'address-table', 'mac',
  'hostname', 'ping', 'clear', 'help', 'history', 'running-config', 'gateway'];

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

export class SimShell {
  /** app: { lab, dev(id), meta(id), ifName(id,i), rename(id,name), refresh() } */
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
      if (c0 === 'show' && (!c1 || c1 === 'ip')) return this.showHostIp();
      if (c0 === 'ip' && !c1) return this.showHostIp();
      if (c0 === 'ip' && c1 === 'dhcp') throw new Error('DHCP is not modelled yet — set a static address: ip 10.0.0.5/24 10.0.0.1');
      if (c0 === 'ip') {
        const a = parseAddr(t.slice(1));
        if (!a) throw new Error('usage: ip <addr>/<len> [gateway]');
        this.app.lab.set_ip(this.id, 0, a.ip, a.mask);
        const gw = t[1 + a.used];
        if (gw) {
          if (!isIp(gw)) throw new Error(`bad gateway "${gw}"`);
          this.app.lab.set_gateway(this.id, gw);
        }
        return this.write(`${d.name} : ${a.ip} ${a.mask}${gw ? ` gateway ${gw}` : ''}\n`);
      }
      if (c0 === 'clear' && c1 === 'ip') { this.app.lab.clear_ip(this.id, 0); return this.write('IPv4 address cleared\n'); }
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
    const d = this.dev;
    if (name == null) throw new Error('interface required');
    const i = d.ports.findIndex((_, i) => this.app.ifName(this.id, i) === name.toLowerCase());
    if (i >= 0) return i;
    if (/^\d+$/.test(name) && +name < d.ports.length) return +name;
    throw new Error(`no interface "${name}" — have ${d.ports.map((_, i) => this.app.ifName(this.id, i)).join(', ')}`);
  }

  portFor(ipStr) {
    const toN = s => s.split('.').reduce((a, o) => (a << 8 | +o) >>> 0, 0);
    return this.dev.ports.findIndex(p => p.ip && ((toN(p.ip) & toN(p.mask)) >>> 0) === ((toN(ipStr) & toN(p.mask)) >>> 0));
  }

  help() {
    const rows = this.isHost ? [
      ['ip <addr>/<len> [gw]', 'set address and default gateway'],
      ['show ip', 'show address, gateway, MAC'],
      ['clear ip', 'remove the address'],
      ['ping <ip> [-c n]', 'send ICMP echo requests'],
      ['arp', 'show the ARP cache'],
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
    const d = this.dev, p = d.ports[0];
    const gw = d.routes.find(r => r.prefix === '0.0.0.0')?.next_hop;
    this.write([
      `NAME        : ${d.name}`,
      `IP/MASK     : ${p.ip ? `${p.ip}/${maskToLen(p.mask)}` : '0.0.0.0/0'}`,
      `GATEWAY     : ${gw ?? '0.0.0.0'}`,
      `MAC         : ${p.mac}`,
      `LINK        : ${this.linkState(0)}`,
    ].join('\n') + '\n');
  }

  linkState(i) {
    const p = this.dev.ports[i];
    if (p.link == null) return 'down (no cable)';
    const l = this.links().find(l => l.id === p.link);
    return l?.up ? 'up' : 'down (unplugged)';
  }

  showIntBrief() {
    const d = this.dev;
    const out = [`${pad('Interface', 12)}${pad('IP-Address', 18)}${pad('Status', 24)}MAC`];
    d.ports.forEach((p, i) => out.push(
      `${pad(this.app.ifName(this.id, i), 12)}${pad(p.ip ? `${p.ip}/${maskToLen(p.mask)}` : 'unassigned', 18)}${pad(this.linkState(i), 24)}${p.mac}`));
    this.write(out.join('\n') + '\n');
  }

  showRoute() {
    const d = this.dev;
    const out = ['Codes: C - connected, S - static, S* - default\n'];
    const toN = s => s.split('.').reduce((a, o) => (a << 8 | +o) >>> 0, 0);
    const fromN = n => [24, 16, 8, 0].map(s => (n >>> s) & 255).join('.');
    d.ports.forEach((p, i) => {
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
    const d = this.dev;
    const out = ['!', `hostname ${d.name}`, '!'];
    d.ports.forEach((p, i) => {
      out.push(`interface ${this.app.ifName(this.id, i)}`);
      out.push(p.ip ? ` ip address ${p.ip} ${p.mask}` : ' no ip address');
      if (p.link == null) out.push(' shutdown');
      out.push('!');
    });
    d.routes.forEach(r => out.push(`ip route ${r.prefix} ${r.mask} ${r.next_hop}`));
    out.push('end');
    this.write(out.join('\n') + '\n');
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
      j.next = tick + 30;
      if (!this.app.lab.ping(this.id, j.ip)) this.write(`icmp_seq=${j.seq} could not send\n`, 'err');
      else this.pings.push({ seq: j.seq, sent: tick });
    }
    while (this.pings.length && tick - this.pings[0].sent > 200) {
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
    if (!this.pingJob || ev.device !== this.id) return;
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
