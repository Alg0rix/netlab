// Synthesises the showreel soundtrack from scratch (no samples): a 120 BPM
// A-minor track whose sections and hits line up with timeline.js, plus sound
// design — risers, impacts, whooshes, pops and UI clicks synced to the clicks
// recorded in the demo footage.
//
//   node promo/music.mjs   → promo/build/music.wav (+ music.m4a for the preview page)
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { DURATION, CUES, SEGMENTS, CHAPTERS, compTimeOf } from './timeline.js';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SR = 48000, LEN = Math.ceil((DURATION + 0.5) * SR);
const TAU = Math.PI * 2;
const bus = () => [new Float32Array(LEN), new Float32Array(LEN)];
const DRUMS = bus(), MUSIC = bus(), FX = bus(), SEND = bus();
const DUCK = new Float32Array(LEN).fill(1);

let seed = 1;
const rnd = () => { seed = (seed * 16807) % 2147483647; return seed / 2147483647; };
const noise = () => rnd() * 2 - 1;
const S = t => Math.floor(t * SR);
function add(b, i, l, r = l) { if (i >= 0 && i < LEN) { b[0][i] += l; b[1][i] += r; } }
function pan(p) { return [Math.cos((p + 1) * Math.PI / 4), Math.sin((p + 1) * Math.PI / 4)]; }

// RBJ biquad
function biquad(type, f, q) {
  const w = TAU * Math.min(f, SR * .45) / SR, cs = Math.cos(w), al = Math.sin(w) / (2 * q);
  let b0, b1, b2, a0 = 1 + al, a1 = -2 * cs, a2 = 1 - al;
  if (type === 'lp') { b0 = (1 - cs) / 2; b1 = 1 - cs; b2 = b0; }
  else if (type === 'hp') { b0 = (1 + cs) / 2; b1 = -(1 + cs); b2 = b0; }
  else { b0 = al; b1 = 0; b2 = -al; }
  return { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0, x1: 0, x2: 0, y1: 0, y2: 0 };
}
function run(f, x) { const y = f.b0 * x + f.b1 * f.x1 + f.b2 * f.x2 - f.a1 * f.y1 - f.a2 * f.y2; f.x2 = f.x1; f.x1 = x; f.y2 = f.y1; f.y1 = y; return y; }
function retune(f, type, fr, q) { const n = biquad(type, fr, q); Object.assign(f, { b0: n.b0, b1: n.b1, b2: n.b2, a1: n.a1, a2: n.a2 }); }

