// DOM widgets: toasts, modals, context menus — self-contained, no lab state.

import { $, $$, esc, h, clamp } from './util.js';
import { iconSVG, UI } from './icons.js';

export const ico = name => `<i data-i="${name}">${UI[name] ?? ''}</i>`;
export const hydrateIcons = (root = document) => $$('i[data-i]', root).forEach(i => { if (!i.innerHTML) i.innerHTML = UI[i.dataset.i] ?? ''; });

export function toast(msg, kind = '') {
  const t = h(`<div class="toast ${kind}">${esc(msg)}</div>`);
  $('#toasts').append(t);
  setTimeout(() => t.remove(), kind === 'err' ? 5000 : 2800);
}

export function modal(title, body, foot = '', cls = '') {
  const m = h(`<div class="modal-bg"><div class="modal ${cls}" role="dialog" aria-modal="true" aria-label="${esc(title)}">
    <header>${esc(title)}<span class="spacer"></span><button class="icon-btn sm" data-x>${UI.x}</button></header>
    <div class="mbody"></div>${foot ? `<footer>${foot}</footer>` : ''}</div></div>`);
  $('.mbody', m).append(typeof body === 'string' ? h(`<div>${body}</div>`) : body);
  const close = () => { m.remove(); m._onclose?.(); };
  m.addEventListener('pointerdown', e => { if (e.target === m) close(); });
  $('[data-x]', m).onclick = close;
  m.addEventListener('keydown', e => { if (e.key === 'Escape') close(); });
  $('#modal-root').append(m);
  hydrateIcons(m);
  m.close = close;
  setTimeout(() => $('button, input', $('.mbody', m))?.focus(), 0);
  return m;
}

/** modal text input → Promise<string|null> */
export function ask(title, value = '', { multiline = false, ok = 'OK', placeholder = '' } = {}) {
  return new Promise(res => {
    const body = h(`<div>${multiline
      ? `<textarea class="field" rows="5" style="height:auto;padding:8px;font-family:var(--font);resize:vertical" placeholder="${esc(placeholder)}">${esc(value)}</textarea>`
      : `<input class="field" style="height:32px" value="${esc(value)}" placeholder="${esc(placeholder)}" spellcheck="false" />`}</div>`);
    const m = modal(title, body, `<button class="btn" data-c>Cancel</button><button class="btn primary" data-ok>${esc(ok)}</button>`);
    let out = null;
    m._onclose = () => res(out);
    const f = $('.field', body);
    const done = () => { out = f.value; m.close(); };
    $('[data-ok]', m).onclick = done;
    $('[data-c]', m).onclick = () => m.close();
    f.addEventListener('keydown', e => { if (e.key === 'Enter' && (!multiline || e.ctrlKey || e.metaKey)) { e.preventDefault(); done(); } });
    setTimeout(() => { f.focus(); f.select?.(); }, 0);
  });
}

export function confirmBox(title, text, ok = 'Continue', danger = false) {
  return new Promise(res => {
    const m = modal(title, `<p style="margin:0;color:var(--text-2)">${esc(text)}</p>`,
      `<button class="btn" data-c>Cancel</button><button class="btn ${danger ? 'danger' : 'primary'}" data-ok>${esc(ok)}</button>`);
    let out = false;
    m._onclose = () => res(out);
    $('[data-ok]', m).onclick = () => { out = true; m.close(); };
    $('[data-c]', m).onclick = () => m.close();
    setTimeout(() => $('[data-ok]', m).focus(), 0);
  });
}

const ctxEl = $('#ctx');

export function openMenu(items, x, y) {
  ctxEl.innerHTML = '';
  for (const it of items) {
    if (it === '-') { ctxEl.append(h('<hr>')); continue; }
    if (it.header) { ctxEl.append(h(`<div class="mh">${esc(it.header)}</div>`)); continue; }
    const b = h(`<button class="mi ${it.danger ? 'danger' : ''} ${it.cls ?? ''}" role="menuitem" ${it.disabled ? 'disabled' : ''}>
      ${it.icon ? ico(it.icon) : it.art ? `<span style="width:18px;height:18px;display:inline-flex">${iconSVG(it.art)}</span>` : '<i data-i></i>'}
      <span>${it.html ?? esc(it.label)}</span>${it.kbd ? `<kbd>${esc(it.kbd)}</kbd>` : ''}${it.sub ? `<span class="sub">${esc(it.sub)}</span>` : ''}</button>`);
    b.onclick = () => { closeMenu(); it.run?.(); };
    ctxEl.append(b);
  }
  ctxEl.hidden = false;
  const r = ctxEl.getBoundingClientRect();
  ctxEl.style.left = clamp(x, 4, innerWidth - r.width - 4) + 'px';
  ctxEl.style.top = clamp(y, 4, innerHeight - r.height - 4) + 'px';
  ctxEl.querySelector('.mi:not(:disabled)')?.focus();
}
export function closeMenu() {
  if (ctxEl.hidden) return;
  ctxEl.hidden = true;
  $$('#menubar > button').forEach(b => b.classList.remove('open'));
}
addEventListener('pointerdown', e => { if (!ctxEl.contains(e.target) && !e.target.closest('#menubar')) closeMenu(); }, true);
ctxEl.addEventListener('keydown', e => {
  const items = $$('.mi:not(:disabled)', ctxEl);
  const i = items.indexOf(document.activeElement);
  if (e.key === 'ArrowDown') { items[(i + 1) % items.length]?.focus(); e.preventDefault(); }
  if (e.key === 'ArrowUp') { items[(i - 1 + items.length) % items.length]?.focus(); e.preventDefault(); }
  if (e.key === 'Escape') closeMenu();
});
