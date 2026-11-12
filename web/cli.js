// Thin adapter for simulated-node consoles: transport + DOM only, it never
// parses command text. The wasm FFI carries all wording — SimShell composes
// Enter-echoes, draws help/complete carets, renders Op payloads, dispatches
// intent side effects, and owns the scrollback.
//
// Host facade: {app, meta, lab}
//   app  — app-level channels: checkpoint(), refresh(), dirty(),
//          rename(id,name), dev(id)
//   meta — Map<devId, meta record> for meta.startup / meta.startupBoot
//   lab  — the wasm NetLab, reached per call so a restore() swap stays live
// write(text, cls) — one scrollback span; cls ∈ '', dim, err, ok, warn, echo;
//   '\x0c' clears (termShell convention). The DOM is optional — attach()
//   binds it; web/_test drives the same methods headless.

const HIST_CAP = 256;   // JS recall ring — own bound, not synced to `terminal history size`
const sep = p => (p.endsWith(' ') ? 0 : 1);   // the ONE echo-separator predicate
const width = p => [...p].length;             // display_width — ASCII grammar ⇒ byte col
const TRIAD = "% Invalid input detected at '^' marker.";   // pinned constant
const GUARD = '% node was deleted';
const reducedMotion = () => globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches ?? false;

// The ONLY Err the cli_* wire documents is the missing-session family —
// anchored so nothing else maps to the guard; any other throw is a seam
// defect and dies loud.
export const isMissingSession = e => /^netlab: device \d+ was deleted$/.test(String(e?.message ?? e));

// The ONE event parse site (Boundary Schema rule 6). kind × wording-class →
// {cls?, text}; output domain is text renders only — an event never produces
// Intent/Clear/Bell/Echo or a checkpoint/refresh/dirty.
export function parse_event(kind, msg) {
  switch (kind) {
    case 'link': return { text: msg, cls: /to down/.test(msg) ? 'warn' : '' };
    case 'ping': return { text: msg, cls: /^[.!U]$/.test(msg) ? 'echo' : '' };
    case 'trace': return { text: msg, cls: /^\d/.test(msg) ? 'echo' : '' };
    case 'dhcp': return { text: msg, cls: 'dim' };
    case 'syslog': {
      const sev = msg.match(/^%[^-\s]+-(\d)-/);
      return { text: msg, cls: sev && +sev[1] <= 4 ? 'warn' : 'dim' };
    }
    default: return { text: msg, cls: '' };   // unknown kind — verbatim, never dropped
  }
}

export class SimShell {
  constructor(host, devId, write) {
    this.host = host;
    this.id = devId;
    this.write = write;
    this.history = [];        // submitted-line recall ring, adapter-owned cap
    this.hi = -1;             // recall cursor; <0 = live buffer
    this.stash = '';          // live buffer stashed on first ArrowUp
    this.pingJob = null;      // {abort()} — the JS side of an armed probe
    this.probeMuted = false;  // drop further ping|trace event renders after abort
    this.promptText = null;   // promptSpan mirror — pending prompt verbatim while armed
    this.pending = false;     // dialogMode: an armed Pending dialog
    this.state = 'engine-loading';   // live | engine-loading | orphaned
    this.dom = null;
    // Batch lane — write.batch when the host offers it (termShell), else a
    // per-span replay. Multi-line renders append once, not once per line.
    this.writeBatch = write.batch ?? ((spans = []) => { for (const s of spans) write(s.text, s.cls); });
    // Silent first resolution — the banner waits for mount()/resolveEngine().
    try { this.promptText = this.lab.cli_prompt(this.id); this.state = 'live'; }
    catch { /* session not yet minted — engine-loading until cli_prompt resolves */ }
  }

  get lab() { return this.host.lab; }   // per-call — restore() reassigns the NetLab
  get app() { return this.host.app; }
  get meta() { return this.host.meta; }
  get isHost() { return this.app.dev?.(this.id)?.kind === 'host'; }

