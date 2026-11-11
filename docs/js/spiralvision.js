// SPIRALVISION-10 // JS bindings for the shared Rust DSP core (WebAssembly).
// The exact same encoder/decoder runs in the desktop app, CLI and here.

export async function loadSpiralVision(url = new URL('../wasm/spiralvision.wasm', import.meta.url)) {
  let instance;
  if (typeof url === 'object' && url.byteLength !== undefined) {
    ({ instance } = await WebAssembly.instantiate(url, {}));
  } else {
    const res = await fetch(url);
    if (WebAssembly.instantiateStreaming && res.headers.get('content-type') === 'application/wasm') {
      ({ instance } = await WebAssembly.instantiateStreaming(res, {}));
    } else {
      ({ instance } = await WebAssembly.instantiate(await res.arrayBuffer(), {}));
    }
  }
  const x = instance.exports;
  const W = x.sv_width(), H = x.sv_height(), N = x.sv_frame_samples(), RATE = x.sv_sample_rate();
  const mem = () => x.memory.buffer;

  class Scratch {
    constructor() { this.ptr = 0; this.bytes = 0; }
    get(bytes) {
      if (bytes > this.bytes) {
        if (this.ptr) x.sv_free(this.ptr, this.bytes);
        this.bytes = Math.max(bytes, this.bytes * 2);
        this.ptr = x.sv_alloc(this.bytes);
      }
      return this.ptr;
    }
    free() { if (this.ptr) x.sv_free(this.ptr, this.bytes); this.ptr = 0; this.bytes = 0; }
  }

  class Encoder {
    constructor() { this.h = x.enc_new(); this.img = new Scratch(); }
    /** rgba: Uint8ClampedArray W*H*4 -> Float32Array N*2 interleaved stereo (copy) */
    encode(rgba) {
      const p = this.img.get(W * H * 4);
      new Uint8Array(mem(), p, W * H * 4).set(rgba);
      const out = x.enc_frame(this.h, p);
      return new Float32Array(mem(), out, N * 2).slice();
    }
    free() { x.enc_free(this.h); this.img.free(); }
  }

  class Decoder {
    constructor(sampleRate = RATE) { this.h = x.dec_new(sampleRate); this.l = new Scratch(); this.r = new Scratch(); }
    /** Feed planar Float32Arrays (WebAudio style). Returns frames waiting. */
    pushPlanar(left, right) {
      const n = left.length;
      const pl = this.l.get(n * 4), pr = this.r.get(n * 4);
      new Float32Array(mem(), pl, n).set(left);
      new Float32Array(mem(), pr, n).set(right);
      return x.dec_push_planar(this.h, pl, pr, n);
    }
    /** Feed interleaved stereo Float32Array. */
    push(interleaved) {
      const n = interleaved.length;
      const p = this.l.get(n * 4);
      new Float32Array(mem(), p, n).set(interleaved);
      return x.dec_push(this.h, p, n / 2);
    }
    /** Oldest frame as a fresh ImageData-compatible Uint8ClampedArray, or null. */
    pop() {
      const p = x.dec_pop(this.h);
      if (!p) return null;
      return new Uint8ClampedArray(mem(), p, W * H * 4).slice();
    }
    set(persistence, saturation) { x.dec_set(this.h, persistence, saturation); }
    get locked() { return !!x.dec_locked(this.h); }
    get speed() { return x.dec_speed(this.h); }
    get framesOut() { return x.dec_frames_out(this.h); }
    free() { x.dec_free(this.h); this.l.free(); this.r.free(); }
  }

  class Tape {
    constructor(wear = 1, seed = 1) { this.h = x.tape_new(wear, seed); this.buf = new Scratch(); }
    setWear(w) { x.tape_set(this.h, w); }
    /** interleaved stereo in -> interleaved stereo out (length may differ slightly) */
    process(interleaved) {
      const n = interleaved.length;
      const p = this.buf.get(n * 4);
      new Float32Array(mem(), p, n).set(interleaved);
      const frames = x.tape_process(this.h, p, n / 2);
      return new Float32Array(mem(), x.tape_out(this.h), frames * 2).slice();
    }
    free() { x.tape_free(this.h); this.buf.free(); }
  }

  return { W, H, N, RATE, Encoder, Decoder, Tape };
}
