// Packet dissection (Ethernet II / ARP / IPv4 / ICMP / UDP / TCP), hex dump
// and libpcap export so captures open in Wireshark.

export const TICK_MS = 0.1; // simulated time per tick (ms) — capture timestamps, ping RTT

const hex = (n, w = 2) => n.toString(16).padStart(w, '0');
const mac = (b, o) => Array.from(b.subarray(o, o + 6), x => hex(x)).join(':');
const ip = (b, o) => `${b[o]}.${b[o + 1]}.${b[o + 2]}.${b[o + 3]}`;
const u16 = (b, o) => (b[o] << 8) | b[o + 1];

const ETHERTYPES = {
  0x0800: 'IPv4', 0x0806: 'ARP', 0x86dd: 'IPv6',
  0x8100: '802.1Q', 0x88a8: '802.1ad QinQ',
  0x88cc: 'LLDP', 0x8809: 'LACP', 0x9000: 'Loopback',
};
const IPPROTO = { 1: 'ICMP', 6: 'TCP', 17: 'UDP', 47: 'GRE', 88: 'EIGRP', 89: 'OSPF', 112: 'VRRP' };
const ICMPTYPE = { 0: 'Echo (ping) reply', 3: 'Destination unreachable', 8: 'Echo (ping) request', 11: 'Time exceeded' };

/**
 * Returns { proto, src, dst, info, tree } where tree is
 * [{ title, range:[start,end], fields:[{ k, v, range }] }]
 */