  /* ── mount / lifecycle ── */

  // Host calls once after construction. Live → banner + first prompt;
  // engine-loading → one dim `connecting…` line, cmdInput disabled until the
  // first cli_prompt resolution (retried from onTick).
  mount() {
    if (this.state === 'live') { this.banner(); }
    else if (this.state !== 'orphaned') {
      if (!this.app.dev?.(this.id)) return this.orphan();
      this.write('connecting…\n', 'dim');
    }
    this.syncDom();
  }

  // engine-loading → live on the first cli_prompt that resolves.
  resolveEngine() {
    if (this.state !== 'engine-loading') return;
    try { this.promptText = this.lab.cli_prompt(this.id); }
    catch {
      if (!this.app.dev?.(this.id)) this.orphan();
      return;
    }
    this.state = 'live';
    this.banner();
    this.syncDom();
  }

  orphan() {
    if (this.state === 'orphaned') return;
    this.state = 'orphaned';
    this.pingJob?.abort();
    this.write(`${GUARD}\n`, 'err');
    this.syncDom();
  }

  // Missing-session Err → the guard line; no retry.
  isMissing(e) { return isMissingSession(e); }
  deleted() { this.orphan(); return null; }

  // Adapter-owned — the banner text lives here, core emits no banner op.
  banner() {
    const d = this.app.dev?.(this.id);
    if (!d) return;
    if (d.kind === 'host') {
      this.write(`netlab VPCS shell — ${d.name}\n`, 'dim');
      this.write('Type ? for commands. Try:  ip 10.0.0.5/24 10.0.0.254   then   ping 10.0.0.254\n', 'dim');
    } else {
      this.write(`${d.name} — netlab ${d.kind} CLI\n`, 'dim');
      this.write('Type ? for commands. Abbreviations work: sh ip int br, sh ip ro, sh run\n', 'dim');
    }
  }

  prompt() { return this.promptText ?? ''; }

  /* ── chords ── */

  // Enter — EchoLine first (every Enter incl. dialog answers and the
  // prompt-only empty submit; p = pre-submit promptSpan text), then the
  // unconditional pre-call checkpoint, then cli_exec.
  run(line) {
    if (this.state === 'orphaned') return null;
    const p = this.promptText ?? '';
    this.write(`${p}${' '.repeat(sep(p))}${line}\n`, 'echo');
    const raw = line.trim();
    if (raw) {
      this.history.push(raw);
      if (this.history.length > HIST_CAP) this.history.shift();
      this.hi = -1;
    }
    this.app.checkpoint();
    let r;
    try { r = this.lab.cli_exec(this.id, line); }
    catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
    // r.prompt lands BEFORE renderOps: an intent (reload) sets promptText
    // itself through cli_prompt and must win; without one the result's own
    // prompt — incl. a pending dialog's — stands.
    this.promptText = r.prompt;
    this.renderOps(r.out);
    this.pending = !!r.pending;
    if (r.mutated) { this.app.refresh(); this.app.dirty(); }
    this.syncDom();
    return r;
  }

  // `?` — intercepted pre-input; before = value.slice(0, selectionStart).
  // The result's prompt rides every variant incl. err — an armed pending
  // prompt can never regress promptSpan.
  contextHelp(before) {
    if (this.state === 'orphaned') return null;
    let r;
    try { r = this.lab.cli_help(this.id, before); }
    catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
    this.promptText = r.prompt;
    if (r.t === 'rows') {
      const w = r.rows.reduce((m, row) => Math.max(m, row.word.length), 0);
      this.writeBatch(r.rows.map(row => ({ text: `  ${row.word.padEnd(w)}  ${row.help ?? ''}\n`, cls: '' })));
    } else if (r.t === 'inline') {
      this.write(`  ${r.inline.join('  ')}\n`, '');
    } else {
      this.caret(r.prompt, r.index);
    }
    this.syncDom();
    return r;
  }

