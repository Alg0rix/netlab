import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { NetLab, initSync } from '../pkg/netlab_wasm.js';
import { SimShell, parse_event } from '../cli.js';
import {
  sep,
  opFrames,
  promptFrame,
  helpFrames,
  completeFrames,
  eventFrames,
  maskSet,
  checkMasksUsed,
  normalizeFrames,
  checkGap,
} from './frames.mjs';
import { RECORD_RIGS, buildRig } from './rigs.mjs';

initSync({ module: readFileSync(new URL('../pkg/netlab_wasm_bg.wasm', import.meta.url)) });

const records = (() => {
  const url = new URL('./corpus-steps.json', import.meta.url);
  try {
    const data = JSON.parse(readFileSync(url, 'utf8'));
    if (!Array.isArray(data) || data.length === 0) throw new Error('empty record list');
    return data;
  } catch (e) {
    throw new Error(`web/_test/corpus-steps.json: ${e.message}`);
  }
})();

const TRIAD_LINE = "% Invalid input detected at '^' marker.";
const width = p => [...p].length;

const fakeInput = () => ({
  value: '',
  selectionStart: 0,
  selectionEnd: 0,
  disabled: false,
  listeners: {},
  addEventListener(t, f) { this.listeners[t] = f; },
  setSelectionRange(a, b) { this.selectionStart = a; this.selectionEnd = b; },
  focus() {},
});

const fakeDom = (input, onFlash) => ({
  shell: { dataset: {}, addEventListener() {} },
  input,
  pr: { textContent: '' },
  flash: onFlash,
});

// The mock facade — {app, meta, lab} per web/cli.js's host contract.
// app.* channels only record; lab binds the real wasm exports through a
// call-logging proxy so ordering is observable.
function makeFacade(lab, devices, meta) {
  const calls = [];
  const writes = [];
  const app = {
    dev: i => devices.get(i),
    checkpoint: () => calls.push({ f: 'app.checkpoint' }),
    refresh: () => calls.push({ f: 'app.refresh' }),
    dirty: () => calls.push({ f: 'app.dirty' }),
    realtime: () => false,
    rename: (i, n) => {
      calls.push({ f: 'app.rename', args: [i, n] });
      devices.get(i).name = n;
      lab.rename_device(i, n);
    },
  };
  const labSpy = new Proxy(lab, {
    get(t, k) {
      const v = t[k];
      if (typeof v !== 'function') return v;
      return (...args) => {
        const entry = { f: `lab.${String(k)}`, args };
        calls.push(entry);
        try {
          entry.r = v.apply(t, args);
          return entry.r;
        } catch (e) {
          entry.threw = e;
          throw e;
        }
      };
    },
  });
  const host = { app, meta, get lab() { return labSpy; } };
  return { host, calls, writes };
}

const MUT_CALLS = new Set(['lab.cli_exec', 'lab.cli_script']);

// checkpoint-before-mutation: every cli_exec/cli_script in a step window is
// preceded by an app.checkpoint; the step's first mutation call follows
// exactly one push. key/tick/drain/allow-events cross no mutation call and
// must observe zero checkpoints.
function checkOrdering(id, stepType, log) {
  const fail = msg => { throw new Error(`${id}: ${msg}`); };
  const mutIdx = log.map((c, i) => (MUT_CALLS.has(c.f) ? i : -1)).filter(i => i >= 0);
  const cps = log.filter(c => c.f === 'app.checkpoint').length;
  if (stepType === 'exec' || stepType === 'gap' || stepType === 'script') {
    const want = stepType === 'script' ? 'lab.cli_script' : 'lab.cli_exec';
    if (mutIdx.length === 0 || log[mutIdx[0]].f !== want) fail(`${stepType} step did not reach ${want}`);
    if (stepType !== 'script' && log.filter(c => c.f === want).length !== 1) {
      fail(`${stepType} step called ${want} ${log.filter(c => c.f === want).length} times, want exactly 1`);
    }
    const before = log.slice(0, mutIdx[0]).filter(c => c.f === 'app.checkpoint').length;
    if (before !== 1) fail(`checkpoint ordering violated — ${before} pushes before ${want}, want exactly 1`);
    for (const [n, i] of mutIdx.entries()) {
      const lo = n > 0 ? mutIdx[n - 1] : -1;
      if (!log.slice(lo + 1, i).some(c => c.f === 'app.checkpoint')) {
        fail(`checkpoint ordering violated — no push between ${log[lo]?.f ?? 'step start'} and ${log[i].f}`);
      }
    }
  } else {
    if (mutIdx.length > 0) fail(`${stepType} step crossed ${log[mutIdx[0]].f} — read-only steps never mutate`);
    if (cps > 0) fail(`${stepType} step pushed ${cps} checkpoint(s) — carries no checkpoint expectation`);
    if (log.some(c => c.f === 'app.refresh' || c.f === 'app.dirty')) {
      fail(`${stepType} step touched refresh/dirty — read-only steps leave the document alone`);
    }
  }
}

