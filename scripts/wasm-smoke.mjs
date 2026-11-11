// Smoke test: the WASM build encodes, runs the tape sim and decodes with lock.
import { readFileSync } from 'fs';
import { loadSpiralVision } from '../docs/js/spiralvision.js';
const sv = await loadSpiralVision(readFileSync(new URL('../docs/wasm/spiralvision.wasm', import.meta.url)));
const enc = new sv.Encoder(), dec = new sv.Decoder(48000), tape = new sv.Tape(1, 3);
const img = new Uint8ClampedArray(sv.W * sv.H * 4);
for (let i = 0; i < sv.W * sv.H; i++) img.set([255, 0, 0, 255], i * 4);
let last = null;
for (let f = 0; f < 15; f++) { dec.push(tape.process(enc.encode(img))); let fr; while ((fr = dec.pop())) last = fr; }
const c = (120 * 320 + 160) * 4;
console.log('locked', dec.locked, 'speed', dec.speed.toFixed(3), 'rgb', last[c], last[c + 1], last[c + 2]);
if (!dec.locked || last[c] < 180 || last[c + 1] > 70) { console.error('WASM smoke test FAILED'); process.exit(1); }
