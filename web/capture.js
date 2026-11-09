// Packet dissection (Ethernet II / ARP / IPv4 / ICMP / UDP / TCP), hex dump
// and libpcap export so captures open in Wireshark.

export const TICK_MS = 0.1; // simulated time per tick (ms) — capture timestamps, ping RTT

const hex = (n, w = 2) => n.toString(16).padStart(w, '0');
const mac = (b, o) => Array.from(b.subarray(o, o + 6), x => hex(x)).join(':');
const ip = (b, o) => `${b[o]}.${b[o + 1]}.${b[o + 2]}.${b[o + 3]}`;
const u16 = (b, o) => (b[o] << 8) | b[o + 1];

const ETHERTYPES = { 0x0800: 'IPv4', 0x0806: 'ARP', 0x86dd: 'IPv6', 0x8100: '802.1Q', 0x9000: 'Loopback' };
const IPPROTO = { 1: 'ICMP', 6: 'TCP', 17: 'UDP', 89: 'OSPF', 88: 'EIGRP' };
const ICMPTYPE = { 0: 'Echo (ping) reply', 3: 'Destination unreachable', 8: 'Echo (ping) request', 11: 'Time exceeded' };

/**
 * Returns { proto, src, dst, info, tree } where tree is
 * [{ title, range:[start,end], fields:[{ k, v, range }] }]
 */
export function dissect(b) {
  const tree = [];
  const out = { proto: 'ETH', src: '', dst: '', info: '', tree };
  if (b.length < 14) { out.info = 'Truncated frame'; return out; }

  let et = u16(b, 12), off = 14;
  out.src = mac(b, 6); out.dst = mac(b, 0);
  const eth = {
    title: `Ethernet II, Src: ${mac(b, 6)}, Dst: ${mac(b, 0)}`, range: [0, 14], fields: [
      { k: 'Destination', v: mac(b, 0) + (b[0] === 0xff && b[5] === 0xff ? ' (Broadcast)' : ''), range: [0, 6] },
      { k: 'Source', v: mac(b, 6), range: [6, 12] },
      { k: 'Type', v: `${ETHERTYPES[et] ?? 'Unknown'} (0x${hex(et, 4)})`, range: [12, 14] },
    ],
  };
  tree.push(eth);
  if (et < 0x0600) { out.proto = 'LLC'; out.info = `802.3 length ${et}`; return out; }
  if (et === 0x8100 && b.length >= 18) {
    tree.push({ title: `802.1Q Virtual LAN, VID: ${u16(b, 14) & 0xfff}`, range: [14, 18], fields: [
      { k: 'Priority', v: String(b[14] >> 5), range: [14, 15] },
      { k: 'ID', v: String(u16(b, 14) & 0xfff), range: [14, 16] },
    ] });
    et = u16(b, 16); off = 18;
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
      out.info = `${sp} → ${dp}` + (proto === 17 ? `  Len=${u16(b, p + 4) - 8}` : '');
      tree.push({ title: `${proto === 17 ? 'User Datagram' : 'Transmission Control'} Protocol, Src Port: ${sp}, Dst Port: ${dp}`, range: [p, b.length], fields: [
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
