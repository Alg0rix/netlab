// Renders promo/reel.html frame by frame with headless Chromium and encodes
// the result with ffmpeg (plus the soundtrack from music.mjs).
//
//   node promo/render.mjs                    full render → promo/build/netlab-showreel.mp4
//   node promo/render.mjs --stills 3,14.5    PNG stills → promo/build/stills/
//   node promo/render.mjs --from 30 --to 40  render only a range (preview)
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { FPS, DURATION } from './timeline.js';

const { chromium } = createRequire(import.meta.url)('playwright');

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..');
const BUILD = path.join(HERE, 'build');
const args = process.argv.slice(2);
const opt = k => { const i = args.indexOf(k); return i < 0 ? null : args[i + 1]; };
const WORKERS = +(opt('--workers') ?? 4);

// tiny static server for the repo root (reel.html imports ../web/icons.js)
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.json': 'application/json', '.jpg': 'image/jpeg', '.png': 'image/png', '.m4a': 'audio/mp4', '.wav': 'audio/wav' };
const server = http.createServer((req, res) => {
  const p = path.join(ROOT, decodeURIComponent(new URL(req.url, 'http://x').pathname));
  if (!p.startsWith(ROOT) || !fs.existsSync(p) || fs.statSync(p).isDirectory()) { res.writeHead(404); return res.end(); }
  res.writeHead(200, { 'Content-Type': TYPES[path.extname(p)] ?? 'application/octet-stream' });
  fs.createReadStream(p).pipe(res);
});
await new Promise(r => server.listen(0, r));
const PAGE_URL = `http://127.0.0.1:${server.address().port}/promo/reel.html?render`;

const browser = await chromium.launch({ args: ['--force-color-profile=srgb', '--disable-lcd-text'] });
async function newPage() {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  page.on('pageerror', e => console.error('page error:', e.message));
  page.on('console', m => { if (m.type() === 'error') console.error('console:', m.text()); });
  await page.goto(PAGE_URL);
  await page.evaluate(() => window.__ready);
  return page;
}
const shot = (page, t, type = 'jpeg') => page.evaluate(t => window.__seek(t), t)
  .then(() => page.screenshot({ type, quality: type === 'jpeg' ? 95 : undefined, clip: { x: 0, y: 0, width: 1920, height: 1080 } }));

if (opt('--stills')) {
  const dir = path.join(BUILD, 'stills', opt('--tag') ?? 'default');
  fs.mkdirSync(dir, { recursive: true });
  const page = await newPage();
  for (const t of opt('--stills').split(',').map(Number)) {
    fs.writeFileSync(path.join(dir, `t${t.toFixed(2).padStart(6, '0')}.png`), await shot(page, t, 'png'));
    console.log('still', t);
  }
} else {
  const from = +(opt('--from') ?? 0), to = +(opt('--to') ?? DURATION);
  const fps = +(opt('--fps') ?? FPS);
  const n0 = Math.round(from * fps), n1 = Math.round(to * fps);
  const dir = path.join(BUILD, 'frames');
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  const started = Date.now();
  let done = 0;
  // each worker renders an interleaved slice of frames so they all finish together
  await Promise.all(Array.from({ length: WORKERS }, async (_, w) => {
    const page = await newPage();
    for (let i = n0 + w; i < n1; i += WORKERS) {
      fs.writeFileSync(path.join(dir, `${String(i - n0).padStart(5, '0')}.jpg`), await shot(page, i / fps));
      if (++done % 120 === 0) {
        const el = (Date.now() - started) / 1000;
        console.log(`${done}/${n1 - n0} frames · ${el.toFixed(0)}s · eta ${((n1 - n0 - done) * el / done).toFixed(0)}s`);
      }
    }
    await page.close();
  }));
  const out = opt('--out') ?? path.join(BUILD, from === 0 && to === DURATION ? 'netlab-showreel.mp4' : `preview-${from}-${to}.mp4`);
  const audio = path.join(BUILD, 'music.wav');
  const ff = ['-y', '-loglevel', 'error', '-framerate', String(fps), '-i', path.join(dir, '%05d.jpg')];
  if (fs.existsSync(audio)) ff.push('-ss', String(from), '-t', String(to - from), '-i', audio);
  ff.push('-c:v', 'libx264', '-preset', 'slow', '-crf', '19', '-pix_fmt', 'yuv420p', '-profile:v', 'high', '-movflags', '+faststart',
    '-color_primaries', 'bt709', '-color_trc', 'bt709', '-colorspace', 'bt709');
  if (fs.existsSync(audio)) ff.push('-c:a', 'aac', '-b:a', '256k', '-shortest');
  ff.push(out);
  await new Promise((res, rej) => spawn('ffmpeg', ff, { stdio: 'inherit' }).on('exit', c => c ? rej(new Error('ffmpeg ' + c)) : res()));
  console.log('wrote', out, `in ${((Date.now() - started) / 1000).toFixed(0)}s`);
}
await browser.close();
server.close();