// mutated ↔ refresh+dirty: unmutated arms with no dispatched intents must
// observe neither call; intents account for their own channels (reload →
// checkpoint+refresh+replay, nvram-write/erase → cli_set_startup+dirty,
// rename → app.rename, ping/trace → lab.ping/lab.traceroute).
function checkEffects(id, stepType, log, r, intents) {
  const fail = msg => { throw new Error(`${id}: ${msg}`); };
  const count = f => log.filter(c => c.f === f).length;
  const reloads = intents.filter(i => i.kind === 'reload').length;
  const nvrams = intents.filter(i => i.kind === 'nvram-write' || i.kind === 'nvram-erase').length;
  // a mutating cli_script inside an exec window is the reload replay; inside
  // a script window the first call is the batch itself — never counted twice
  const scriptCalls = log.filter(c => c.f === 'lab.cli_script' && c.r?.mutated).length;
  const replayDirty = scriptCalls - (stepType === 'script' && r.mutated ? 1 : 0);
  const wantRefresh = (r.mutated ? 1 : 0) + reloads;
  const wantDirty = (r.mutated ? 1 : 0) + nvrams + replayDirty;
  if (count('app.refresh') !== wantRefresh) {
    fail(`refresh count ${count('app.refresh')} — want ${wantRefresh} (mutated=${r.mutated}, reloads=${reloads})`);
  }
  if (count('app.dirty') !== wantDirty) {
    fail(`dirty count ${count('app.dirty')} — want ${wantDirty} (mutated=${r.mutated}, nvrams=${nvrams}, replayMutated=${replayDirty})`);
  }
  for (const i of intents) {
    if (i.kind === 'rename') {
      const hit = log.find(c => c.f === 'app.rename');
      if (!hit) fail('rename intent produced no app.rename call');
      else if (hit.args[1] !== i.name) fail(`app.rename called with '${hit.args[1]}', want '${i.name}'`);
    } else if (i.kind === 'nvram-write') {
      if (!log.some(c => c.f === 'lab.cli_set_startup' && c.args[1] === i.text)) {
        fail('nvram-write intent did not mirror cli_set_startup(text)');
      }
    } else if (i.kind === 'nvram-erase') {
      if (!log.some(c => c.f === 'lab.cli_set_startup' && c.args[1] == null)) {
        fail('nvram-erase intent did not mirror cli_set_startup(null)');
      }
    } else if (i.kind === 'reload') {
      if (!log.some(c => c.f === 'lab.reset_config')) fail('reload intent did not call reset_config');
      if (!log.some(c => c.f === 'lab.set_power')) fail('reload intent did not restore power state');
    } else if (i.kind === 'ping' || i.kind === 'trace') {
      const f = i.kind === 'ping' ? 'lab.ping' : 'lab.traceroute';
      if (!log.some(c => c.f === f)) fail(`${i.kind} intent armed no ${f} call`);
    }
  }
}

const intentsOf = out => (out ?? []).filter(o => o.t === 'intent');