  // Tab — same slice → cli_complete. Returns the raw result; Line{line} is
  // spliced by tabComplete (the adapter's DOM half) or by the host keymap.
  complete(before) {
    if (this.state === 'orphaned') return null;
    let r;
    try { r = this.lab.cli_complete(this.id, before); }
    catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
    if (r.t === 'list') this.write(`${r.list.join('  ')}\n`, '');
    else if (r.t === 'err') this.caret(this.promptText ?? '', r.index);
    // CompleteResult carries no prompt — the post-Tab prompt is the session
    // prompt, never the result payload.
    try { this.promptText = this.lab.cli_prompt(this.id); }
    catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
    this.syncDom();
    return r;
  }

  // The DOM splice half of Tab: input = {value, selectionStart,
  // setSelectionRange}. Line{line} → input.value = line + tail, caret lands
  // at line.length — the post-cursor tail is preserved.
  tabComplete(input) {
    const tail = input.value.slice(input.selectionStart);
    const r = this.complete(input.value.slice(0, input.selectionStart));
    if (r?.t === 'line') {
      input.value = r.line + tail;
      input.setSelectionRange(r.line.length, r.line.length);
    }
    return r;
  }

  // Ctrl+C / Ctrl+Z → cli_key; Ctrl+C also aborts the JS pingJob. Core owns
  // dialog release (^C op), pending drains, and the PrivExec guard.
  key(op) {
    if (this.state === 'orphaned') return null;
    if (op === 'ctrlc') this.pingJob?.abort();
    let r;
    try { r = this.lab.cli_key(this.id, op); }
    catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
    this.promptText = r.prompt;
    this.renderOps(r.out);
    this.pending = !!r.pending;
    this.syncDom();
    return r;
  }

  // Paste → one scriptBatch: exactly one checkpoint per batch, renderOps,
  // errors[] at original lineno, mutated → refresh+dirty, then cli_prompt —
  // ScriptResult carries no prompt.
  paste(text) {
    if (this.state === 'orphaned') return null;
    this.app.checkpoint();
    let r;
    try { r = this.lab.cli_script(this.id, text); }
    catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
    this.renderOps(r.out);
    if (r.errors?.length) this.writeBatch(r.errors.map(e => ({ text: `line ${e.lineno}: ${e.msg}\n`, cls: 'err' })));
    if (r.mutated) { this.app.refresh(); this.app.dirty(); }
    try { this.promptText = this.lab.cli_prompt(this.id); }
    catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
    this.pending = false;
    this.syncDom();
    return r;
  }

  /* ── render + intent dispatch ── */

  // The only Op render table — core decides content, JS never parses it.
  // Line ops collect into one batch — a single append + scroll adjustment
  // per render; a clear/bell/intent flushes first so order stays exact.
  renderOps(out = []) {
    let batch = [];
    const flush = () => { if (batch.length) { this.writeBatch(batch); batch = []; } };
    for (const op of out) {
      if (op.t === 'text') {
        for (const seg of String(op.text).split('\n')) batch.push({ text: `${seg}\n`, cls: op.cls ?? '' });
      } else if (op.t === 'clear') {
        flush(); this.write('\x0c');
      } else if (op.t === 'bell') {
        flush(); if (!reducedMotion()) this.dom?.flash?.();   // non-content cue — never a line
      } else if (op.t === 'echo') {
        for (const seg of String(op.text).split('\n')) batch.push({ text: `${seg}\n`, cls: 'echo' });
      } else if (op.t === 'intent') {
        flush(); this.dispatchIntent(op);   // never renders a line
      }
    }
    flush();
  }

