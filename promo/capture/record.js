// Records the live netlab app (served on :8471) as timestamped screencast frames,
// plus a log of the scripted cursor path, clicks and named marks. The motion
// composition (promo/reel.html) plays these back with its own animated cursor.
//
//   node promo/capture/record.js            → promo/capture/out/<clip>/*.jpg + clip.json
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const URL = process.env.NETLAB_URL || 'http://localhost:8471/';
const OUT = path.join(__dirname, 'out');
const DSF = 1.5;                       // 1600×900 CSS px → 2400×1350 frames (room to zoom)
const sleep = ms => new Promise(r => setTimeout(r, ms));
const ease = t => t < .5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;

async function clip(browser, name, { blank, dockH }, script) {
  // viewport null + --force-device-scale-factor: the only way CDP screencast frames come back hi-DPI
  const ctx = await browser.newContext({ viewport: null, colorScheme: 'dark' });
  await ctx.addInitScript(([blank, dockH]) => {
    if (sessionStorage.getItem('init')) return;
    sessionStorage.setItem('init', 1);
    localStorage.clear();
    localStorage.setItem('netlab.pref.theme', '"dark"');
    if (dockH) localStorage.setItem('netlab.pref.dockH', String(dockH));
    if (blank) localStorage.setItem('netlab.autosave.v1', JSON.stringify({ format: 'netlab', version: 1, name: 'My first lab', nodes: [], links: [], notes: [] }));
  }, [blank, dockH]);
  const page = await ctx.newPage();
  page.on('pageerror', e => console.log(`[${name}] page error:`, e.message));
  await page.goto(URL);
  await sleep(1800);

  const dir = path.join(OUT, name);
  fs.rmSync(dir, { recursive: true, force: true });
  fs.mkdirSync(dir, { recursive: true });
  const frames = [], cursor = [], clicks = [], marks = {};
  let t0 = null, cur = { x: 800, y: 450 };
  const now = () => Date.now() / 1000 - t0;

  const cdp = await ctx.newCDPSession(page);
  const pending = [];
  cdp.on('Page.screencastFrame', f => {
    cdp.send('Page.screencastFrameAck', { sessionId: f.sessionId }).catch(() => {});
    if (t0 == null) return;
    const i = frames.length, file = String(i).padStart(4, '0') + '.jpg';
    frames.push({ t: +(f.metadata.timestamp - t0).toFixed(3), f: file });
    pending.push(fs.promises.writeFile(path.join(dir, file), Buffer.from(f.data, 'base64')));
  });
  t0 = Date.now() / 1000;
  await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 85, maxWidth: 2400, maxHeight: 1350, everyNthFrame: 1 });

  const api = {
    page, sleep,
    mark(k) { marks[k] = +now().toFixed(3); },
    async move(x, y, ms = 600) {
      const from = { ...cur }, n = Math.max(2, Math.round(ms / 45)), start = Date.now();
      for (let i = 1; i <= n; i++) {
        const target = start + (ms * i) / n;
        const k = ease(i / n);
        cur = { x: from.x + (x - from.x) * k, y: from.y + (y - from.y) * k };
        await page.mouse.move(cur.x, cur.y);
        cursor.push({ t: +now().toFixed(3), x: +cur.x.toFixed(1), y: +cur.y.toFixed(1) });
        const wait = target - Date.now();
        if (wait > 0) await sleep(wait);
      }
    },
    async click(x, y, ms, opts = {}) {
      if (x != null) await api.move(x, y, ms);
      clicks.push({ t: +now().toFixed(3), x: cur.x, y: cur.y, button: opts.button || 'left' });
      await page.mouse.down({ button: opts.button }); await sleep(70); await page.mouse.up({ button: opts.button });
      if (opts.dbl) { await sleep(60); await page.mouse.dblclick(cur.x, cur.y); }
      await sleep(opts.after ?? 250);
    },
    async type(text, delay = 55) {
      for (const ch of text) { await page.keyboard.type(ch); await sleep(delay + Math.random() * 35); }
    },
    async key(k, after = 200) { await page.keyboard.press(k); await sleep(after); },
    async shot(file) { await page.screenshot({ path: file }); },
  };
  await script(api);
  await sleep(400);
  await cdp.send('Page.stopScreencast');
  await Promise.all(pending);
  fs.writeFileSync(path.join(dir, 'clip.json'), JSON.stringify({ name, w: 1600, h: 900, dsf: DSF, duration: +now().toFixed(3), frames, cursor, clicks, marks }));
  console.log(`[${name}] ${frames.length} frames over ${now().toFixed(1)}s`, marks);
  await ctx.close();
}