// Scrollback contract — the same Ops the frames encode must reach the
// write channel as spans, in order, with the adapter's own shaping.
function* opSpans(out, ctx) {
  for (const op of out) {
    if (op.t === 'text' || op.t === 'echo') {
      const cls = op.t === 'echo' ? 'echo' : (op.cls ?? '');
      for (const seg of String(op.text).split('\n')) yield { text: `${seg}\n`, cls };
    } else if (op.t === 'clear') {
      yield { text: '\x0c', cls: undefined };
    } else if (op.t === 'bell') {
      ctx.bells += 1;
    } else if (op.t === 'intent') {
      if (op.kind === 'ping' || op.kind === 'trace') {
        const armed = ctx.probeCalls.shift();
        if (armed === false) yield { text: 'could not send probe\n', cls: 'err' };
      } else if (op.kind === 'reload' && ctx.replay) {
        yield* opSpans(ctx.replay.out ?? [], ctx);
      }
    }
  }
}

const caretSpans = function* (p, index) {
  yield { text: `${' '.repeat(width(p) + sep(p) + index)}^\n`, cls: '' };
  yield { text: `${TRIAD_LINE}\n`, cls: 'err' };
};

function* stepSpans(st, ctx) {
  const r = ctx.result;
  if (st.t === 'exec' || st.t === 'gap') {
    yield { text: `${ctx.p}${' '.repeat(sep(ctx.p))}${st.line}\n`, cls: 'echo' };
    yield* opSpans(r.out, ctx);
  } else if (st.t === 'script') {
    yield* opSpans(r.out, ctx);
    for (const e of r.errors ?? []) yield { text: `line ${e.lineno}: ${e.msg}\n`, cls: 'err' };
  } else if (st.t === 'key' && st.op === '?') {
    if (r.t === 'rows') {
      const w = r.rows.reduce((m, row) => Math.max(m, row.word.length), 0);
      for (const row of r.rows) yield { text: `  ${row.word.padEnd(w)}  ${row.help ?? ''}\n`, cls: '' };
    } else if (r.t === 'inline') {
      yield { text: `  ${r.inline.join('  ')}\n`, cls: '' };
    } else {
      yield* caretSpans(r.prompt, r.index);
    }
  } else if (st.t === 'key' && st.op === 'tab') {
    if (r.t === 'list') yield { text: `${r.list.join('  ')}\n`, cls: '' };
    else if (r.t === 'err') yield* caretSpans(ctx.p, r.index);
  } else if (st.t === 'key') {
    yield* opSpans(r.out, ctx);
  } else if (st.t === 'tick' || st.t === 'drain') {
    for (const ev of ctx.evs) {
      if (ev.device !== ctx.dev) continue;
      if (ctx.probeMuted && (ev.kind === 'ping' || ev.kind === 'trace')) continue;
      const { text, cls } = parse_event(ev.kind, ev.msg);
      for (const seg of String(text).split('\n')) yield { text: `${seg}\n`, cls };
    }
  }
}

function assertSpans(id, st, writes, expected) {
  if (writes.length !== expected.length || writes.some((w, i) => w.text !== expected[i].text || w.cls !== expected[i].cls)) {
    const diff = Math.max(writes.length, expected.length);
    const rows = [];
    for (let i = 0; i < diff; i++) {
      const g = writes[i], e = expected[i];
      if (!e || !g || g.text !== e.text || g.cls !== e.cls) {
        rows.push(`  span ${i}: got ${JSON.stringify(g)} want ${JSON.stringify(e)}`);
      }
      if (rows.length > 4) break;
    }
    throw new Error(`${id}: ${st.t} step scrollback mismatch\n${rows.join('\n')}`);
  }
}

function firstMismatch(got, want) {
  const n = Math.max(got.length, want.length);
  for (let i = 0; i < n; i++) {
    if (got[i] !== want[i]) return { i, got: got[i], want: want[i] };
  }
  return null;
}

