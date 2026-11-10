// Node artwork. One SVG source per template, used both in the DOM (library,
// inspector, menus) and on the canvas (rasterised once into an Image).

const cyl = (id, top, side, dark, glyph) => `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs>
    <linearGradient id="${id}s" x1="0" x2="1">
      <stop offset="0" stop-color="${dark}"/><stop offset=".5" stop-color="${side}"/><stop offset="1" stop-color="${dark}"/>
    </linearGradient>
    <linearGradient id="${id}t" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${top}"/><stop offset="1" stop-color="${side}"/>
    </linearGradient>
  </defs>
  <path d="M6 26v14c0 6 11.6 10.5 26 10.5S58 46 58 40V26" fill="url(#${id}s)"/>
  <ellipse cx="32" cy="26" rx="26" ry="10.5" fill="url(#${id}t)"/>
  <ellipse cx="32" cy="26" rx="26" ry="10.5" fill="none" stroke="#fff" stroke-opacity=".25"/>
  ${glyph}
</svg>`;

// four arrows on the cylinder top — the classic router symbol
const routerArrows = `
  <g fill="#fff" transform="translate(32 26) scale(1 .42)">
    <path d="M-3-6h6v-8h5l-8-9-8 9h5z"/>
    <path d="M-3 6h6v8h5l-8 9-8-9h5z"/>
    <path d="M-6-3v6h-8v5l-9-8 9-8v5z" transform="translate(-1 0)"/>
    <path d="M6-3v6h8v5l9-8-9-8v5z" transform="translate(1 0)"/>
  </g>`;

const SVG = {
  router: cyl('rt', '#5aa9ff', '#2f7de1', '#1b4f99', routerArrows),
  ios: cyl('io', '#3fd3c4', '#119f94', '#0a5e58', routerArrows + `
    <text x="32" y="46" text-anchor="middle" font-family="system-ui,sans-serif" font-size="8.5" font-weight="700" fill="#fff" fill-opacity=".9">IOS</text>`),
  routeros: cyl('mk', '#b495ff', '#7a55e0', '#45308a', routerArrows + `
    <text x="32" y="46" text-anchor="middle" font-family="system-ui,sans-serif" font-size="8.5" font-weight="700" fill="#fff" fill-opacity=".9">ROS</text>`),
  x86: `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs><linearGradient id="vmg" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ffb86b"/><stop offset="1" stop-color="#e07a2e"/></linearGradient></defs>
  <rect x="8" y="12" width="48" height="36" rx="5" fill="#2b3442"/>
  <rect x="11" y="15" width="42" height="30" rx="3" fill="#161c26"/>
  <rect x="20" y="20" width="24" height="20" rx="2.5" fill="url(#vmg)"/>
  ${[0, 1, 2, 3].map(i => `<rect x="${23 + i * 5.5}" y="16.5" width="2" height="4" fill="#ffb86b"/><rect x="${23 + i * 5.5}" y="39.5" width="2" height="4" fill="#ffb86b"/>`).join('')}
  <text x="32" y="33.5" text-anchor="middle" font-family="ui-monospace,monospace" font-size="8" font-weight="700" fill="#1b1206">x86</text>
  <rect x="22" y="48" width="20" height="4" rx="2" fill="#4a566a"/>
</svg>`,
  switch: `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs>
    <linearGradient id="swt" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#62b4ff"/><stop offset="1" stop-color="#3b8ef0"/></linearGradient>
    <linearGradient id="sws" x1="0" x2="1"><stop offset="0" stop-color="#1d58a8"/><stop offset=".5" stop-color="#2c74d0"/><stop offset="1" stop-color="#1d58a8"/></linearGradient>
  </defs>
  <path d="M4 30l12-10h44L48 30z" fill="url(#swt)"/>
  <path d="M4 30h44v14H4z" fill="url(#sws)"/>
  <path d="M48 30l12-10v14L48 44z" fill="#174a8f"/>
  <g fill="#fff">
    <path d="M18 21.9h22v-1.5l5 2.2-5 2.2v-1.5H18z"/>
    <path d="M44 27.5H21v1.5l-5-2.2 5-2.2v1.5h23z"/>
  </g>
  <g fill="#0b2a52">${[0, 1, 2, 3, 4, 5, 6, 7].map(i => `<rect x="${7 + i * 5}" y="35" width="3.6" height="3" rx=".6"/>`).join('')}</g>
  <g fill="#7ff0a8">${[0, 1, 2, 3, 4, 5, 6, 7].map(i => `<rect x="${7.6 + i * 5}" y="40" width="2.4" height="1.2" rx=".5"/>`).join('')}</g>
</svg>`,
  pc: `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs><linearGradient id="pcs" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#7cc4ff"/><stop offset="1" stop-color="#2d6fd1"/></linearGradient></defs>
  <rect x="9" y="10" width="46" height="32" rx="3.5" fill="#2b3442"/>
  <rect x="12" y="13" width="40" height="26" rx="1.5" fill="url(#pcs)"/>
  <path d="M12 13h40L12 33z" fill="#fff" fill-opacity=".12"/>
  <path d="M27 42h10l2 8H25z" fill="#3a4556"/>
  <rect x="19" y="49" width="26" height="4" rx="2" fill="#4a566a"/>
</svg>`,
  laptop: `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs><linearGradient id="lps" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#7cc4ff"/><stop offset="1" stop-color="#2d6fd1"/></linearGradient></defs>
  <rect x="12" y="14" width="40" height="27" rx="3" fill="#2b3442"/>
  <rect x="15" y="17" width="34" height="21" rx="1.2" fill="url(#lps)"/>
  <path d="M15 17h34L15 33z" fill="#fff" fill-opacity=".12"/>
  <path d="M6 43h52l-4 6H10z" fill="#4a566a"/>
  <rect x="27" y="43.6" width="10" height="1.6" rx=".8" fill="#2b3442"/>
</svg>`,
  server: `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  <defs><linearGradient id="svs" x1="0" x2="1"><stop offset="0" stop-color="#3a4556"/><stop offset="1" stop-color="#252d3a"/></linearGradient></defs>
  <rect x="18" y="6" width="28" height="52" rx="3" fill="url(#svs)"/>
  ${[0, 1, 2].map(i => `<rect x="21" y="${10 + i * 11}" width="22" height="8" rx="1.4" fill="#151b25"/>
  <circle cx="39" cy="${14 + i * 11}" r="1.4" fill="#7ff0a8"/>
  <rect x="24" y="${13 + i * 11}" width="10" height="1.6" rx=".8" fill="#5b6a80"/>`).join('')}
  <circle cx="32" cy="50" r="2.6" fill="none" stroke="#5aa9ff" stroke-width="1.4"/>
</svg>`,
};