(async () => {
  const browser = await chromium.launch({ args: [`--force-device-scale-factor=${DSF}`, '--window-size=1600,900'] });
  const only = process.argv[2];

  // ── Clip A: build a two-PC lab from an empty canvas, address it, ping ──
  if (!only || only === 'build') await clip(browser, 'build', { blank: true }, async a => {
    await a.sleep(600);
    a.mark('place');
    await a.click(125, 172, 700);            // VPCS
    await a.click(560, 470, 650);
    await a.click(125, 172, 650);
    await a.click(960, 470, 700);
    await a.click(125, 364, 650);            // Ethernet switch
    await a.click(760, 250, 700, { after: 500 });
    a.mark('cable');
    await a.key('c', 300);
    await a.click(560, 470, 600, { after: 350 });
    await a.click(760, 250, 600, { after: 450 });
    await a.click(805, 295, 350, { after: 400 });   // e0
    await a.click(960, 470, 650, { after: 350 });
    await a.click(760, 250, 600, { after: 450 });
    await a.click(805, 325, 350, { after: 500 });   // e1
    await a.key('v', 200);
    await a.move(1000, 560, 500);
    await a.sleep(400);
    a.mark('console1');
    await a.click(560, 470, 650, { dbl: true, after: 500 });
    await a.type('ip 10.0.0.1/24'); await a.key('Enter', 600);
    a.mark('console2');
    await a.click(960, 470, 700, { dbl: true, after: 500 });
    await a.type('ip 10.0.0.2/24'); await a.key('Enter', 500);
    a.mark('ping');
    await a.type('ping 10.0.0.1 -c 3'); await a.key('Enter', 3400);
    a.mark('end');
  });

  // ── Clip B: Branch office in Simulation mode — packets step hop by hop ──
  if (!only || only === 'sim') await clip(browser, 'sim', { blank: false }, async a => {
    await a.sleep(500);
    a.mark('mode');
    await a.click(1318, 21, 800, { after: 500 });     // Simulation
    await a.click(1468, 21, 600, { after: 300 });     // speed slider → slower
    a.mark('console');
    await a.click(483, 478, 800, { dbl: true, after: 500 });
    await a.type('ping 192.168.20.13 -c 1', 45); await a.key('Enter', 400);
    a.mark('play');
    await a.click(1386, 21, 900, { after: 200 });     // Play
    await a.move(760, 360, 1200);
    await a.sleep(9000);
    a.mark('events');
    await a.click(36, 632, 900, { after: 2200 });     // Events tab
    a.mark('end');
  });

  // ── Clip C: live capture on R1↔SW1, decoded frames + .pcap export ──
  if (!only || only === 'cap') await clip(browser, 'cap', { blank: false }, async a => {
    await a.sleep(500);
    a.mark('ctx');
    await a.click(701, 258, 900, { button: 'right', after: 700 });
    a.mark('start');
    const item = await a.page.locator('#ctx button', { hasText: 'Start capture' }).boundingBox();
    await a.click(item.x + 40, item.y + item.height / 2, 600, { after: 500 });
    a.mark('console');
    const pc1 = await a.page.locator('#topo-list >> text=PC1').first().boundingBox();
    await a.click(pc1.x + 20, pc1.y + pc1.height / 2, 800, { after: 400 });
    const con = await a.page.locator('#inspector button', { hasText: 'Console' }).first().boundingBox();
    await a.click(con.x + con.width / 2, con.y + con.height / 2, 700, { after: 500 });
    await a.type('ping 192.168.20.13 -c 3', 45); await a.key('Enter', 3200);
    a.mark('tab');
    const tab = await a.page.locator('#dock-tabs >> text=Capture').boundingBox();
    await a.click(tab.x + tab.width / 2, tab.y + tab.height / 2, 800, { after: 900 });
    a.mark('row');
    const rows = a.page.locator('#dock-body tbody tr');
    const n = await rows.count();
    const r = n ? await rows.nth(n - 1).boundingBox({ timeout: 2000 }).catch(() => null) : null;
    if (r) await a.click(r.x + 200, r.y + r.height / 2, 800, { after: 1600 });
    a.mark('pcap');
    const save = await a.page.locator('#dock-body [data-a="pcap"]').boundingBox({ timeout: 2000 }).catch(() => null);
    if (save) await a.move(save.x + save.width / 2, save.y + save.height / 2, 900);
    await a.sleep(1500);
    await a.shot(path.join(OUT, 'cap-final.png'));
    a.mark('end');
  });

  await browser.close();
})();
