// SPIRALVISION-10 // web deck + page FX. Rewind. Play. Spiral. Forever.
import { loadSpiralVision } from './spiralvision.js';

const $ = (s) => document.querySelector(s);
const $$ = (s) => [...document.querySelectorAll(s)];
const NEON = ['#ff2bd6', '#00f0ff', '#39ff14', '#ffe600', '#ff007a'];

/* ======================= HERO: NEON TUNNEL ======================= */
function superellipse(ctx, cx, cy, rx, ry, n, rot) {
  ctx.beginPath();
  for (let i = 0; i <= 96; i++) {
    const t = (i / 96) * Math.PI * 2;
    const c = Math.cos(t), s = Math.sin(t);
    const x = Math.sign(c) * Math.abs(c) ** (2 / n) * rx;
    const y = Math.sign(s) * Math.abs(s) ** (2 / n) * ry;
    const xr = x * Math.cos(rot) - y * Math.sin(rot);
    const yr = x * Math.sin(rot) + y * Math.cos(rot);
    i ? ctx.lineTo(cx + xr, cy + yr) : ctx.moveTo(cx + xr, cy + yr);
  }
  ctx.closePath();
}

function runWhenVisible(el, draw) {
  let on = false, raf = 0;
  const loop = (t) => { draw(t); if (on) raf = requestAnimationFrame(loop); };
  new IntersectionObserver(([e]) => {
    on = e.isIntersecting && !matchMedia('(prefers-reduced-motion: reduce)').matches;
    cancelAnimationFrame(raf);
    if (on) raf = requestAnimationFrame(loop); else draw(0);
  }).observe(el);
}

(function tunnel() {
  const cv = $('#tunnel'); const ctx = cv.getContext('2d');
  const fit = () => { const r = cv.getBoundingClientRect(); const d = Math.min(devicePixelRatio, 2); cv.width = r.width * d; cv.height = r.height * d; };
  addEventListener('resize', fit); fit();
  runWhenVisible(cv, (t) => {
    const w = cv.width, h = cv.height, T = t / 1000;
    ctx.fillStyle = 'rgba(18,0,42,0.35)'; ctx.fillRect(0, 0, w, h);
    const cx = w / 2 + Math.sin(T * 0.6) * w * 0.05, cy = h / 2 + Math.cos(T * 0.5) * h * 0.05;
    for (let i = 0; i < 18; i++) {
      const z = ((i + T * 1.6) % 18) / 18;           // 0 far .. 1 near
      const s = Math.pow(z, 2.2) * Math.max(w, h) * 1.1;
      ctx.strokeStyle = NEON[i % NEON.length];
      ctx.lineWidth = 1 + z * 9 * devicePixelRatio;
      ctx.shadowColor = ctx.strokeStyle; ctx.shadowBlur = 18;
      ctx.globalAlpha = Math.min(1, z * 1.6);
      superellipse(ctx, cx, cy, s * 0.66, s * 0.5, 2 + z * 8, T * 0.4 + i * 0.12);
      ctx.stroke();
    }
    ctx.globalAlpha = 1; ctx.shadowBlur = 0;
  });
})();

