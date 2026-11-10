// Shared helpers — DOM, masks, encodings. One home so main/cli/capture/
// templates don't each grow a private copy.

export const $ = (s, r = document) => r.querySelector(s);
export const $$ = (s, r = document) => [...r.querySelectorAll(s)];

export const h = html => {
  const t = document.createElement('template');
  t.innerHTML = html.trim();
  return t.content.firstElementChild;
};

export const esc = s => String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

export const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

export const isTyping = e => e.target.closest?.('input, textarea, select, [contenteditable], .term');

export const b64 = buf => {
  let s = '';
  const u = new Uint8Array(buf);
  for (let i = 0; i < u.length; i += 0x8000) s += String.fromCharCode(...u.subarray(i, i + 0x8000));
  return btoa(s);
};
export const unb64 = str => Uint8Array.from(atob(str), c => c.charCodeAt(0));

export const PROTO_NAME = { 1: 'icmp', 6: 'tcp', 17: 'udp' };

export function maskLen(m) {
  return (m ?? '').split('.').reduce((s, o) => s + ((+o).toString(2).match(/1/g)?.length ?? 0), 0);
}

export function lenMask(n) {
  return [0, 1, 2, 3].map(i => { const b = clamp(n - i * 8, 0, 8); return (0xff << (8 - b)) & 0xff; }).join('.');
}