const mismatchError = (id, m, got, want) => {
  const ctx = [];
  const n = Math.max(got.length, want.length);
  for (let j = Math.max(0, m.i - 2); j < Math.min(n, m.i + 3); j++) {
    ctx.push(`  ${j}: got  ${JSON.stringify(got[j])}\n  ${j}: want ${JSON.stringify(want[j])}`);
  }
  return new Error(`${id}: first frame mismatch at index ${m.i} (${got.length} vs ${want.length})\n${ctx.join('\n')}`);
};

const RECORD_IDS = [
  'c01-modes-ladder', 'c02-triad-invalid', 'c03-triad-incomplete', 'c04-triad-ambiguous',
  'c05-abbrev-ifname', 'c06-pending-dialogs', 'c07-pending-field', 'c08-no-forms',
  'c09-kind-gates', 'c10-show-bank-router', 'c11-show-bank-switch', 'c12-script-flat',
  'c13-script-prelude-dedup', 'c14-script-mode-restore', 'c15-startup-mirror',
  'c16-help-complete', 'c17-events-pinned', 'c18-nvgen-seeds', 'c19-gap-ledger',
  'c20-history-clear',
];

function assertFrames(id, got, want) {
  const m = firstMismatch(got, want);
  if (m) throw mismatchError(id, m, got, want);
}

const labCall = (log, f) => log.filter(c => c.f === f);
const lastCall = (log, f) => labCall(log, f).at(-1);

function driveRecord(rec) {
  const lab = new NetLab();
  try { return driveRecordOn(rec, lab); } finally { lab.free(); }
}