/* ======================= HOW: SCAN PATH ======================= */
(function scan() {
  const cv = $('#scanCanvas'); const ctx = cv.getContext('2d');
  const K = 80, W = 640, H = 480, STEPS = 1400;
  const pt = (p, arm) => {
    const q = Math.sqrt(p), th = K * q, a = 1 + (4 / 3 - 1) * q, n = 2 + 12 * q;
    const c = Math.cos(th), s = Math.sin(th);
    const rb = (Math.abs(c / a) ** n + Math.abs(s) ** n) ** (-1 / n);
    const sc = q * (H / 2) * 1.03 * (arm ? -1 : 1);
    return [W / 2 + rb * c * sc, H / 2 + rb * s * sc];
  };
  runWhenVisible(cv, (t) => {
    const prog = ((t / 1000) % 6) / 5;            // 5 s sweep + 1 s hold
    ctx.fillStyle = '#05000c'; ctx.fillRect(0, 0, W, H);
    const end = Math.min(1, prog) * STEPS;
    for (const arm of [0, 1]) {
      ctx.strokeStyle = arm ? '#00f0ff' : '#ff2bd6'; ctx.lineWidth = 2.2;
      ctx.shadowColor = ctx.strokeStyle; ctx.shadowBlur = 8;
      ctx.beginPath();
      for (let i = 0; i <= end; i++) { const [x, y] = pt(i / STEPS, arm); i ? ctx.lineTo(x, y) : ctx.moveTo(x, y); }
      ctx.stroke();
      const [x, y] = pt(Math.min(1, prog), arm);
      ctx.fillStyle = '#ffe600'; ctx.beginPath(); ctx.arc(x, y, 7, 0, Math.PI * 2); ctx.fill();
    }
    ctx.shadowBlur = 0;
    ctx.font = '28px VT323, monospace'; ctx.fillStyle = '#39ff14';
    ctx.fillText(`SAMPLE ${Math.floor(Math.min(1, prog) * 4320).toString().padStart(4, '0')}/4320`, 14, 30);
  });
})();

/* ======================= DEMO TAPES ======================= */
const DEMOS = [
  { id: 'mandelbrot', title: 'MANDELBROT MALL', color: '#ff2bd6', cap: 'Infinite zoom into the fractal food court. Fine detail melts into chunky spiral glow, colours hold strong.' },
  { id: 'neonsign', title: 'NEON SIGN', color: '#ffe600', cap: 'Bouncing chrome-outlined text over a gradient. Yes — you can read it after a cassette. RADICAL.' },
  { id: 'plasma', title: 'NEON PLASMA', color: '#00f0ff', cap: 'Smooth colour fields are SpiralVision’s happy place: FM luma + calibrated AM chroma nail the hues.' },
  { id: 'life', title: 'GAME OF LIFE', color: '#39ff14', cap: 'Pixel-sized cells stress the bandwidth: the spiral turns them into glowing blobs. Tracking error = art.' },
  { id: 'testcard', title: 'TEST CARD 2000', color: '#d7dbe4', cap: 'Colour bars + moving gradient. Watch the edges smear along the spiral like a VHS dub of a dub.' },
  { id: 'noise', title: 'NOISE MODE', color: '#ff007a', cap: 'No video at all: pink noise, then two tones. No sync → the flywheel free-runs at 10 fps and paints the audio as spiral noise.', before: 'AUDIO IN' },
];

