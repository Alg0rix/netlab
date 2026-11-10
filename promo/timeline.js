// Shared timeline for the netlab showreel. Imported by reel.html (picture),
// render.mjs (frames → mp4) and music.mjs (soundtrack + synced sound effects).
// Tempo is 120 BPM, so every scene boundary lands on a bar line (2 s).

export const FPS = 60;
export const BPM = 120;
export const DURATION = 88;

export const SCENES = {
  open:    [0, 6],     // a single packet grows into a network
  problem: [6, 12],    // what a lab normally costs
  reveal:  [12, 20],   // netlab, in a browser tab
  how:     [20, 32],   // two engines, one virtual wire
  demo:    [32, 70],   // real footage of the app
  features:[70, 78],   // feature montage + numbers
  outro:   [78, 88],
};

// ── Demo footage ────────────────────────────────────────────────────────────
// Clip times refer to promo/capture/out/<clip>/clip.json (recorded by
// capture/record.js). Each segment plays [a, b] of a clip at `speed`.
export const DEMO_START = 33.5;
export const CHAPTERS = [
  { n: '01', title: 'Drop devices on the canvas', sub: 'PCs, switches and routers boot instantly' },
  { n: '02', title: 'Cable them up', sub: 'Click a device, pick an interface, click the next' },
  { n: '03', title: 'Address it. Ping it.', sub: 'A real shell on every node' },
  { n: '04', title: 'Watch every packet', sub: 'Simulation mode steps the network hop by hop' },
  { n: '05', title: 'Capture. Decode. Export.', sub: 'Live sniffer on any link → .pcap for Wireshark' },
];
export const SEGMENTS = [
  { ch: 0, clip: 'build', a: 0.6, b: 6.95, speed: 1.45 },
  { ch: 1, clip: 'build', a: 6.95, b: 14.4, speed: 1.5 },
  { ch: 2, clip: 'build', a: 14.4, b: 25.3, speed: 1.75 },
  { ch: 3, clip: 'sim', a: 0.6, b: 6.3, speed: 2.4 },
  { ch: 3, clip: 'sim', a: 6.3, b: 17.4, speed: 1.35 },
  { ch: 4, clip: 'cap', a: 0.6, b: 3.2, speed: 1.3 },
  { ch: 4, clip: 'cap', a: 3.2, b: 11.0, speed: 2.6 },
  { ch: 4, clip: 'cap', a: 11.0, b: 17.8, speed: 1.3 },
];
{
  let t = DEMO_START;
  for (const s of SEGMENTS) { s.t0 = t; t += (s.b - s.a) / s.speed; s.t1 = t; }
}
export const DEMO_END = SEGMENTS[SEGMENTS.length - 1].t1;

// composition time → { seg, clip, ct } (clamped to the demo range)
export function footageAt(T) {
  let s = SEGMENTS[0];
  for (const x of SEGMENTS) if (T >= x.t0) s = x;
  const ct = Math.min(s.b, s.a + Math.max(0, T - s.t0) * s.speed);
  return { seg: s, clip: s.clip, ct };
}
// clip time → composition time (for syncing sounds to recorded clicks)
export function compTimeOf(clip, ct) {
  for (const s of SEGMENTS) if (s.clip === clip && ct >= s.a && ct < s.b) return s.t0 + (ct - s.a) / s.speed;
  return null;
}

// Camera keyframes per clip, in app CSS px (the app was recorded at 1600×900): [clipTime, cx, cy, zoom]
export const CAMERA = {
  build: [[0.6, 800, 450, 1], [1.1, 615, 330, 1.3], [6.6, 615, 330, 1.3], [7.3, 760, 360, 1.6], [14.1, 760, 360, 1.6],
          [14.8, 640, 560, 1.32], [20.3, 640, 560, 1.32], [21.0, 520, 700, 1.6], [25.3, 520, 700, 1.6]],
  sim:   [[0.6, 800, 450, 1], [6.4, 800, 450, 1], [7.4, 700, 300, 1.42], [16.3, 700, 300, 1.42], [17.4, 720, 320, 1.3]],
  cap:   [[0.6, 800, 450, 1], [1.0, 720, 300, 1.45], [3.0, 720, 300, 1.45], [3.8, 800, 450, 1], [10.4, 800, 450, 1],
          [11.2, 700, 700, 1.42], [17.8, 700, 700, 1.42]],
};

// Callouts pinned to app coordinates: [clip, from, to, x, y, text, side]
export const CALLOUTS = [
  ['build', 2.4, 6.8, 560, 470, 'Boots instantly — no VM, no image', 'r'],
  ['build', 11.0, 14.3, 760, 250, 'Every interface, named like real gear', 'l'],
  ['build', 22.0, 25.3, 380, 780, 'Real ICMP over a simulated wire', 'r'],
  ['sim', 1.5, 6.0, 1318, 30, 'One toggle: Realtime ⇄ Simulation', 'b'],
  ['sim', 8.0, 16.8, 787, 182, 'ARP floods, ICMP hops — frame by frame', 'r'],
  ['cap', 1.5, 3.1, 701, 258, 'Right-click any link → Start capture', 'r'],
  ['cap', 12.8, 15.0, 420, 760, 'Decoded like Wireshark — Ethernet, IP, ICMP', 'r'],
  ['cap', 15.2, 17.8, 1220, 548, 'One click → .pcap', 'l'],
];

// Sound-design cues (seconds). Clicks from the footage are added by music.mjs.
export const CUES = {
  hits: [12, 20, 32, 70, 78, 84],
  whooshes: [5.55, 11.55, 19.45, 31.35, 33.2, 69.55, 77.5],
  risers: [[9, 12], [29, 32], [66, 70]],
  pops: [1.0, 1.35, 1.7, 2.05, 2.4, 7.0, 7.45, 7.9, 8.35, 8.8, 12.9, 13.05, 13.2, 21.3, 21.6, 21.9, 22.2, 22.5, 23.0, 23.5, 24.0,
         70.3, 70.42, 70.54, 70.66, 70.78, 70.9, 78.6, 78.75, 78.9],
  ticks: [12.65, 12.75, 12.85, 12.95, 13.05, 13.15, 13.25, 13.35],
};