export const TEMPLATES = [
  { id: 'pc', kind: 'host', label: 'VPCS', sub: 'Virtual PC with a shell', group: 'End devices', prefix: 'PC', ports: 1, maxPorts: 8 },
  { id: 'laptop', kind: 'host', label: 'Laptop', sub: 'End host, multi-NIC ready', group: 'End devices', prefix: 'Laptop', ports: 1, maxPorts: 8 },
  { id: 'server', kind: 'host', label: 'Server', sub: 'End host, dual NIC default', group: 'End devices', prefix: 'Server', ports: 2, maxPorts: 8 },
  { id: 'switch', kind: 'switch', label: 'Ethernet switch', sub: '8–16 port L2, MAC learning', group: 'Switching', prefix: 'SW', ports: 8, maxPorts: 16 },
  { id: 'router', kind: 'router', label: 'Router', sub: '4–8 port L3, static routes', group: 'Routing', prefix: 'R', ports: 4, maxPorts: 8 },
  { id: 'ios', kind: 'ext', emu: 'dynamips', label: 'Cisco 7200', sub: 'Real IOS · your image', group: 'Emulated', prefix: 'IOS', ports: 4, maxPorts: 7 },
  { id: 'routeros', kind: 'ext', emu: 'v86', label: 'MikroTik RouterOS', sub: 'RouterOS v6 x86 · 4× Ethernet', group: 'Emulated', prefix: 'MT', ports: 4, maxPorts: 8 },
  { id: 'x86', kind: 'ext', emu: 'v86', label: 'x86 VM', sub: '32-bit x86 ISO/disk · multi-NIC', group: 'Emulated', prefix: 'VM', ports: 4, maxPorts: 8 },
];
export const tpl = id => TEMPLATES.find(t => t.id === id);

export function iconSVG(id) { return SVG[id] ?? SVG.pc; }

const images = {};
export function iconImage(id) {
  if (!images[id]) {
    const img = new Image();
    img.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(iconSVG(id).trim());
    images[id] = img;
  }
  return images[id];
}
export function preloadIcons() {
  return Promise.all(Object.keys(SVG).map(id => {
    const img = iconImage(id);
    return img.complete ? null : new Promise(r => { img.onload = img.onerror = r; });
  }));
}

/** interface naming per template, GNS3-style */
export function ifName(template, i) {
  switch (template) {
    case 'switch': return `e${i}`;
    case 'router': return `g0/${i}`;
    case 'ios': return `f${i}/0`;
    case 'routeros': return `ether${i + 1}`;
    default: return `eth${i}`;
  }
}