(function demos() {
  const box = $('#tapes'), vB = $('#vBefore'), vA = $('#vAfter'), cmp = $('#compare'), split = $('#split');
  let current = null, playing = true;
  DEMOS.forEach((d, i) => {
    const b = document.createElement('button');
    b.className = 'tape'; b.setAttribute('role', 'tab'); b.setAttribute('aria-pressed', 'false');
    b.innerHTML = `<div class="label" style="background:${d.color}">${d.title}</div><div class="reels"><div class="reel"></div><div class="reel"></div></div><div class="side">SIDE ${i % 2 ? 'B' : 'A'} · C-${(i + 3) * 10}</div>`;
    b.onclick = () => select(d, b);
    box.appendChild(b);
  });
  function select(d, btn) {
    current = d;
    $$('.tape').forEach((t) => t.setAttribute('aria-pressed', String(t === btn)));
    vB.poster = vA.poster = `videos/${d.id}-poster.jpg`;
    vB.src = `videos/${d.id}-in.mp4`; vA.src = `videos/${d.id}-out.mp4`;
    $('.compare .tagL').textContent = d.before || 'ORIGINAL';
    $('#demoCaption').textContent = '▶ ' + d.cap;
    // start both halves together once both can play
    const ready = (v) => new Promise((r) => (v.readyState >= 3 ? r() : v.addEventListener('canplay', r, { once: true })));
    const token = (select.token = (select.token || 0) + 1);
    vB.pause(); vA.pause();
    Promise.all([ready(vB), ready(vA)]).then(() => {
      if (token !== select.token) return;
      vB.currentTime = vA.currentTime = 0;
      if (playing) { vB.play().catch(() => {}); vA.play().catch(() => {}); }
    });
  }
  const setSplit = (v) => { cmp.style.setProperty('--split', v + '%'); split.value = v; };
  split.oninput = () => setSplit(split.value);
  let drag = false;
  const fromEv = (e) => { const r = cmp.getBoundingClientRect(); setSplit(Math.max(0, Math.min(100, ((e.clientX - r.left) / r.width) * 100)).toFixed(1)); };
  cmp.addEventListener('pointerdown', (e) => { drag = true; cmp.setPointerCapture(e.pointerId); fromEv(e); });
  cmp.addEventListener('pointermove', (e) => drag && fromEv(e));
  cmp.addEventListener('pointerup', () => (drag = false));
  $('#demoPlay').onclick = (e) => {
    playing = !playing;
    if (playing) { vB.play(); vA.play(); } else { vB.pause(); vA.pause(); }
    e.target.textContent = playing ? '❚❚ PAUSE' : '▶ PLAY';
  };
  $('#demoSound').onclick = (e) => {
    vA.muted = !vA.muted;
    if (!vA.muted) { vA.volume = 0.35; vA.play(); }
    e.target.textContent = vA.muted ? '🔊 HEAR THE TAPE' : '🔇 MUTE (phew)';
  };
  setInterval(() => { if (!vA.paused && Math.abs(vB.currentTime - vA.currentTime) > 0.08) vB.currentTime = vA.currentTime; }, 400);
  // load lazily when the section first scrolls into view
  new IntersectionObserver(([e], o) => { if (e.isIntersecting) { select(DEMOS[0], box.firstChild); o.disconnect(); } }, { rootMargin: '200px' }).observe(cmp);
})();

/* ======================= THE DECK ======================= */
const deck = {
  mode: 'loop', src: 'pattern', input: 'mic',
  sv: null, ctx: null, node: null, gain: null, running: false,
  enc: null, dec: null, tape: null,
  media: null, video: null, image: null, fileSrc: null,
  nextT: 0, timer: 0, queue: [], shown: 0, fpsT: [], rec: null, recAudio: null,
  lastAudio: null,
};
const cvSrc = $('#cvSrc'), cvOut = $('#cvOut'), scope = $('#scope');
const gSrc = cvSrc.getContext('2d', { willReadFrequently: true }), gOut = cvOut.getContext('2d'), gScope = scope.getContext('2d');
const grab = document.createElement('canvas'); grab.width = 320; grab.height = 240;
const gGrab = grab.getContext('2d', { willReadFrequently: true });

const NOTES = {
  loop: 'Source → encoder → simulated cassette → decoder. No hardware needed. Crank the TAPE WEAR, dude.',
  enc: 'Source → encoder → your speakers / line out. Plug into a tape deck, hit REC on both. REC here saves a .wav tape.',
  dec: 'Line in / mic / audio file → decoder. Play a SpiralVision tape (or ANY audio for noise mode). Turn OFF any voice processing.',
};

function segment(id, key, onChange) {
  $$(`#${id} button`).forEach((b) => b.onclick = () => {
    $$(`#${id} button`).forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
    onChange(b.dataset[key]);
  });
}
function updateModeUI() {
  $('#modeNote').textContent = NOTES[deck.mode];
  $('#srcBox').hidden = deck.mode === 'dec';
  $('#inBox').hidden = deck.mode !== 'dec';
  $('#wearBox').hidden = deck.mode !== 'loop';
  $('#screenA .lbl').textContent = deck.mode === 'dec' ? 'SIDE A: TAPE IN' : 'SIDE A: SOURCE';
  $('#monitor').checked = deck.mode === 'enc';
}
segment('modeSeg', 'mode', (m) => { stop(); deck.mode = m; updateModeUI(); });
segment('srcSeg', 'src', (s) => { deck.src = s; if (s === 'file') $('#videoFile').click(); else if (deck.running) restartSource(); });
segment('inSeg', 'in', (s) => { deck.input = s; if (s === 'file') $('#audioFile').click(); });
$('#videoFile').onchange = (e) => { deck.fileSrc = e.target.files[0] || null; if (deck.running) restartSource(); };
$('#audioFile').onchange = (e) => { deck.fileSrc = e.target.files[0] || null; };
updateModeUI();