/* ───────────── instruments ───────────── */
function kick(t0, amp = 1, duck = true) {
  const i0 = S(t0), n = S(.5);
  let ph = 0;
  for (let k = 0; k < n; k++) {
    const t = k / SR, f = 44 + 120 * Math.exp(-t * 30);
    ph += TAU * f / SR;
    const v = (Math.sin(ph) * Math.exp(-t * 6.5) + (k < 120 ? noise() * .3 * (1 - k / 120) : 0)) * amp;
    add(DRUMS, i0 + k, v * .9);
  }
  if (duck) for (let k = 0; k < S(.42); k++) { const i = i0 + k; if (i < LEN) DUCK[i] = Math.min(DUCK[i], 1 - .55 * Math.exp(-k / SR * 9) * Math.min(1, k / 60)); }
}
function clap(t0, amp = 1, p = 0) {
  const i0 = S(t0), f = biquad('bp', 1400, .9), [l, r] = pan(p);
  for (let k = 0; k < S(.35); k++) {
    const t = k / SR;
    let env = 0;
    for (const o of [0, .011, .022]) if (t >= o) env = Math.max(env, Math.exp(-(t - o) * (o === .022 ? 16 : 120)));
    const v = run(f, noise()) * env * amp * 1.6;
    add(DRUMS, i0 + k, v * l, v * r); add(SEND, i0 + k, v * .35 * l, v * .35 * r);
  }
}
function hat(t0, amp = .3, open = false, p = .2) {
  const i0 = S(t0), f = biquad('hp', 7500, .7), [l, r] = pan(p), d = open ? 9 : 55;
  for (let k = 0; k < S(open ? .35 : .08); k++) { const v = run(f, noise()) * Math.exp(-k / SR * d) * amp; add(DRUMS, i0 + k, v * l, v * r); }
}
function crash(t0, amp = .5) {
  const i0 = S(t0), f = biquad('hp', 4200, .5);
  for (let k = 0; k < S(2.6); k++) { const v = run(f, noise()) * Math.exp(-k / SR * 1.8) * amp; add(DRUMS, i0 + k, v * (.8 + .2 * noise()), v * (.8 + .2 * noise())); add(SEND, i0 + k, v * .3); }
}
function bass(t0, dur, freq, amp = .5) {
  const i0 = S(t0), n = S(dur), f = biquad('lp', 300, 1.1);
  let ph = 0, ph2 = 0;
  for (let k = 0; k < n + S(.05); k++) {
    const t = k / SR;
    if (k % 32 === 0) retune(f, 'lp', 160 + 900 * Math.exp(-t * 14), 1.4);
    ph = (ph + freq / SR) % 1; ph2 += TAU * freq / SR;
    const env = Math.min(1, t / .004) * (k < n ? 1 : Math.max(0, 1 - (k - n) / S(.05)));
    const v = (run(f, ph * 2 - 1) * .8 + Math.sin(ph2) * .55) * env * amp;
    add(MUSIC, i0 + k, v);
  }
}
function pad(t0, dur, freqs, amp = .12, cutoff = 1400) {
  const i0 = S(t0), n = S(dur), rel = S(1.2);
  const fl = biquad('lp', cutoff, .7), fr = biquad('lp', cutoff, .7);
  const osc = freqs.flatMap(f => [-1, 0, 1].map(d => ({ f: f * Math.pow(2, d * 9 / 1200), ph: rnd(), side: d })));
  for (let k = 0; k < n + rel; k++) {
    const t = k / SR, env = Math.min(1, t / .5) * (k < n ? 1 : Math.max(0, 1 - (k - n) / rel));
    let l = 0, r = 0;
    for (const o of osc) { o.ph = (o.ph + o.f / SR) % 1; const v = o.ph * 2 - 1; l += v * (o.side <= 0 ? 1 : .4); r += v * (o.side >= 0 ? 1 : .4); }
    l = run(fl, l) * env * amp / osc.length * 3; r = run(fr, r) * env * amp / osc.length * 3;
    l *= 1.35; r *= 1.35; add(MUSIC, i0 + k, l, r); add(SEND, i0 + k, l * .5, r * .5);
  }
}
function pluck(t0, freq, amp = .12, p = 0, decay = 7) {
  const i0 = S(t0), [l, r] = pan(p);
  let ph = 0;
  for (let k = 0; k < S(.6); k++) {
    const t = k / SR; ph += TAU * freq / SR;
    const v = (Math.sin(ph) + .35 * Math.sin(2 * ph) + .15 * Math.sin(3 * ph)) * Math.exp(-t * decay) * Math.min(1, t / .002) * amp * 1.4;
    add(MUSIC, i0 + k, v * l, v * r);
    // dotted-eighth echo, ping-pong
    for (const [d, g, s] of [[.375, .35, -1], [.75, .15, 1]]) add(SEND, i0 + k + S(d), v * g * (s < 0 ? 1 : .3), v * g * (s > 0 ? 1 : .3));
  }
}
function riser(t0, t1, amp = .35) {
  const i0 = S(t0), n = S(t1 - t0), f = biquad('bp', 300, 2);
  let ph = 0;
  for (let k = 0; k < n; k++) {
    const u = k / n;
    if (k % 32 === 0) retune(f, 'bp', 250 * Math.pow(30, u), 2.5);
    ph += TAU * (180 * Math.pow(6, u)) / SR;
    const v = (run(f, noise()) * 1.4 + Math.sin(ph) * .12 * u) * Math.pow(u, 1.8) * amp;
    const p = Math.sin(u * TAU * 3) * .5, [l, r] = pan(p);
    add(FX, i0 + k, v * l, v * r); add(SEND, i0 + k, v * .3);
  }
}
function impact(t0, amp = 1) {
  const i0 = S(t0), f = biquad('lp', 900, .7);
  let ph = 0;
  for (let k = 0; k < S(3); k++) {
    const t = k / SR; ph += TAU * (32 + 60 * Math.exp(-t * 5)) / SR;
    const v = (Math.sin(ph) * Math.exp(-t * 1.6) * .9 + run(f, noise()) * Math.exp(-t * 7) * .8) * amp;
    add(FX, i0 + k, v); add(SEND, i0 + k, v * .45);
  }
  crash(t0, .32 * amp);
}
function whoosh(t0, amp = .3, dir = 1) {
  const i0 = S(t0 - .35), n = S(.75), f = biquad('bp', 500, 1.2);
  for (let k = 0; k < n; k++) {
    const u = k / n, env = Math.sin(Math.PI * Math.pow(u, .7));
    if (k % 32 === 0) retune(f, 'bp', 400 + 4200 * Math.sin(Math.PI * u), 1.4);
    const v = run(f, noise()) * env * env * amp * 1.8, [l, r] = pan(dir * (u * 2 - 1) * .8);
    add(FX, i0 + k, v * l, v * r); add(SEND, i0 + k, v * .25);
  }
}
function pop(t0, amp = .16, base = 700) {
  const i0 = S(t0);
  let ph = 0;
  for (let k = 0; k < S(.12); k++) { const t = k / SR; ph += TAU * (base + 900 * Math.min(1, t * 30)) / SR; add(FX, i0 + k, Math.sin(ph) * Math.exp(-t * 38) * amp); }
}
function uiClick(t0, amp = .5, right = false) {
  const i0 = S(t0), f = biquad('bp', right ? 1800 : 3200, 3);
  let ph = 0;
  for (let k = 0; k < S(.05); k++) {
    const t = k / SR; ph += TAU * 140 / SR;
    const v = (run(f, noise()) * Math.exp(-t * 260) * 1.6 + Math.sin(ph) * Math.exp(-t * 90) * .35) * amp;
    add(FX, i0 + k, v);
  }
}
function tick(t0, amp = .22, hi = true) {
  const i0 = S(t0), f = biquad('bp', hi ? 5200 : 3400, 6);
  for (let k = 0; k < S(.04); k++) add(FX, i0 + k, run(f, noise()) * Math.exp(-k / SR * 180) * amp * 2);
}