// small UI glyphs (stroke icons, currentColor)
const g = d => `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${d}</svg>`;
export const UI = {
  select: g('<path d="M3 2l9 5-4 1.2L6.5 12z"/>'),
  cable: g('<path d="M2 13c3 0 3-10 6-10s3 10 6 10"/><circle cx="2" cy="13" r="1.2" fill="currentColor"/><circle cx="14" cy="13" r="1.2" fill="currentColor"/>'),
  note: g('<path d="M3 3h10v7l-3 3H3z"/><path d="M10 13v-3h3"/>'),
  rect: g('<rect x="2.5" y="3.5" width="11" height="9" rx="1.5"/>'),
  ellipse: g('<ellipse cx="8" cy="8" rx="5.8" ry="4.6"/>'),
  line: g('<path d="M3 13L13 3"/><circle cx="3" cy="13" r="1.3" fill="currentColor"/><circle cx="13" cy="3" r="1.3" fill="currentColor"/>'),
  front: g('<rect x="2" y="2" width="8" height="8" rx="1"/><rect x="6" y="6" width="8" height="8" rx="1" fill="currentColor" fill-opacity=".35"/>'),
  back: g('<rect x="6" y="6" width="8" height="8" rx="1"/><rect x="2" y="2" width="8" height="8" rx="1" fill="currentColor" fill-opacity=".35"/>'),
  hand: g('<path d="M5 8V3.5a1 1 0 012 0V7m0-3.5a1 1 0 012 0V7m0-2.5a1 1 0 012 0V8m0-1.5a1 1 0 012 0V10a4 4 0 01-4 4H8.5A4 4 0 015 12L3.2 9.2a1 1 0 011.6-1.2L5 8.5"/>'),
  play: g('<path d="M5 3l8 5-8 5z" fill="currentColor"/>'),
  stop: g('<rect x="4" y="4" width="8" height="8" rx="1" fill="currentColor"/>'),
  pause: g('<path d="M5 3v10M11 3v10"/>'),
  step: g('<path d="M4 3l6 5-6 5z" fill="currentColor"/><path d="M12.5 3v10"/>'),
  reload: g('<path d="M13 8a5 5 0 11-1.5-3.6M13 2.5V5h-2.5"/>'),
  term: g('<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M5 7l2 1.5L5 10M8.5 10.5H11"/>'),
  trash: g('<path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5"/>'),
  zoomIn: g('<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5L14 14M5 7h4M7 5v4"/>'),
  zoomOut: g('<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5L14 14M5 7h4"/>'),
  fit: g('<path d="M2 6V2h4M10 2h4v4M14 10v4h-4M6 14H2v-4"/>'),
  grid: g('<path d="M2 6h12M2 10h12M6 2v12M10 2v12"/>'),
  tag: g('<path d="M2 2h6l6 6-6 6-6-6z"/><circle cx="5.5" cy="5.5" r="1"/>'),
  map: g('<rect x="2" y="3" width="12" height="10" rx="1.5"/><rect x="8" y="8" width="4" height="3" fill="currentColor"/>'),
  capture: g('<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5L14 14"/><path d="M4.8 7h1.2l.8-1.6 1 3.2.8-1.6h1.2"/>'),
  unplug: g('<path d="M6 10L3 13M10 6l3-3M5 6l5 5M7 4l5 5M4 7l2-2M9 12l2-2"/>'),
  plug: g('<path d="M5 11l-2 2M11 5l2-2M4 8l4 4 2-2-4-4zM8 4l4 4-2 2-4-4z"/>'),
  download: g('<path d="M8 2v8M4.5 6.5L8 10l3.5-3.5M3 13h10"/>'),
  upload: g('<path d="M8 10V2M4.5 5.5L8 2l3.5 3.5M3 13h10"/>'),
  plus: g('<path d="M8 3v10M3 8h10"/>'),
  x: g('<path d="M4 4l8 8M12 4l-8 8"/>'),
  chev: g('<path d="M6 4l4 4-4 4"/>'),
  search: g('<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5L14 14"/>'),
  sun: g('<circle cx="8" cy="8" r="3"/><path d="M8 1.5v1.5M8 13v1.5M1.5 8H3M13 8h1.5M3.4 3.4l1 1M11.6 11.6l1 1M3.4 12.6l1-1M11.6 4.4l1-1"/>'),
  moon: g('<path d="M13 9.5A5.5 5.5 0 016.5 3a5.5 5.5 0 106.5 6.5z"/>'),
  image: g('<rect x="2" y="3" width="12" height="10" rx="1.5"/><circle cx="6" cy="6.5" r="1.2"/><path d="M14 11l-3.5-3.5L4 13"/>'),
  chip: g('<rect x="4" y="4" width="8" height="8" rx="1"/><path d="M6 2v2M10 2v2M6 12v2M10 12v2M2 6h2M2 10h2M12 6h2M12 10h2"/>'),
  minus: g('<path d="M3 8h10"/>'),
  dock: g('<rect x="2" y="2.5" width="12" height="11" rx="1.5"/><path d="M2 10h12"/>'),
  templates: g('<rect x="2" y="2" width="5" height="5" rx="1"/><rect x="9" y="2" width="5" height="5" rx="1"/><rect x="2" y="9" width="5" height="5" rx="1"/><rect x="9" y="9" width="5" height="5" rx="1"/>'),
  info: g('<circle cx="8" cy="8" r="6"/><path d="M8 7v4M8 5h.01"/>'),
};