const knob = (id, fmt = (v) => (+v).toFixed(2)) => {
  const el = $('#' + id), out = $('#' + id + 'Val');
  const f = () => { out.textContent = fmt(el.value); applyKnobs(); };
  el.oninput = f; return el;
};
const wearEl = knob('wear'), persistEl = knob('persist'), satEl = knob('sat');
function applyKnobs() {
  if (deck.dec) deck.dec.set(+persistEl.value, +satEl.value);
  if (deck.tape) deck.tape.setWear(+wearEl.value);
}
$('#monitor').onchange = () => { if (deck.gain) deck.gain.gain.value = $('#monitor').checked ? 1 : 0; };

function hud(lock, speed, fps, frames) {
  const l = $('#hudLock');
  l.className = 'pill ' + (lock === null ? '' : lock ? 'on' : 'free');
  l.textContent = lock === null ? 'STOPPED' : lock ? '● SYNC LOCKED' : '○ FREE-RUN / NOISE';
  $('#hudSpeed').textContent = speed ? `SPEED ${(speed * 100).toFixed(1)}%` : 'SPEED —';
  $('#hudFps').textContent = `${fps.toFixed(1)} FPS`;
  $('#hudFrames').textContent = `FRAME ${frames}`;
}

function noSignal(g, txt = 'NO SIGNAL') {
  g.fillStyle = '#000'; g.fillRect(0, 0, 320, 240);
  for (let i = 0; i < 1800; i++) { const v = Math.random() * 90 | 0; g.fillStyle = `rgb(${v},${v},${v})`; g.fillRect(Math.random() * 320, Math.random() * 240, 2, 2); }
  g.font = '34px Bungee, Impact, sans-serif'; g.textAlign = 'center'; g.fillStyle = '#ff2bd6'; g.fillText(txt, 160, 132); g.textAlign = 'start';
}
noSignal(gSrc); noSignal(gOut);

function drawCover(g, el, w, h) {
  const sw = el.videoWidth || el.naturalWidth || el.width, sh = el.videoHeight || el.naturalHeight || el.height;
  if (!sw || !sh) return false;
  const s = Math.max(w / sw, h / sh), dw = sw * s, dh = sh * s;
  g.drawImage(el, (w - dw) / 2, (h - dh) / 2, dw, dh);
  return true;
}

let patT = 0;
function drawPattern(g, t) {
  // neon mall test pattern: bars, spinning spiral, bouncing logo, clock
  const bars = ['#ffffff', '#ffe600', '#00f0ff', '#39ff14', '#ff2bd6', '#ff1744', '#2a5cff', '#000000'];
  bars.forEach((c, i) => { g.fillStyle = c; g.fillRect(i * 40, 0, 40, 240); });
  g.save(); g.translate(160, 120); g.rotate(t * 0.9);
  for (let i = 0; i < 9; i++) { g.strokeStyle = NEON[i % 5]; g.lineWidth = 7; g.beginPath(); g.arc(0, 0, 12 + i * 11, i * 0.7, i * 0.7 + 4); g.stroke(); }
  g.restore();
  g.font = '30px Bungee, Impact, sans-serif'; g.textAlign = 'center';
  const y = 60 + Math.sin(t * 2.2) * 30;
  g.lineWidth = 6; g.strokeStyle = '#000'; g.strokeText('SPIRAL', 160, y); g.fillStyle = '#fff'; g.fillText('SPIRAL', 160, y);
  g.strokeText('VISION', 160, 240 - y + 30); g.fillStyle = '#ffe600'; g.fillText('VISION', 160, 240 - y + 30);
  g.fillStyle = '#000'; g.fillRect(110, 214, 100, 24); g.fillStyle = '#39ff14'; g.font = '22px VT323, monospace';
  g.fillText(new Date().toLocaleTimeString(), 160, 233); g.textAlign = 'start';
}