function driveRecordOn(rec, lab) {
  const id = rec.id;
  const fail = msg => { throw new Error(`${id}: ${msg}`); };
  if (!Array.isArray(rec.steps) || rec.steps.length === 0) fail('record carries zero steps');
  if (!Array.isArray(rec.frames)) fail('record carries no frames array');
  const rig = RECORD_RIGS[id];
  if (!rig) fail(`no rig entry for record`);
  const { dev, devs } = buildRig(lab, rig);
  const devices = new Map(devs.map(d => [d.id, { name: d.name, kind: d.kind, powered: true }]));
  const meta = new Map();
  meta.set(dev, rig.startup != null ? { startup: rig.startup, startupBoot: true } : {});
  const { host, calls, writes } = makeFacade(lab, devices, meta);
  const fx = { flashes: 0 };
  const shell = new SimShell(host, dev, (text, cls) => writes.push({ text, cls }));
  const input = fakeInput();
  const dom = fakeDom(input, () => { fx.flashes += 1; });
  shell.attach(dom);
  shell.mount();
  if (input.disabled) fail('attach left cmdInput disabled on a live shell');
  if (dom.pr.textContent !== shell.promptText) fail('promptSpan does not mirror promptText after mount');

  const frames = [];
  let armed = false;
  let probeJob = false;
  let probeMuted = false;
  let allowEvents = false;
  let bellsSeen = 0;
  const fire = ev => input.listeners.keydown?.({ preventDefault() {}, currentTarget: input, target: input, ...ev });
  const drainNow = () => {
    const evs = JSON.parse(host.lab.drain_events());
    shell.onEvents(evs);
    return evs;
  };

  for (const [si, st] of rec.steps.entries()) {
    const where = `${id} step ${si}`;
    const cm = calls.length;
    const wm = writes.length;
    const ctx = { dev, bells: 0, probeCalls: [], replay: null, probeMuted, evs: null };
    if (st.t === 'exec' || st.t === 'gap') {
      if (st.t === 'gap') frames.push({ text: `## gap: ${st.fr}${st.tok != null ? ` tok${st.tok}` : ''}`, scope: 'bare' });
      const p = shell.promptText ?? '';
      frames.push({ text: `${p}${' '.repeat(sep(p))}${st.line}`, scope: 'bare' });
      ctx.p = p;
      const r = shell.run(st.line);
      if (r == null) fail(`${where} '${st.line}' returned null on a live shell`);
      ctx.result = r;
      const log = calls.slice(cm);
      ctx.probeCalls = labCall(log, 'lab.ping').concat(labCall(log, 'lab.traceroute')).map(c => c.r);
      if (ctx.probeCalls.length) { probeJob = ctx.probeCalls.at(-1) === true; probeMuted = false; }
      const replayCalls = labCall(log, 'lab.cli_script');
      if (replayCalls.length) ctx.replay = replayCalls.at(-1).r;
      checkOrdering(id, 'exec', log);
      checkEffects(id, st.t, log, r, intentsOf(r.out));
      if (st.t === 'gap') checkGap(st, st.line, p, r.out, r.mutated, id);
      frames.push(...opFrames(r.out));
      armed = !!r.pending;
      if (shell.pending !== armed) fail(`${where} adapter pending flag diverged`);
      const wantPrompt = lastCall(log, 'lab.cli_prompt')?.r ?? r.prompt;
      if (shell.promptText !== wantPrompt) fail(`${where} promptText '${shell.promptText}' diverged — want '${wantPrompt}'`);
    } else if (st.t === 'script') {
      frames.push({ text: '## script', scope: 'bare' });
      const r = shell.paste(st.text);
      ctx.result = r;
      const log = calls.slice(cm);
      const pcs = labCall(log, 'lab.ping').concat(labCall(log, 'lab.traceroute')).map(c => c.r);
      if (pcs.length) { probeJob = pcs.at(-1) === true; probeMuted = false; }
      checkOrdering(id, 'script', log);
      checkEffects(id, st.t, log, r, intentsOf(r.out));
      frames.push(...opFrames(r.out));
      for (const e of r.errors ?? []) frames.push({ text: `!error ${e.lineno} ${e.msg}`, scope: 'engine' });
      frames.push({ text: '## endscript', scope: 'bare' });
      armed = false;
      if (shell.pending) fail(`${where} script left a pending dialog armed`);
      const rePrompt = lastCall(log, 'lab.cli_prompt');
      if (!rePrompt) fail(`${where} script did not re-read cli_prompt`);
      if (shell.promptText !== rePrompt.r) fail(`${where} promptText diverged from post-script cli_prompt`);
    } else if (st.t === 'key') {
      const log = () => calls.slice(cm);
      if (st.op === '?') {
        frames.push({ text: `## key: ?${st.text ? ` ${st.text}` : ''}`, scope: 'bare' });
        input.value = st.text;
        input.selectionStart = st.text.length;
        fire({ key: '?' });
        const help = lastCall(log(), 'lab.cli_help');
        if (!help) fail(`${where} '?' step reached no cli_help call`);
        if (help.args[0] !== dev || help.args[1] !== st.text) fail(`${where} cli_help args diverged`);
        ctx.result = help.r;
        checkOrdering(id, 'key', log());
        frames.push(...helpFrames(help.r, armed));
        if (shell.promptText !== help.r.prompt) fail(`${where} promptText diverged from help prompt`);
      } else if (st.op === 'tab') {
        frames.push({ text: `## key: tab${st.text ? ` ${st.text}` : ''}`, scope: 'bare' });
        input.value = st.text;
        input.selectionStart = st.text.length;
        ctx.p = shell.promptText ?? '';
        fire({ key: 'Tab' });
        const comp = lastCall(log(), 'lab.cli_complete');
        if (!comp) fail(`${where} tab step reached no cli_complete call`);
        if (comp.args[0] !== dev || comp.args[1] !== st.text) fail(`${where} cli_complete args diverged`);
        const rePrompt = lastCall(log(), 'lab.cli_prompt');
        if (!rePrompt) fail(`${where} tab step did not re-read cli_prompt`);
        ctx.result = comp.r;
        checkOrdering(id, 'key', log());
        frames.push(...completeFrames(comp.r, rePrompt.r, armed));
        if (comp.r.t === 'line' && input.value !== comp.r.line) {
          fail(`${where} Line splice left input '${input.value}', want '${comp.r.line}'`);
        }
      } else if (st.op === 'ctrlc' || st.op === 'ctrlz') {
        frames.push({ text: `## key: ${st.op}`, scope: 'bare' });
        fire({ key: st.op === 'ctrlc' ? 'c' : 'z', ctrlKey: true });
        const key = lastCall(log(), 'lab.cli_key');
        if (!key) fail(`${where} ${st.op} step reached no cli_key call`);
        if (key.args[0] !== dev || key.args[1] !== st.op) fail(`${where} cli_key args diverged`);
        ctx.result = key.r;
        checkOrdering(id, 'key', log());
        frames.push(...opFrames(key.r.out));
        frames.push(promptFrame(key.r.prompt, key.r.pending != null));
        armed = key.r.pending != null;
        if (st.op === 'ctrlc' && probeJob) { probeMuted = true; probeJob = false; }
        if (shell.pending !== armed) fail(`${where} adapter pending flag diverged`);
        if (shell.promptText !== key.r.prompt) fail(`${where} promptText diverged from key prompt`);
      } else {
        fail(`${where} unknown key op '${st.op}'`);
      }
    } else if (st.t === 'tick') {
      frames.push({ text: `## tick: +${st.n}`, scope: 'bare' });
      for (let i = 0; i < st.n; i++) host.lab.tick();
      ctx.evs = drainNow();
      const log = calls.slice(cm);
      checkOrdering(id, 'tick', log);
      if (labCall(log, 'lab.tick').length !== st.n) fail(`${where} pumped ${labCall(log, 'lab.tick').length} ticks, want ${st.n}`);
      if (labCall(log, 'lab.drain_events').length !== 1) fail(`${where} drained ${labCall(log, 'lab.drain_events').length} times, want 1`);
      frames.push(...eventFrames(ctx.evs));
    } else if (st.t === 'drain') {
      frames.push({ text: '## drain', scope: 'bare' });
      ctx.evs = drainNow();
      checkOrdering(id, 'drain', calls.slice(cm));
      frames.push(...eventFrames(ctx.evs));
    } else if (st.t === 'allow-events') {
      frames.push({ text: '## allow-events', scope: 'bare' });
      allowEvents = true;
      checkOrdering(id, 'allow-events', calls.slice(cm));
    } else {
      fail(`${where} unknown step type '${st.t}'`);
    }
    bellsSeen += ctx.bells;
    if (fx.flashes !== bellsSeen) {
      fail(`${where} bell ops flashed ${fx.flashes} times, want ${bellsSeen}`);
    }
    assertSpans(id, st, writes.slice(wm), [...stepSpans(st, ctx)]);
  }

  frames.push(promptFrame(lab.cli_prompt(dev), armed));
  if (!allowEvents) {
    const left = JSON.parse(lab.drain_events());
    if (left.length !== 0) {
      fail(`unasserted events left at case end — ${left.length} events undrained (use ## drain or ## allow-events)`);
    }
  }
  const set = maskSet(rec.masks ?? []);
  const got = normalizeFrames(frames, set);
  assertFrames(id, got, rec.frames);
  checkMasksUsed(set, id);
  return { frames: got.length };
}

