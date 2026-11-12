// JS port of core/tests/cli/runner.rs rendering + normalization + gap
// classification. The Rust runner is the single grammar/dispatch site; this
// module only re-renders the wire Ops the committed wasm returns into the
// same frame vocabulary and applies the record's declared masks.

export const sep = p => (p.endsWith(' ') ? 0 : 1);
const displayWidth = p => [...p].length;

const CLS_TAG = { ok: '!ok', dim: '!dim', warn: '!warn', err: '!err', echo: '!echo' };

const textFrames = (tag, text) =>
  String(text).split('\n').map(s => ({ text: tag ? `${tag} ${s}` : s, scope: 'engine' }));

const bare = text => ({ text, scope: 'bare' });
const engine = text => ({ text, scope: 'engine' });
const event = text => ({ text, scope: 'event' });

export function opFrames(out = []) {
  const frames = [];
  for (const op of out) {
    if (op.t === 'text') {
      frames.push(...textFrames(CLS_TAG[op.cls] ?? '', op.text));
    } else if (op.t === 'echo') {
      frames.push(...textFrames('', op.text));
    } else if (op.t === 'clear') {
      frames.push(bare('!clear'));
    } else if (op.t === 'bell') {
      frames.push(bare('!bell'));
    } else if (op.t === 'intent') {
      if (op.kind === 'ping' || op.kind === 'trace') {
        frames.push(engine(`!intent ${op.kind} ${op.target}`));
      } else if (op.kind === 'rename') {
        frames.push(engine(`!intent rename ${op.name}`));
      } else if (op.kind === 'nvram-write') {
        const segs = String(op.text).split('\n');
        frames.push(bare(`!intent nvram-write ${segs.length}`));
        for (const seg of segs) frames.push(engine(`  ${seg}`));
      } else if (op.kind === 'nvram-erase' || op.kind === 'reload') {
        frames.push(engine(`!intent ${op.kind}`));
      } else {
        throw new Error(`unknown intent kind ${op.kind}`);
      }
    } else {
      throw new Error(`unknown op tag ${op.t}`);
    }
  }
  return frames;
}

export const promptFrame = (prompt, armed) =>
  bare(armed ? `!pending "${prompt}"` : `!prompt ${prompt}`);

export function helpFrames(r, armed) {
  const frames = [];
  if (r.t === 'rows') {
    frames.push(bare(`!rows ${r.rows.length}`));
    for (const row of r.rows) {
      frames.push(engine(row.help != null ? `  ${row.word} ${row.help}` : `  ${row.word}`));
    }
  } else if (r.t === 'inline') {
    frames.push(engine(`!inline ${r.inline.join(' ')}`));
  } else if (r.t === 'err') {
    frames.push(bare(`!caret ${r.index}`));
  } else {
    throw new Error(`unknown help result tag ${r.t}`);
  }
  frames.push(promptFrame(r.prompt, armed));
  return frames;
}

export function completeFrames(r, prompt, armed) {
  const frames = [];
  if (r.t === 'line') {
    frames.push(engine(`!line ${r.line}`));
  } else if (r.t === 'list') {
    frames.push(engine(`!list ${r.list.join(' ')}`));
  } else if (r.t === 'err') {
    frames.push(bare(`!caret ${r.index}`));
  } else {
    throw new Error(`unknown complete result tag ${r.t}`);
  }
  frames.push(promptFrame(prompt, armed));
  return frames;
}

export const eventFrames = evs =>
  evs.map(ev => event(`!event ${ev.kind} ${ev.msg}`));

// N2 boundary masks — the six ported constants. uptime applies to
// engine+event scopes, tick to event frames only; uptime always precedes
// tick regardless of declaration order. A declared mask that never fires
// fails the record.
const UPTIME_RES = [/\d+:\d{2}:\d{2}/g, /\d+w\d+d/g, /\d+d\d+h/g];
const TICK_RES = [
  [/tick \d+/g, 'tick <tick>'],
  [/tick=\d+/g, 'tick=<tick>'],
  [/t=\d+/g, 't=<tick>'],
];