export function dissect(b) {
  const tree = [];
  const out = { proto: 'ETH', src: '', dst: '', info: '', vlan: null, sport: null, dport: null, tree };
  if (b.length < 14) { out.info = 'Truncated frame'; return out; }

  let et = u16(b, 12), off = 14, vlanId = null;
  out.src = mac(b, 6); out.dst = mac(b, 0);
  const eth = {
    title: `Ethernet II, Src: ${mac(b, 6)}, Dst: ${mac(b, 0)}`, range: [0, 14], fields: [
      { k: 'Destination', v: mac(b, 0) + (b[0] === 0xff && b[5] === 0xff ? ' (Broadcast)' : (b[0] & 1) ? ' (Multicast)' : ''), range: [0, 6] },
      { k: 'Source', v: mac(b, 6), range: [6, 12] },
      { k: 'Type / Length', v: et < 0x0600 ? `802.3 Length (${et} bytes)` : `${ETHERTYPES[et] ?? 'Unknown'} (0x${hex(et, 4)})`, range: [12, 14] },
    ],
  };
  tree.push(eth);
  while ((et === 0x8100 || et === 0x88a8) && b.length >= off + 4) {
    const vid = u16(b, off) & 0xfff;
    vlanId = vid;
    out.vlan = vid;
    tree.push({ title: `${et === 0x88a8 ? '802.1ad S-VLAN' : '802.1Q Virtual LAN'}, PRI: ${b[off] >> 5}, VID: ${vid}`, range: [off, off + 4], fields: [
      { k: 'Priority (PCP)', v: String(b[off] >> 5), range: [off, off + 1] },
      { k: 'DEI / CFI', v: String((b[off] >> 4) & 1), range: [off, off + 1] },
      { k: 'VLAN ID (VID)', v: String(vid), range: [off, off + 2] },
      { k: 'Encapsulated Type', v: `0x${hex(u16(b, off + 2), 4)}`, range: [off + 2, off + 4] },
    ] });
    et = u16(b, off + 2); off += 4;
  }
  const vlanPrefix = vlanId != null ? `[VLAN ${vlanId}] ` : '';
  if (et < 0x0600) {
    // IEEE 802.3 / LLC (STP BPDU, PVST+ BPDU, CDP/VTP)
    const isStpMac = b[0] === 0x01 && b[1] === 0x80 && b[2] === 0xc2 && b[3] === 0x00 && b[4] === 0x00 && b[5] === 0x00;
    const isCiscoMcast = b[0] === 0x01 && b[1] === 0x00 && b[2] === 0x0c && b[3] === 0xcc && b[4] === 0xcc;
    if ((isStpMac || (isCiscoMcast && b[5] === 0xcd)) && b.length >= off + 7) {
      const bpduOff = (b[off] === 0xaa && b[off + 1] === 0xaa) ? off + 8 : off + 3;
      const isPvst = isCiscoMcast && b[5] === 0xcd;
      out.proto = isPvst ? 'PVST+' : 'STP';
      if (b.length >= bpduOff + 27) {
        const bpduType = b[bpduOff + 3];
        const rootPri = u16(b, bpduOff + 5);
        const rootMac = mac(b, bpduOff + 7);
        const bridgePri = u16(b, bpduOff + 17);
        const bridgeMac = mac(b, bpduOff + 19);
        const portId = u16(b, bpduOff + 25);
        out.info = `${vlanPrefix}${bpduType === 0x02 ? 'RSTP' : isPvst ? 'PVST+' : 'STP'} BPDU · Root=${rootPri}/${rootMac} Bridge=${bridgePri}/${bridgeMac} Port=0x${hex(portId, 4)}`;
        tree.push({ title: `Spanning Tree Protocol (${out.proto})`, range: [off, b.length], fields: [
          { k: 'BPDU Type', v: `0x${hex(bpduType)} (${bpduType === 0x02 ? 'Rapid/MST' : bpduType === 0x80 ? 'Topology Change' : 'Configuration'})`, range: [bpduOff + 3, bpduOff + 4] },
          { k: 'Root Identifier', v: `${rootPri} / ${rootMac}`, range: [bpduOff + 5, bpduOff + 13] },
          { k: 'Bridge Identifier', v: `${bridgePri} / ${bridgeMac}`, range: [bpduOff + 17, bpduOff + 25] },
          { k: 'Port Identifier', v: `0x${hex(portId, 4)}`, range: [bpduOff + 25, bpduOff + 27] },
        ] });
      } else {
        out.info = `${vlanPrefix}${out.proto} BPDU (${et} bytes)`;
      }
      return out;
    }
    if (isCiscoMcast && b[5] === 0xcc) {
      out.proto = 'CDP/VTP';
      out.info = `${vlanPrefix}Cisco Discovery / VTP Frame (${et} bytes)`;
      return out;
    }
    out.proto = 'LLC';
    out.info = `${vlanPrefix}802.3 length ${et}`;
    return out;
  }

  if (et === 0x0806 && b.length >= off + 28) {
    const op = u16(b, off + 6);
    const spa = ip(b, off + 14), tpa = ip(b, off + 24);
    out.proto = 'ARP';
    out.src = mac(b, off + 8);
    out.dst = op === 1 ? 'Broadcast' : mac(b, off + 18);
    out.info = op === 1 ? `Who has ${tpa}? Tell ${spa}` : `${spa} is at ${mac(b, off + 8)}`;
    tree.push({ title: `Address Resolution Protocol (${op === 1 ? 'request' : 'reply'})`, range: [off, off + 28], fields: [
      { k: 'Hardware type', v: `Ethernet (${u16(b, off)})`, range: [off, off + 2] },
      { k: 'Protocol type', v: `IPv4 (0x${hex(u16(b, off + 2), 4)})`, range: [off + 2, off + 4] },
      { k: 'Hardware size', v: String(b[off + 4]), range: [off + 4, off + 5] },
      { k: 'Protocol size', v: String(b[off + 5]), range: [off + 5, off + 6] },
      { k: 'Opcode', v: `${op === 1 ? 'request' : 'reply'} (${op})`, range: [off + 6, off + 8] },
      { k: 'Sender MAC address', v: mac(b, off + 8), range: [off + 8, off + 14] },
      { k: 'Sender IP address', v: spa, range: [off + 14, off + 18] },
      { k: 'Target MAC address', v: mac(b, off + 18), range: [off + 18, off + 24] },
      { k: 'Target IP address', v: tpa, range: [off + 24, off + 28] },
    ] });
    return out;
  }

  if (et === 0x0800 && b.length >= off + 20) {
    const ihl = (b[off] & 0x0f) * 4;
    const proto = b[off + 9];
    const src = ip(b, off + 12), dst = ip(b, off + 16);
    out.proto = IPPROTO[proto] ?? 'IPv4';
    out.src = src; out.dst = dst;
    out.info = `${IPPROTO[proto] ?? 'proto ' + proto} ${src} → ${dst}`;
    tree.push({ title: `Internet Protocol Version 4, Src: ${src}, Dst: ${dst}`, range: [off, off + ihl], fields: [
      { k: 'Version', v: String(b[off] >> 4), range: [off, off + 1] },
      { k: 'Header length', v: `${ihl} bytes`, range: [off, off + 1] },
      { k: 'DSCP/ECN', v: `0x${hex(b[off + 1])}`, range: [off + 1, off + 2] },
      { k: 'Total length', v: String(u16(b, off + 2)), range: [off + 2, off + 4] },
      { k: 'Identification', v: `0x${hex(u16(b, off + 4), 4)}`, range: [off + 4, off + 6] },
      { k: 'Flags', v: `0x${hex(b[off + 6] >> 5)}${b[off + 6] & 0x40 ? " (Don't fragment)" : ''}`, range: [off + 6, off + 8] },
      { k: 'Time to live', v: String(b[off + 8]), range: [off + 8, off + 9] },
      { k: 'Protocol', v: `${IPPROTO[proto] ?? 'Unknown'} (${proto})`, range: [off + 9, off + 10] },
      { k: 'Header checksum', v: `0x${hex(u16(b, off + 10), 4)}`, range: [off + 10, off + 12] },
      { k: 'Source address', v: src, range: [off + 12, off + 16] },
      { k: 'Destination address', v: dst, range: [off + 16, off + 20] },
    ] });
    const p = off + ihl;
    if (proto === 1 && b.length >= p + 8) {
      const t = b[p], id = u16(b, p + 4), seq = u16(b, p + 6);
      const name = ICMPTYPE[t] ?? `type ${t}`;
      out.info = t === 0 || t === 8 ? `${name}  id=0x${hex(id, 4)}, seq=${seq}, ttl=${b[off + 8]}` : name;
      tree.push({ title: 'Internet Control Message Protocol', range: [p, b.length], fields: [
        { k: 'Type', v: `${t} (${name})`, range: [p, p + 1] },
        { k: 'Code', v: String(b[p + 1]), range: [p + 1, p + 2] },
        { k: 'Checksum', v: `0x${hex(u16(b, p + 2), 4)}`, range: [p + 2, p + 4] },
        { k: 'Identifier', v: `${id} (0x${hex(id, 4)})`, range: [p + 4, p + 6] },
        { k: 'Sequence number', v: String(seq), range: [p + 6, p + 8] },
        { k: 'Data', v: `${b.length - p - 8} bytes`, range: [p + 8, b.length] },
      ] });
    } else if ((proto === 17 || proto === 6) && b.length >= p + 8) {
      const sp = u16(b, p), dp = u16(b, p + 2);
      out.sport = sp; out.dport = dp;
      const udp = proto === 17;
      if (udp && (sp === 67 || dp === 67 || sp === 68 || dp === 68)) out.proto = 'DHCP';
      else if (udp && (sp === 53 || dp === 53)) out.proto = 'DNS';
      else if (!udp) {
        const doff = Math.max(20, (b[p + 12] >> 4) * 4);
        const end = Math.min(b.length, p + doff + 8);
        let head = '';
        for (let i = p + doff; i < end; i++) head += String.fromCharCode(b[i]);
        if (/^(GET |POST |HEAD |HTTP\/)/.test(head)) out.proto = 'HTTP';
      }
      const name = out.proto === 'DHCP' ? 'DHCP' : out.proto === 'DNS' ? 'DNS' : out.proto === 'HTTP' ? 'HTTP' : (udp ? 'UDP' : 'TCP');
      out.info = `${name} ${sp} → ${dp}` + (udp && out.proto === 'UDP' ? `  Len=${u16(b, p + 4) - 8}` : '');
      tree.push({ title: `${udp ? 'User Datagram' : 'Transmission Control'} Protocol, Src Port: ${sp}, Dst Port: ${dp}`, range: [p, b.length], fields: [
        { k: 'Source port', v: String(sp), range: [p, p + 2] },
        { k: 'Destination port', v: String(dp), range: [p + 2, p + 4] },
      ] });
    }
    return out;
  }

  out.proto = ETHERTYPES[et] ?? `0x${hex(et, 4)}`;
  out.info = `Ethertype 0x${hex(et, 4)}, ${b.length - off} bytes payload`;
  return out;
}