test('corpus-steps.json carries assertable records', () => {
  assert.deepStrictEqual(records.map(r => r.id).sort(), RECORD_IDS, 'record id set drifted from c01..c20');
  for (const rec of records) {
    assert.ok(rec.id, 'record without id');
    assert.ok(Array.isArray(rec.steps) && rec.steps.length > 0, `${rec.id}: zero-step record`);
    assert.ok(Array.isArray(rec.frames), `${rec.id}: no frames array`);
    assert.ok(RECORD_RIGS[rec.id], `${rec.id}: no rig entry`);
  }
});

for (const rec of records) {
  test(rec.id, () => driveRecord(rec));
}

test('missing session maps to the deleted-node guard', t => {
  const lab = new NetLab();
  t.after(() => lab.free());
  const { dev, devs } = buildRig(lab, { kind: 'router', topo: 'single-router' });
  const devices = new Map(devs.map(d => [d.id, { name: d.name, kind: d.kind, powered: true }]));
  const meta = new Map([[dev, {}]]);
  const { host, calls, writes } = makeFacade(lab, devices, meta);
  const shell = new SimShell(host, dev, (text, cls) => writes.push({ text, cls }));
  shell.attach(fakeDom(fakeInput(), () => {}));
  shell.mount();
  lab.remove_device(dev);
  const before = calls.filter(c => c.f === 'lab.cli_exec').length;
  const r = shell.run('enable');
  assert.strictEqual(r, null);
  assert.strictEqual(shell.state, 'orphaned');
  assert.deepStrictEqual(writes.at(-1), { text: '% node was deleted\n', cls: 'err' });
  assert.strictEqual(calls.filter(c => c.f === 'lab.cli_exec').length, before + 1);
  assert.strictEqual(shell.run('enable'), null);
  assert.strictEqual(calls.filter(c => c.f === 'lab.cli_exec').length, before + 1);
});