/* ───────────── arrangement ───────────── */
const N = { A1: 55, F1: 43.65, C2: 65.41, G1: 49, E1: 41.2, D2: 73.42 };
const CHORD = [
  { b: N.A1, pad: [220, 261.63, 329.63], arp: [440, 523.25, 659.25, 880] },     // Am
  { b: N.F1, pad: [174.61, 220, 261.63], arp: [349.23, 440, 523.25, 698.46] },  // F
  { b: N.C2, pad: [261.63, 329.63, 392], arp: [523.25, 659.25, 783.99, 1046.5] }, // C
  { b: N.G1, pad: [196, 246.94, 293.66], arp: [392, 493.88, 587.33, 783.99] },  // G
];
const BAR = 2, BEAT = .5;
const section = t => t < 6 ? 'open' : t < 12 ? 'problem' : t < 20 ? 'reveal' : t < 32 ? 'how' : t < 70 ? 'demo' : t < 78 ? 'features' : 'outro';

for (let bar = 0; bar < 44; bar++) {
  const t0 = bar * BAR, sec = section(t0 + .01), ch = CHORD[bar % 4];
  if (t0 >= DURATION) break;
  // pads
  if (sec === 'open') pad(t0, BAR, ch.pad, .1 * Math.min(1, (bar + 1) / 2), 900 + bar * 250);
  else if (sec === 'problem') pad(t0, BAR, bar === 5 ? [164.81, 207.65, 246.94] : [174.61, 207.65, 261.63], .1, 700);  // F→Fm(add)→E: tension
  else if (sec === 'outro') { if (bar === 39) pad(t0, 3, ch.pad, .13, 1800); else if (bar === 42) pad(84, 4, [220, 261.63, 329.63, 440], .15, 2200); else if (bar < 42) pad(t0, BAR, ch.pad, .12, 1800); }
  else pad(t0, BAR, ch.pad, sec === 'demo' ? .07 : .1, sec === 'demo' ? 1300 : 2000);

  for (let b = 0; b < 4; b++) {
    const tb = t0 + b * BEAT;
    if (tb >= DURATION - .2) continue;
    const s = section(tb + .01);
    const groove = s === 'reveal' || s === 'how' || s === 'demo' || s === 'features' || (s === 'outro' && tb < 84);
    // kick
    if (s === 'open' && tb >= 2) kick(tb, .55, false);
    if (s === 'problem') kick(tb, .8);
    if (groove) kick(tb, s === 'demo' ? .75 : .95);
    // claps on 2 & 4
    if (groove && (b === 1 || b === 3)) clap(tb, s === 'demo' ? .4 : .6, b === 1 ? -.1 : .1);
    // hats
    if (groove) { hat(tb + .25, s === 'demo' ? .16 : .22, true, .25); for (let h = 0; h < 4; h++) if (h !== 2) hat(tb + h * .125, .09 + (h === 0 ? .03 : 0), false, -.2); }
    if (s === 'open' && tb >= 4) for (let h = 0; h < 4; h++) hat(tb + h * .125, .05, false, -.2);
    if (s === 'problem') hat(tb + .25, .14, false, .3);
    // bass
    if (groove || s === 'problem') {
      const f = s === 'problem' ? (bar === 5 ? N.E1 : N.F1) : ch.b;
      for (let e = 0; e < 2; e++) {
        const te = tb + e * .25;
        const oct = s !== 'problem' && e === 1 && (b === 1 || b === 3) ? 2 : 1;
        bass(te + (e === 0 ? .02 : 0), .2, f * oct, s === 'demo' ? .32 : .42);
      }
    }
  }
  // arps (16ths)
  if (sec === 'open' || sec === 'reveal' || sec === 'how' || sec === 'demo' || sec === 'features' || (sec === 'outro' && t0 < 84)) {
    const pat = [0, 2, 1, 3, 2, 0, 3, 1];
    const step = sec === 'open' ? .5 : .25;
    for (let k = 0; k < BAR / step; k++) {
      const tt = t0 + k * step;
      if (sec === 'open' && tt < .8) continue;
      const amp = sec === 'open' ? .05 + .02 * bar : sec === 'demo' ? .045 : .07;
      pluck(tt, ch.arp[pat[k % 8]], amp, k % 2 ? .45 : -.45, sec === 'open' ? 5 : 8);
    }
  }
}
// setup-timer clock ticks during "the problem"
for (let t = 7.0; t < 10.4; t += .25) tick(t, .16, Math.round((t - 7) / .25) % 2 === 0);
// snare roll into the reveal
for (let k = 0; k < 24; k++) { const t = 11 + (1 - Math.pow(1 - k / 24, 1.5)) * .98; clap(t, .12 + .3 * k / 24, (k % 2) - .5); }
// sound design from the timeline
for (const [a, b] of CUES.risers) riser(a, b);
for (const t of CUES.hits) impact(t, t === 84 ? 1.2 : t === 12 ? 1.1 : .85);
for (const [i, t] of CUES.whooshes.entries()) whoosh(t, .28, i % 2 ? -1 : 1);
for (const [i, t] of CUES.pops.entries()) pop(t, .13, 600 + (i % 5) * 120);
for (const t of CUES.ticks) tick(t, .12);
for (const t of [75.35, 75.53, 75.71]) { kick(t, .7, false); pop(t, .1, 400); }   // the three stats landing
// chapter swishes
CHAPTERS.forEach((_, i) => { const s = SEGMENTS.find(x => x.ch === i); if (i) whoosh(s.t0 + .1, .14, i % 2 ? 1 : -1); });
// UI clicks synced to the recorded footage
for (const clip of ['build', 'sim', 'cap']) {
  const j = JSON.parse(fs.readFileSync(path.join(HERE, 'capture/out', clip, 'clip.json')));
  for (const c of j.clicks) { const t = compTimeOf(clip, c.t); if (t != null) uiClick(t, .55, c.button === 'right'); }
}

