// One floating console window with a tab per open console (PNETLab-style).
// Each tab owns an element that is moved into the window's body while it is active,
// not cloned, so a console keeps its state (scrollback, VGA frame, shell) when hidden.
import { $, h, clamp, esc } from './util.js';
import { ico, hydrateIcons } from './ui.js';

const DEFAULT_W = 820, DEFAULT_H = 500, MIN_W = 360, MIN_H = 220;
const tabs = new Map();   // key -> { label, art, state(), el, focus(), onClose() }
let active = null;
let win = null;           // { el, prev }
let lastRect = null;      // remembered across close/reopen
let zTop = 60;

export const hasTab = key => tabs.has(key);

/** Add a console tab (or do nothing if it exists) and make it the visible one. */
export function addTab(key, tab) {
  if (tabs.has(key)) return activateTab(key);
  tabs.set(key, tab);
  ensureWin();
  activateTab(key);
}

export function activateTab(key) {
  const t = tabs.get(key);
  if (!t) return;
  active = key;
  if (win) {
    $('.cwin-body', win.el).replaceChildren(t.el);
    renderTabs();
  }
  t.focus?.();
}

/** Drop a tab without calling its onClose (the caller already closed the console). */
export function removeTab(key) {
  if (!tabs.delete(key)) return;
  if (active === key) {
    active = null;
    if (win) $('.cwin-body', win.el).replaceChildren();
  }
  if (!tabs.size) return destroyWin();
  if (!active) activateTab(tabs.keys().next().value);
  renderTabs();
}

/** Re-draw the tab strip (status dots). Cheap; call on any node state change. */
export function refreshTabs() { renderTabs(); }

/** Window dock button: hand every tab to the bottom panel (each tab's onDock) and close the window. */
export function dockAllTabs() {
  for (const key of [...tabs.keys()]) {
    const t = tabs.get(key);
    removeTab(key);
    t?.onDock?.();
  }
}

/** Window × : asks every tab to close, the same as clicking each tab's ×. */
export function closeAllTabs() {
  for (const t of [...tabs.values()]) t.onClose?.();
  for (const key of [...tabs.keys()]) removeTab(key);
}

function renderTabs() {
  if (!win) return;
  const bar = $('.cwin-tabs', win.el);
  bar.innerHTML = '';
  for (const [key, t] of tabs) {
    const st = t.state?.();
    const b = h(`<button class="ctab ${key === active ? 'on' : ''}" role="tab" title="${esc(t.label)}">
      ${t.art ?? ''}${st ? `<span class="dot ${st}"></span>` : ''}<span class="ctab-name">${esc(t.label)}</span>
      <span class="x" title="Close console">${ico('x')}</span></button>`);
    b.onclick = e => {
      if (e.target.closest('.x')) return t.onClose?.();
      activateTab(key);
    };
    bar.append(b);
  }
  hydrateIcons(bar);
}

function ensureWin() {
  if (win) return win;
  const el = h(`<div class="cwin" role="dialog">
    <div class="cwin-head">
      <div class="cwin-tabs" role="tablist"></div><span class="spacer"></span>
      <button class="icon-btn sm" data-dock title="Dock all to bottom panel">${ico('dock')}</button>
      <button class="icon-btn sm" data-min title="Minimize">${ico('minus')}</button>
      <button class="icon-btn sm" data-max title="Maximize">${ico('fit')}</button>
      <button class="icon-btn sm" data-close title="Close all consoles">${ico('x')}</button>
    </div>
    <div class="cwin-body"></div>
    <div class="cwin-grip" title="Resize"></div></div>`);
  const r = lastRect ?? {};
  const w = Math.min(DEFAULT_W, innerWidth - 40), hgt = Math.min(DEFAULT_H, innerHeight - 120);
  Object.assign(el.style, {
    width: (r.width ?? w) + 'px', height: (r.height ?? hgt) + 'px',
    left: clamp(r.left ?? 90, 8, innerWidth - 80) + 'px',
    top: clamp(r.top ?? 70, 8, innerHeight - 60) + 'px',
  });
  document.body.append(el);
  win = { el, prev: null };
  el.addEventListener('pointerdown', () => { el.style.zIndex = ++zTop; });
  // any click in the body puts the keyboard in the active console (terminal or VGA screen)
  $('.cwin-body', el).addEventListener('pointerdown', () => tabs.get(active)?.focus?.());
  el.style.zIndex = ++zTop;
  hydrateIcons(el);

  const head = $('.cwin-head', el);
  const minBtn = $('[data-min]', el), maxBtn = $('[data-max]', el);
  $('[data-dock]', el).onclick = dockAllTabs;
  $('[data-close]', el).onclick = closeAllTabs;

  const toggleMin = () => {
    const on = el.classList.toggle('cwin-min');
    minBtn.title = on ? 'Restore' : 'Minimize';
  };
  minBtn.onclick = toggleMin;

  const toggleMax = () => {
    if (win.prev) {
      Object.assign(el.style, win.prev);
      win.prev = null;
    } else {
      win.prev = { left: el.style.left, top: el.style.top, width: el.style.width, height: el.style.height };
      Object.assign(el.style, { left: '8px', top: '8px', width: innerWidth - 16 + 'px', height: innerHeight - 16 + 'px' });
    }
    maxBtn.title = win.prev ? 'Restore' : 'Maximize';
  };
  maxBtn.onclick = toggleMax;
  head.addEventListener('dblclick', e => { if (!e.target.closest('button')) toggleMax(); });

  // drag by the header (tabs and buttons excluded)
  head.addEventListener('pointerdown', e => {
    if (e.button !== 0 || e.target.closest('button') || win.prev) return;
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
    if (e.button !== 0 || win.prev || el.classList.contains('cwin-min')) return;
    e.preventDefault();
    const sx = e.clientX, sy = e.clientY, sw = el.offsetWidth, sh = el.offsetHeight;
    grip.setPointerCapture(e.pointerId);
    grip.onpointermove = ev => {
      el.style.width = clamp(sw + ev.clientX - sx, MIN_W, innerWidth - el.offsetLeft - 4) + 'px';
      el.style.height = clamp(sh + ev.clientY - sy, MIN_H, innerHeight - el.offsetTop - 4) + 'px';
    };
    grip.onpointerup = grip.onpointercancel = () => { grip.onpointermove = null; grip.onpointerup = null; };
  });
  return win;
}

function destroyWin() {
  if (!win) return;
  const el = win.el;
  if (!win.prev) lastRect = { left: parseFloat(el.style.left), top: parseFloat(el.style.top), width: el.offsetWidth, height: el.offsetHeight };
  el.remove();
  win = null;
  active = null;
}