/** hex dump rows: [{ off, bytes:[{ i, h }], ascii:[{ i, c }] }] */
export function hexRows(b) {
  const rows = [];
  for (let o = 0; o < b.length; o += 16) {
    const bytes = [], ascii = [];
    for (let i = o; i < Math.min(o + 16, b.length); i++) {
      bytes.push({ i, h: hex(b[i]) });
      ascii.push({ i, c: b[i] >= 0x20 && b[i] < 0x7f ? String.fromCharCode(b[i]) : '·' });
    }
    rows.push({ off: hex(o, 4), bytes, ascii });
  }
  return rows;
}

/** libpcap (LINKTYPE_ETHERNET) file from [{ tick, bytes }] */
export function toPcap(packets) {
  const len = 24 + packets.reduce((s, p) => s + 16 + p.bytes.length, 0);
  const buf = new ArrayBuffer(len), v = new DataView(buf), u8 = new Uint8Array(buf);
  v.setUint32(0, 0xa1b2c3d4, true);
  v.setUint16(4, 2, true); v.setUint16(6, 4, true);
  v.setUint32(16, 65535, true); v.setUint32(20, 1, true);
  let o = 24;
  for (const p of packets) {
    const ms = p.tick * TICK_MS;
    v.setUint32(o, Math.floor(ms / 1000), true);
    v.setUint32(o + 4, Math.round((ms % 1000) * 1000), true);
    v.setUint32(o + 8, p.bytes.length, true);
    v.setUint32(o + 12, p.bytes.length, true);
    u8.set(p.bytes, o + 16);
    o += 16 + p.bytes.length;
  }
  return new Blob([buf], { type: 'application/vnd.tcpdump.pcap' });
}

