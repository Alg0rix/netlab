// Floating console windows (PNETLab-style). One window per key. The content element
// is moved into the window, not cloned, so the console keeps its state while it moves.
import { $, h, clamp } from './util.js';
import { ico, hydrateIcons } from './ui.js';

const DEFAULT_W = 760, DEFAULT_H = 480, MIN_W = 320, MIN_H = 200, CASCADE = 28;
const wins = new Map();   // key -> { el, close(notify) }
let zTop = 60, cascade = 0;

const raise = rec => { rec.el.style.zIndex = ++zTop; };

export const hasFloat = key => wins.has(key);

/** Open (or raise) the window for `key`. `content` is appended once; `onClose` runs on the user's ×. */
export function openFloat(key, { title, sub = '', content, onClose, onFocus }) {
  const existing = wins.get(key);
  if (existing) { raise(existing); return; }

  const el = h(`<div class="cwin" role="dialog">
    <div class="cwin-head">
      <span class="cwin-title"></span><span class="spacer"></span>
      <button class="icon-btn sm" data-max title="Maximize">${ico('fit')}</button>
      <button class="icon-btn sm" data-close title="Close console">${ico('x')}</button>
    </div>
    <div class="cwin-body"></div>
    <div class="cwin-grip" title="Resize"></div></div>`);
  $('.cwin-title', el).textContent = sub ? `${title} · ${sub}` : title;
  $('.cwin-body', el).append(content);

  const n = cascade++ % 8;
  const w = Math.min(DEFAULT_W, innerWidth - 40), hgt = Math.min(DEFAULT_H, innerHeight - 120);
  const rec = { el, prev: null, close: notify => {
    wins.delete(key);
    el.remove();
    if (notify) onClose?.();
  } };
  Object.assign(el.style, {
    width: w + 'px', height: hgt + 'px',
    left: clamp(90 + n * CASCADE, 8, innerWidth - w - 8) + 'px',
    top: clamp(70 + n * CASCADE, 8, innerHeight - hgt - 8) + 'px',
  });
  wins.set(key, rec);
  document.body.append(el);
  raise(rec);
  hydrateIcons(el);

  el.addEventListener('pointerdown', () => { raise(rec); onFocus?.(); });
  $('[data-close]', el).onclick = () => rec.close(true);

  const head = $('.cwin-head', el);
  const maxBtn = $('[data-max]', el);
  const toggleMax = () => {
    if (rec.prev) {
      Object.assign(el.style, rec.prev);
      rec.prev = null;
    } else {
      rec.prev = { left: el.style.left, top: el.style.top, width: el.style.width, height: el.style.height };
      Object.assign(el.style, { left: '8px', top: '8px', width: innerWidth - 16 + 'px', height: innerHeight - 16 + 'px' });
    }
    maxBtn.title = rec.prev ? 'Restore' : 'Maximize';
  };
  maxBtn.onclick = toggleMax;
  head.addEventListener('dblclick', e => { if (!e.target.closest('button')) toggleMax(); });

  // drag by the title bar
  head.addEventListener('pointerdown', e => {
    if (e.button !== 0 || e.target.closest('button') || rec.prev) return;
    const dx = e.clientX - el.offsetLeft, dy = e.clientY - el.offsetTop;
    head.setPointerCapture(e.pointerId);
    head.onpointermove = ev => {
      el.style.left = clamp(ev.clientX - dx, 8 - el.offsetWidth + 80, innerWidth - 80) + 'px';
      el.style.top = clamp(ev.clientY - dy, 0, innerHeight - 40) + 'px';
    };
    head.onpointerup = head.onpointercancel = () => { head.onpointermove = null; head.onpointerup = null; };
  });

  // resize from the bottom-right corner
  const grip = $('.cwin-grip', el);
  grip.addEventListener('pointerdown', e => {
    if (e.button !== 0 || rec.prev) return;
    e.preventDefault();
    const sx = e.clientX, sy = e.clientY, sw = el.offsetWidth, sh = el.offsetHeight;
    grip.setPointerCapture(e.pointerId);
    grip.onpointermove = ev => {
      el.style.width = clamp(sw + ev.clientX - sx, MIN_W, innerWidth - el.offsetLeft - 4) + 'px';
      el.style.height = clamp(sh + ev.clientY - sy, MIN_H, innerHeight - el.offsetTop - 4) + 'px';
    };
    grip.onpointerup = grip.onpointercancel = () => { grip.onpointermove = null; grip.onpointerup = null; };
  });
}

/** Close a window from code (no onClose callback). */
export function closeFloat(key) {
  wins.get(key)?.close(false);
}