async function restartSource() {
  stopSource();
  if (deck.src === 'camera') {
    deck.media = await navigator.mediaDevices.getUserMedia({ video: { width: { ideal: 640 }, height: { ideal: 480 } }, audio: false });
    deck.video = Object.assign(document.createElement('video'), { muted: true, playsInline: true, srcObject: deck.media });
    await deck.video.play();
  } else if (deck.src === 'file' && deck.fileSrc) {
    const url = URL.createObjectURL(deck.fileSrc);
    if (deck.fileSrc.type.startsWith('image/')) {
      deck.image = new Image(); deck.image.src = url; await deck.image.decode();
    } else {
      deck.video = Object.assign(document.createElement('video'), { muted: true, playsInline: true, loop: true, src: url });
      await deck.video.play();
    }
  }
}
function stopSource() {
  if (deck.media) deck.media.getTracks().forEach((t) => t.stop());
  if (deck.video) { deck.video.pause(); if (deck.video.src.startsWith('blob:')) URL.revokeObjectURL(deck.video.src); }
  deck.media = deck.video = deck.image = null;
}
function grabFrame(t) {
  if (deck.video && drawCover(gGrab, deck.video, 320, 240)) { /* ok */ }
  else if (deck.image) {
    const z = 1 + 0.15 * Math.sin(t * 0.4);
    gGrab.save(); gGrab.translate(160, 120); gGrab.scale(z, z); gGrab.translate(-160, -120); drawCover(gGrab, deck.image, 320, 240); gGrab.restore();
  } else drawPattern(gGrab, t);
  return gGrab.getImageData(0, 0, 320, 240);
}

async function ensureAudio() {
  if (!deck.sv) {
    $('#loading').style.display = 'block';
    deck.sv = await loadSpiralVision();
    $('#loading').style.display = 'none';
  }
  try { deck.ctx = new AudioContext({ sampleRate: 48000, latencyHint: 'interactive' }); }
  catch { deck.ctx = new AudioContext(); }
  await deck.ctx.audioWorklet.addModule(new URL('./sv-worklet.js', import.meta.url));
  deck.node = new AudioWorkletNode(deck.ctx, 'spiralvision-io', { numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [2], channelCount: 2, channelCountMode: 'explicit' });
  deck.gain = deck.ctx.createGain();
  deck.gain.gain.value = $('#monitor').checked ? 1 : 0;
  deck.node.connect(deck.gain).connect(deck.ctx.destination);
  await deck.ctx.resume();
}

async function play() {
  if (deck.running) return;
  try {
    await ensureAudio();
    const sv = deck.sv;
    deck.dec = new sv.Decoder(deck.mode === 'dec' ? deck.ctx.sampleRate : 48000);
    deck.queue = []; deck.shown = 0; deck.fpsT = [];
    applyKnobs();
    if (deck.mode === 'dec') await startDecode();
    else await startEncode();
    deck.running = true;
    $('#play').textContent = '■ STOP'; $('#play').classList.replace('green', 'red');
    requestAnimationFrame(render);
  } catch (err) {
    console.error(err);
    stop();
    alert('BOGUS! ' + (err && err.message ? err.message : err));
  }
}