test('the harness fails loud', () => {
  assert.throws(
    () => checkOrdering('x', 'exec', [{ f: 'lab.cli_exec' }]),
    /checkpoint ordering violated|exactly 1/,
    'a mutation call with no checkpoint must fail',
  );
  assert.throws(
    () => checkOrdering('x', 'exec', [{ f: 'app.checkpoint' }, { f: 'app.checkpoint' }, { f: 'lab.cli_exec' }]),
    /exactly 1/,
    'a doubled checkpoint before cli_exec must fail',
  );
  assert.throws(
    () => checkOrdering('x', 'key', [{ f: 'lab.cli_help' }, { f: 'app.checkpoint' }]),
    /no checkpoint expectation/,
    'a read-only step pushing a checkpoint must fail',
  );
  assert.throws(() => opFrames([{ t: 'bogus' }]), /unknown op tag/, 'unknown Op tag must fail');
  assert.throws(() => helpFrames({ t: 'bogus' }), /unknown help result tag/, 'unknown help tag must fail');
  assert.throws(() => opFrames([{ t: 'intent', kind: 'warp' }]), /unknown intent kind/, 'unknown Intent tag must fail');
  assert.throws(() => maskSet(['uptime', 'bogus']), /unknown mask/, 'unknown mask name must fail');
  {
    // no corpus record declares masks — exercise the port directly:
    // uptime hits Engine+Event scopes and precedes tick; tick is
    // Event-scoped only; a declared-but-silent mask fails.
    const set = maskSet(['uptime', 'tick']);
    const got = normalizeFrames(
      [
        { text: 'up 01:02:03, tick 40', scope: 'engine' },
        { text: '!event sys tick 40 in 2w3d', scope: 'event' },
        { text: '!prompt R#', scope: 'bare' },
      ],
      set,
    );
    assert.deepStrictEqual(got, [
      'up <uptime>, tick 40',
      '!event sys tick <tick> in <uptime>',
      '!prompt R#',
    ]);
    checkMasksUsed(set, 'maskself');
    assert.throws(
      () => checkMasksUsed(maskSet(['uptime']), 'maskself'),
      /declared mask 'uptime' never fired/,
      'a declared mask that never fires must fail',
    );
  }
  const broken = JSON.parse(JSON.stringify(records[0]));
  const mid = Math.floor(broken.frames.length / 2);
  [broken.frames[mid - 1], broken.frames[mid]] = [broken.frames[mid], broken.frames[mid - 1]];
  assert.throws(() => driveRecord(broken), /frame mismatch/, 'a reordered record must fail the byte-compare');
  const empty = { id: 'c01-modes-ladder', masks: [], steps: [], frames: [] };
  assert.throws(() => driveRecord(empty), /zero steps/, 'a zero-step record must fail');
  const alien = { id: 'c99-nowhere', masks: [], steps: [{ t: 'exec', line: 'enable' }], frames: [] };
  assert.throws(() => driveRecord(alien), /no rig entry/, 'an unknown record id must fail');
});