/** Row class for a dissected frame: arp, icmp, tcp, udp, dhcp, dns, stp, vlan, route, eth. */
export function protoClass(d) {
  const p = (d?.proto || '').toLowerCase();
  if (p.includes('pvst') || p === 'stp') return 'stp';
  if (p === 'arp') return 'arp';
  if (p === 'icmp') return 'icmp';
  if (p === 'tcp' || p === 'http') return 'tcp';
  if (p === 'dhcp') return 'dhcp';
  if (p === 'dns') return 'dns';
  if (p === 'udp') return 'udp';
  if (p === 'ospf' || p === 'eigrp' || p === 'vrrp' || p === 'gre') return 'route';
  if (d?.vlan != null) return 'vlan';
  return 'eth';
}

/**
 * Display filter. Terms are AND-ed.
 * Keywords: stp pvst vlan arp icmp ospf eigrp tcp udp dns dhcp http
 * Comparisons: vlan == 10, ip == 10.0.0.1, mac == aa:bb:…, port == 80
 */
export function matchFilter(d, expr) {
  const q = (expr || '').trim().toLowerCase();
  if (!q || !d) return true;
  return q.split(/\s+/).every(term => matchTerm(d, term));
}

function matchTerm(d, term) {
  const m = term.match(/^([a-z0-9.]+)\s*(==|!=)\s*(.+)$/);
  if (m) {
    const val = m[3].replace(/^["']|["']$/g, '');
    let hit = false;
    if (m[1] === 'vlan') hit = d.vlan != null && String(d.vlan) === val;
    else if (m[1] === 'ip') hit = d.src === val || d.dst === val;
    else if (m[1] === 'mac') hit = (d.src + ' ' + d.dst).toLowerCase().includes(val);
    else if (m[1] === 'port' || m[1] === 'tcp.port' || m[1] === 'udp.port') hit = String(d.sport) === val || String(d.dport) === val;
    else if (m[1] === 'proto') hit = d.proto.toLowerCase() === val;
    return m[2] === '!=' ? !hit : hit;
  }
  const p = d.proto.toLowerCase();
  const key = {
    stp: () => p === 'stp' || p.includes('pvst'),
    pvst: () => p.includes('pvst'),
    vlan: () => d.vlan != null || p.includes('802.1'),
    arp: () => p === 'arp',
    icmp: () => p === 'icmp',
    ospf: () => p === 'ospf',
    eigrp: () => p === 'eigrp',
    tcp: () => p === 'tcp' || p === 'http',
    udp: () => p === 'udp' || p === 'dhcp' || p === 'dns',
    dns: () => p === 'dns',
    dhcp: () => p === 'dhcp',
    http: () => p === 'http',
  };
  if (key[term]) return key[term]();
  return `${d.proto} ${d.info} ${d.src} ${d.dst}`.toLowerCase().includes(term);
}

const parseMac = s => {
  const p = String(s || '').trim().split(/[:\-]/);
  if (p.length !== 6) return null;
  const b = p.map(x => parseInt(x, 16));
  if (b.some(n => Number.isNaN(n) || n > 255)) return null;
  return b;
};
const parseIp = s => {
  const p = String(s || '').trim().split('.');
  if (p.length !== 4 || p.some(x => +x > 255 || !/^\d+$/.test(x))) return null;
  return p.map(Number);
};
const sum16 = (bytes, extra = 0) => {
  let s = extra;
  for (let i = 0; i + 1 < bytes.length; i += 2) s += (bytes[i] << 8) | bytes[i + 1];
  if (bytes.length & 1) s += bytes[bytes.length - 1] << 8;
  while (s > 0xffff) s = (s & 0xffff) + (s >>> 16);
  return s;
};

/** Build an Ethernet frame: ARP who-has, or an ICMP echo request. vlan 0 omits the tag. */
export function craftFrame({ dst, src, vlan = 0, kind = 'arp', spa, tpa, icmpSrc, icmpDst }) {
  const d = parseMac(dst), s = parseMac(src);
  if (!d || !s) return null;
  const body = [];
  const u16p = n => body.push((n >> 8) & 255, n & 255);
  if (kind === 'arp') {
    const sa = parseIp(spa), ta = parseIp(tpa);
    if (!sa || !ta) return null;
    u16p(0x0806);
    body.push(0, 1, 8, 0, 6, 4, 0, 1, ...s, ...sa, 0, 0, 0, 0, 0, 0, ...ta);
  } else {
    const sa = parseIp(icmpSrc), da = parseIp(icmpDst);
    if (!sa || !da) return null;
    u16p(0x0800);
    const ip = [0x45, 0, 0, 28, 0, 1, 0, 0, 64, 1, 0, 0, ...sa, ...da];
    const c = (~sum16(ip)) & 0xffff;
    ip[10] = c >> 8; ip[11] = c & 255;
    const icmp = [8, 0, 0, 0, 0, 1, 0, 1];
    const ic = (~sum16(icmp)) & 0xffff;
    icmp[2] = ic >> 8; icmp[3] = ic & 255;
    body.push(...ip, ...icmp);
  }
  const head = [...d, ...s];
  if (vlan) { head.push(0x81, 0x00, (vlan >> 8) & 0x0f, vlan & 255); }
  return new Uint8Array([...head, ...body]);
}