/* ───────────── reverb (Schroeder) on the send bus ───────────── */
function reverb(inp, out, mix = .32) {
  const combs = [1557, 1617, 1491, 1422, 1277, 1356].map(d => ({ d: Math.round(d * SR / 44100 * 1.25), buf: null, i: 0, f: 0 }));
  const alls = [225, 556, 441].map(d => ({ d: Math.round(d * SR / 44100), buf: null, i: 0 }));
  for (let ch = 0; ch < 2; ch++) {
    const cs = combs.map((c, k) => ({ ...c, d: c.d + ch * 23 + k, buf: new Float32Array(c.d + ch * 23 + k), i: 0, f: 0 }));
    const as = alls.map(a => ({ ...a, buf: new Float32Array(a.d), i: 0 }));
    for (let n = 0; n < LEN; n++) {
      const x = inp[ch][n] * .12;
      let y = 0;
      for (const c of cs) { const o = c.buf[c.i]; c.f = o * .7 + c.f * .3; c.buf[c.i] = x + c.f * .86; c.i = (c.i + 1) % c.d; y += o; }
      for (const a of as) { const o = a.buf[a.i]; const v = y + o * -.5; a.buf[a.i] = v; a.i = (a.i + 1) % a.d; y = o + v * .5; }
      out[ch][n] += y * mix;
    }
  }
}