  // The side-effect layer — core stays pure, the adapter performs effects.
  dispatchIntent(op) {
    if (op.kind === 'ping' || op.kind === 'trace') {
      this.pingJob?.abort();
      this.probeMuted = false;
      const armed = op.kind === 'ping'
        ? this.lab.ping(this.id, op.target)
        : this.lab.traceroute(this.id, op.target);
      if (!armed) { this.write('could not send probe\n', 'err'); return; }
      this.pingJob = { kind: op.kind, target: op.target, abort: () => { this.probeMuted = true; this.pingJob = null; } };
      return;
    }
    if (op.kind === 'rename') { this.app.rename(this.id, op.name); return; }
    const m = this.meta?.get(this.id);
    if (op.kind === 'nvram-write') {
      if (m) { m.startup = op.text; m.startupBoot = true; }
      try { this.lab.cli_set_startup(this.id, op.text); }
      catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
      this.app.dirty();
      return;
    }
    if (op.kind === 'nvram-erase') {
      if (m) { m.startup = undefined; m.startupBoot = false; }
      try { this.lab.cli_set_startup(this.id, null); }
      catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
      this.app.dirty();
      return;
    }
    if (op.kind === 'reload') {
      // replay push — the unconditional-checkpoint rule; dedup absorbs it
      this.app.checkpoint();
      // reset_config restores the authored powered flag — capture the live
      // state first (mirrors reloadNode): a running device comes back up.
      const wasPowered = this.app.dev?.(this.id)?.powered === true;
      try { this.lab.reset_config(this.id); }
      catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
      this.lab.set_power(this.id, wasPowered);
      try { this.lab.cli_set_startup(this.id, m?.startup ?? null); }
      catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
      // startup_gate ≡ startupBoot == true ∧ startup is a string — both conjuncts
      if (m?.startupBoot === true && typeof m.startup === 'string') {
        let r;
        try { r = this.lab.cli_script(this.id, m.startup); }
        catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
        this.renderOps(r.out);
        if (r.errors?.length) this.writeBatch(r.errors.map(e => ({ text: `line ${e.lineno}: ${e.msg}\n`, cls: 'err' })));
        if (r.mutated) { this.app.dirty(); }
      }
      this.app.refresh();
      try { this.promptText = this.lab.cli_prompt(this.id); }
      catch (e) { if (!this.isMissing(e)) throw e; return this.deleted(); }
      this.pending = false;
      this.syncDom();
    }
  }

  // The adapter-drawn caret — the help/complete channel carries the bare
  // index; pad(display_width(p) + sep(p) + index) + '^' + the frozen triad.
  caret(p, index) {
    this.write(`${' '.repeat(width(p) + sep(p) + index)}^\n`, '');
    this.write(`${TRIAD}\n`, 'err');
  }

  /* ── event channel ── */

  // ONE parse site through parse_event. Device mismatch or a shell on a
  // deleted device → drop before parse; post-abort probe renders drop too.
  // onEvents is the pump's batch lane — a burst drains into one writeBatch,
  // one layout read.
  onEvents(evs) {
    if (this.state === 'orphaned') return;
    const spans = [];
    for (const ev of evs) {
      if (ev.device !== this.id) continue;
      if (this.probeMuted && (ev.kind === 'ping' || ev.kind === 'trace')) continue;
      const { text, cls } = parse_event(ev.kind, ev.msg);
      for (const seg of String(text).split('\n')) spans.push({ text: `${seg}\n`, cls });
    }
    if (spans.length) this.writeBatch(spans);
  }

  onEvent(ev) { this.onEvents([ev]); }

  // Tick pump — retries the first cli_prompt while engine-loading; the JS
  // guard aborts a probe whose device is gone mid-job.
  onTick() {
    if (this.state === 'engine-loading') this.resolveEngine();
    else if (this.pingJob && !this.app.dev?.(this.id)) this.pingJob.abort();
  }

  /* ── DOM binding (optional — headless never attaches) ── */

  // dom = {shell, input, pr, flash?} — the termShell pieces the host built.
  attach(dom) {
    this.dom = dom;
    dom.input?.addEventListener('keydown', e => this.onKey(e));
    dom.input?.addEventListener('paste', e => this.onPaste(e));
    dom.shell?.addEventListener('mouseup', e => {
      if (!e.target.closest('button') && !(globalThis.getSelection?.()?.toString())) dom.input?.focus();
    });
    this.syncDom();
  }

