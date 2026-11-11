// SPIRALVISION-10 AudioWorklet: plays queued tape audio and captures line-in.
// All DSP happens in WebAssembly on the main thread; this just moves samples.
class SpiralVisionIO extends AudioWorkletProcessor {
  constructor() {
    super();
    this.queue = [];      // interleaved Float32Array chunks to play
    this.off = 0;
    this.capture = false;
    this.thru = false;    // pass the input straight to the output (monitoring)
    this.capL = new Float32Array(2048);
    this.capR = new Float32Array(2048);
    this.capN = 0;
    this.port.onmessage = (e) => {
      const d = e.data;
      if (d.play) this.queue.push(d.play);
      if (d.clear) { this.queue = []; this.off = 0; }
      if ('capture' in d) this.capture = d.capture;
      if ('thru' in d) this.thru = d.thru;
    };
  }

  process(inputs, outputs) {
    const inp = inputs[0];
    const out = outputs[0];
    const n = out[0] ? out[0].length : 128;

    if (this.capture && inp && inp.length) {
      const l = inp[0], r = inp[1] || inp[0];
      for (let i = 0; i < l.length; i++) {
        this.capL[this.capN] = l[i];
        this.capR[this.capN] = r[i];
        if (++this.capN === this.capL.length) {
          this.port.postMessage({ cap: [this.capL, this.capR] }, [this.capL.buffer, this.capR.buffer]);
          this.capL = new Float32Array(2048);
          this.capR = new Float32Array(2048);
          this.capN = 0;
        }
      }
    }

    if (!out || !out.length) return true;
    const oL = out[0], oR = out[1] || out[0];
    if (this.thru && inp && inp.length) {
      oL.set(inp[0]);
      if (out[1]) oR.set(inp[1] || inp[0]);
      return true;
    }
    for (let i = 0; i < n; i++) {
      let l = 0, r = 0;
      while (this.queue.length) {
        const q = this.queue[0];
        if (this.off < q.length) { l = q[this.off]; r = q[this.off + 1]; this.off += 2; break; }
        this.queue.shift(); this.off = 0;
      }
      oL[i] = l;
      if (out[1]) oR[i] = r;
    }
    return true;
  }
}
registerProcessor('spiralvision-io', SpiralVisionIO);