/* ───────────── mix ───────────── */
const OUT = bus();
for (let n = 0; n < LEN; n++) for (let ch = 0; ch < 2; ch++) OUT[ch][n] = DRUMS[ch][n] * .9 + MUSIC[ch][n] * DUCK[n] * .95 + FX[ch][n] * .85;
reverb(SEND, OUT);
// master: gentle fade-out, soft clip, normalise
let peak = 0;
for (let n = 0; n < LEN; n++) {
  const t = n / SR, fade = t > DURATION - 1.6 ? Math.max(0, (DURATION - t) / 1.6) : 1, fin = Math.min(1, t / .05);
  for (let ch = 0; ch < 2; ch++) { const v = Math.tanh(OUT[ch][n] * 1.1) * fade * fin; OUT[ch][n] = v; peak = Math.max(peak, Math.abs(v)); }
}
const gain = .89 / peak;
const buf = Buffer.alloc(44 + LEN * 4);
buf.write('RIFF', 0); buf.writeUInt32LE(36 + LEN * 4, 4); buf.write('WAVEfmt ', 8);
buf.writeUInt32LE(16, 16); buf.writeUInt16LE(1, 20); buf.writeUInt16LE(2, 22); buf.writeUInt32LE(SR, 24);
buf.writeUInt32LE(SR * 4, 28); buf.writeUInt16LE(4, 32); buf.writeUInt16LE(16, 34); buf.write('data', 36); buf.writeUInt32LE(LEN * 4, 40);
for (let n = 0; n < LEN; n++) for (let ch = 0; ch < 2; ch++) buf.writeInt16LE(Math.round(Math.max(-1, Math.min(1, OUT[ch][n] * gain)) * 32767), 44 + n * 4 + ch * 2);
const dir = path.join(HERE, 'build');
fs.mkdirSync(dir, { recursive: true });
fs.writeFileSync(path.join(dir, 'music.wav'), buf);
spawnSync('ffmpeg', ['-y', '-loglevel', 'error', '-i', path.join(dir, 'music.wav'), '-c:a', 'aac', '-b:a', '192k', path.join(dir, 'music.m4a')], { stdio: 'inherit' });
console.log('wrote build/music.wav + music.m4a, peak', peak.toFixed(3));