async function startEncode() {
  const sv = deck.sv;
  deck.enc = new sv.Encoder();
  deck.tape = deck.mode === 'loop' ? new sv.Tape(+wearEl.value, (Math.random() * 1e9) | 0) : null;
  await restartSource();
  deck.node.port.postMessage({ clear: true, capture: false, thru: false });
  deck.nextT = deck.ctx.currentTime + 0.15;
  const t0 = performance.now();
  const tick = () => {
    const ctx = deck.ctx; if (!ctx) return;
    while (deck.nextT < ctx.currentTime + 0.3) {
      const t = (performance.now() - t0) / 1000;
      const img = grabFrame(t);
      let a = deck.enc.encode(img.data);
      if (deck.tape && +wearEl.value > 0) a = deck.tape.process(a);
      deck.lastAudio = a;
      if (deck.recAudio) deck.recAudio.push(a);
      deck.dec.push(a);
      const at = deck.nextT;
      deck.queue.push({ at, src: img });
      let f; let k = 0;
      while ((f = deck.dec.pop())) deck.queue.push({ at: at + 0.1 * k++, out: f });
      deck.nextT += a.length / 2 / 48000; // before transfer: a.length is 0 afterwards
      deck.node.port.postMessage({ play: a.slice() });
    }
  };
  tick();
  deck.timer = setInterval(tick, 25);
}

async function startDecode() {
  const ctx = deck.ctx;
  deck.node.port.postMessage({ clear: true, capture: true, thru: true });
  deck.node.port.onmessage = (e) => {
    const [L, R] = e.data.cap;
    deck.lastAudio = { L, R };
    deck.dec.pushPlanar(L, R);
    let f;
    while ((f = deck.dec.pop())) deck.queue.push({ at: 0, out: f });
    drawTapeIn(L, R);
  };
  if (deck.input === 'file') {
    if (!deck.fileSrc) throw new Error('pick an audio file first (💾 AUDIO FILE)');
    const buf = await ctx.decodeAudioData(await deck.fileSrc.arrayBuffer());
    const src = ctx.createBufferSource(); src.buffer = buf;
    src.connect(deck.node); src.start();
    src.onended = () => setTimeout(stop, 400);
    deck.fileNode = src;
  } else {
    deck.media = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false, channelCount: { ideal: 2 } } });
    ctx.createMediaStreamSource(deck.media).connect(deck.node);
  }
}

function drawTapeIn(L, R) {
  const g = gSrc; g.fillStyle = 'rgba(5,0,12,0.35)'; g.fillRect(0, 0, 320, 240);
  for (const [ch, col, y0] of [[L, '#ff2bd6', 65], [R, '#00f0ff', 175]]) {
    g.strokeStyle = col; g.lineWidth = 2; g.beginPath();
    for (let x = 0; x < 320; x++) { const v = ch[Math.floor((x / 320) * ch.length)] || 0; const y = y0 - v * 50; x ? g.lineTo(x, y) : g.moveTo(x, y); }
    g.stroke();
  }
}

function drawScope() {
  const a = deck.lastAudio; const g = gScope;
  g.fillStyle = 'rgba(0,10,0,0.25)'; g.fillRect(0, 0, 260, 260);
  if (!a) return;
  g.strokeStyle = '#39ff14'; g.lineWidth = 1.2; g.shadowColor = '#39ff14'; g.shadowBlur = 6; g.beginPath();
  const n = a.L ? a.L.length : a.length / 2;
  const step = Math.max(1, Math.floor(n / 900));
  for (let i = 0; i < n; i += step) {
    const l = a.L ? a.L[i] : a[2 * i], r = a.L ? a.R[i] : a[2 * i + 1];
    const x = 130 + (l - r) * 0.7 * 90, y = 130 - (l + r) * 0.7 * 90;
    i ? g.lineTo(x, y) : g.moveTo(x, y);
  }
  g.stroke(); g.shadowBlur = 0;
}

function render() {
  if (!deck.running) return;
  const now = deck.ctx.currentTime;
  while (deck.queue.length && (deck.mode === 'dec' || deck.queue[0].at <= now)) {
    const q = deck.queue.shift();
    if (q.src) gSrc.putImageData(q.src, 0, 0);
    if (q.out) {
      gOut.putImageData(new ImageData(q.out, 320, 240), 0, 0);
      deck.shown++;
      deck.fpsT.push(performance.now());
      if (deck.fpsT.length > 20) deck.fpsT.shift();
    }
  }
  const ft = deck.fpsT, fps = ft.length > 1 ? ((ft.length - 1) * 1000) / (ft[ft.length - 1] - ft[0]) : 0;
  hud(deck.dec.locked, deck.dec.locked ? deck.dec.speed : 0, fps, deck.shown);
  drawScope();
  requestAnimationFrame(render);
}