  // The whole keymap on cmdInput — Enter, `?`, Tab, the Ctrl chords,
  // ArrowUp/Down recall. Inert by design: Esc-B/F, Ctrl-R, Ctrl-Shift-6,
  // the 10-char wrap — they do nothing; they are not grammar.
  onKey(e) {
    const input = e.currentTarget ?? e.target;
    if (this.state !== 'live') { e.preventDefault(); return; }
    if (e.key === 'Enter') {
      this.run(input.value);
      input.value = '';
    } else if (e.key === '?') {
      e.preventDefault();
      this.contextHelp(input.value.slice(0, input.selectionStart));
    } else if (e.key === 'Tab') {
      e.preventDefault();
      this.tabComplete(input);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      this.recall(input, -1);
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      this.recall(input, 1);
    } else if (e.ctrlKey) {
      const k = e.key.toLowerCase();
      if (k === 'c') { e.preventDefault(); this.key('ctrlc'); }
      else if (k === 'z') { e.preventDefault(); this.key('ctrlz'); }
      else if (k === 'l') { e.preventDefault(); this.clearScreen(); }
      else if ('aebufw'.includes(k)) { e.preventDefault(); this.edit(input, k); }
    }
  }

  onPaste(e) {
    const text = e.clipboardData?.getData('text');
    if (text == null) return;
    e.preventDefault();
    this.paste(text);
  }

  // The shipped line-edit subset: line start, line end, back-char,
  // forward-char, kill-line, kill-word.
  edit(input, op) {
    const v = input.value, s = input.selectionStart ?? v.length;
    if (op === 'a') input.setSelectionRange(0, 0);
    else if (op === 'e') input.setSelectionRange(v.length, v.length);
    else if (op === 'b') input.setSelectionRange(Math.max(0, s - 1), Math.max(0, s - 1));
    else if (op === 'f') input.setSelectionRange(Math.min(v.length, s + 1), Math.min(v.length, s + 1));
    else if (op === 'u') { input.value = ''; input.setSelectionRange(0, 0); }
    else if (op === 'w') {
      // unix-word-rubout: kill the word left of the cursor plus the whitespace
      // run ahead of it; a cursor on whitespace kills nothing. A manual scan —
      // /\s*\S+$/ backtracks quadratically on a long single-token line.
      let i = s;
      if (i > 0 && !/\s/.test(v[i - 1])) {
        while (i > 0 && !/\s/.test(v[i - 1])) i--;
        while (i > 0 && /\s/.test(v[i - 1])) i--;
      }
      input.value = v.slice(0, i) + v.slice(s);
      input.setSelectionRange(i, i);
    }
  }

  // Ctrl+L — local scrollback clear + prompt redraw; presentation only.
  clearScreen() { this.write('\x0c'); this.syncDom(); }

  // ArrowUp/Down recall ring — first ArrowUp stashes the live buffer,
  // ArrowDown past the newest entry restores it.
  recall(input, dir) {
    const h = this.history;
    if (dir < 0) {
      if (!h.length) return;
      if (this.hi < 0) { this.stash = input.value; this.hi = h.length; }
      this.hi = Math.max(0, this.hi - 1);
      input.value = h[this.hi];
    } else {
      if (this.hi < 0) return;
      if (++this.hi >= h.length) { this.hi = -1; input.value = this.stash; }
      else input.value = h[this.hi];
    }
  }

  // DOM projection — termShell state class, cmdInput disabled seat,
  // promptSpan rewritten wholesale. No-op when unattached.
  syncDom() {
    const d = this.dom;
    if (!d) return;
    if (d.shell) d.shell.dataset.state = this.state;
    if (d.input) d.input.disabled = this.state !== 'live';
    if (d.pr) d.pr.textContent = this.promptText ?? '';
  }
}