export const maskSet = names => {
  const set = { usedUptime: false, usedTick: false };
  for (const name of names) {
    if (name !== 'uptime' && name !== 'tick') throw new Error(`unknown mask ${name}`);
    set[name] = true;
  }
  return set;
};

export const checkMasksUsed = (set, id) => {
  if (set.uptime && !set.usedUptime) throw new Error(`${id}: declared mask 'uptime' never fired`);
  if (set.tick && !set.usedTick) throw new Error(`${id}: declared mask 'tick' never fired`);
};

export function normalizeFrames(frames, set) {
  return frames.map(f => {
    let s = f.text;
    if (set.uptime && f.scope !== 'bare') {
      for (const re of UPTIME_RES) {
        const n = s.replace(re, '<uptime>');
        set.usedUptime ||= n !== s;
        s = n;
      }
    }
    if (set.tick && f.scope === 'event') {
      for (const [re, rep] of TICK_RES) {
        const n = s.replace(re, rep);
        set.usedTick ||= n !== s;
        s = n;
      }
    }
    return s.replace(/\r/g, '').split('\n').map(l => l.trimEnd()).join('\n');
  });
}

const TRIAD = [
  "% Invalid input detected at '^' marker.",
  '% Incomplete command.',
  '% Ambiguous command',
];
const TRANSLATE_ERR =
  '% Unknown command or computer name, or unable to find computer address';

export const tokenStarts = line => {
  const b = Buffer.from(line, 'utf8');
  const out = [];
  let i = 0;
  while (i < b.length) {
    if (b[i] === 0x20 || b[i] === 0x09) { i += 1; continue; }
    out.push(i);
    while (i < b.length && b[i] !== 0x20 && b[i] !== 0x09) i += 1;
  }
  return out;
};

// The three honest-error classes a `## gap:` line may discharge on. Any Op
// outside the envelope is a fabricated row — the marker only stands on a
// pure rejection render.
export function checkGap({ fr, tok }, line, prompt, out, mutated, id) {
  const fail = msg => { throw new Error(`${id}: ${msg}`); };
  if (mutated) fail(`gap closed — remove marker, write real golden (line '${line}' mutated)`);
  let caretPad = null, triad = false, pctReject = false, translate = false;
  for (const op of out) {
    if (op.t !== 'text') fail(`gap ${fr} on '${line}' — fabricated ${op.t} op`);
    const text = String(op.text);
    if (text.startsWith('Translating "')) { translate = true; continue; }
    if (text.trim() === '^') { caretPad = displayWidth(text) - 1; continue; }
    if (op.cls === 'err' && (TRIAD.includes(text) || text === TRANSLATE_ERR || text.startsWith('%'))) {
      triad ||= TRIAD.includes(text);
      translate ||= text === TRANSLATE_ERR;
      pctReject ||= !TRIAD.includes(text) && text !== TRANSLATE_ERR;
      continue;
    }
    fail(`gap ${fr} on '${line}' — fabricated row '${text}'`);
  }
  if (!(triad || pctReject || translate)) {
    fail(`gap ${fr} unmet classes on '${line}' — not a caret+triad, a '%' rejection, or a translate render; if the line succeeded: gap closed — remove marker, write real golden`);
  }
  if (tok === 0) {
    if (!translate) fail(`gap ${fr} tok0 unmet on '${line}' — expected the translate render`);
  } else if (tok != null) {
    const starts = tokenStarts(line);
    const start = starts[tok];
    if (start == null) fail(`gap ${fr} tok${tok} unmet on '${line}' — token ${tok} does not exist`);
    const expected = displayWidth(prompt) + sep(prompt) + start;
    if (caretPad !== expected) {
      fail(`gap ${fr} tok${tok} unmet on '${line}' — NoMatch caret did not land on Token[${tok}]`);
    }
  }
}