function stop() {
  clearInterval(deck.timer);
  if (deck.rec || deck.recAudio) finishRec();
  stopSource();
  if (deck.fileNode) { try { deck.fileNode.stop(); } catch {} deck.fileNode = null; }
  if (deck.ctx) deck.ctx.close();
  for (const k of ['enc', 'dec', 'tape']) if (deck[k]) { deck[k].free(); deck[k] = null; }
  deck.ctx = deck.node = deck.gain = null;
  deck.running = false; deck.lastAudio = null;
  $('#play').textContent = '▶ PLAY'; $('#play').classList.replace('red', 'green');
  hud(null, 0, 0, deck.shown);
}

$('#play').onclick = () => (deck.running ? stop() : play());

/* ---- recording ---- */
function wavBlob(chunks, rate = 48000) {
  const n = chunks.reduce((s, c) => s + c.length, 0);
  const buf = new ArrayBuffer(44 + n * 2), v = new DataView(buf);
  const str = (o, s) => [...s].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  str(0, 'RIFF'); v.setUint32(4, 36 + n * 2, true); str(8, 'WAVE'); str(12, 'fmt ');
  v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 2, true); v.setUint32(24, rate, true);
  v.setUint32(28, rate * 4, true); v.setUint16(32, 4, true); v.setUint16(34, 16, true); str(36, 'data'); v.setUint32(40, n * 2, true);
  let o = 44;
  for (const c of chunks) for (let i = 0; i < c.length; i++, o += 2) v.setInt16(o, Math.max(-1, Math.min(1, c[i])) * 32767, true);
  return new Blob([buf], { type: 'audio/wav' });
}
function download(blob, name) {
  const a = Object.assign(document.createElement('a'), { href: URL.createObjectURL(blob), download: name });
  document.body.appendChild(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(a.href), 5000);
}
function finishRec() {
  if (deck.recAudio) { download(wavBlob(deck.recAudio), 'spiralvision-tape.wav'); deck.recAudio = null; }
  if (deck.rec) { deck.rec.stop(); deck.rec = null; }
  $('#rec').textContent = '⏺ REC';
}
$('#rec').onclick = async () => {
  if (deck.rec || deck.recAudio) return finishRec();
  if (!deck.running) await play();
  if (!deck.running) return;
  if (deck.mode === 'enc') deck.recAudio = [];
  else {
    const chunks = []; const r = new MediaRecorder(cvOut.captureStream(10));
    r.ondataavailable = (e) => chunks.push(e.data);
    r.onstop = () => download(new Blob(chunks, { type: r.mimeType || 'video/webm' }), 'spiralvision.webm');
    r.start(); deck.rec = r;
  }
  $('#rec').textContent = '■ SAVE';
};
$('#snap').onclick = () => cvOut.toBlob((b) => download(b, 'spiralvision-snap.png'));

/* ======================= PWA ======================= */
if ('serviceWorker' in navigator && location.protocol !== 'file:') {
  navigator.serviceWorker.register('sw.js').catch(() => {});
}
let installEvt = null;
addEventListener('beforeinstallprompt', (e) => { e.preventDefault(); installEvt = e; });
$('#installBtn').onclick = async (e) => {
  e.preventDefault();
  if (installEvt) { installEvt.prompt(); installEvt = null; return; }
  if (matchMedia('(display-mode: standalone)').matches) return alert('You already installed it, dude. TUBULAR.');
  alert('INSTALL IT:\n• Chrome / Edge / Android: browser menu → "Install app"\n• iPhone / iPad: Share → "Add to Home Screen"\n• Firefox: use the downloads above');
};
