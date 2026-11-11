//! Stereo audio -> video frames.
//!
//! Pipeline: (resample to 48 kHz) -> quadrature FM/AM demodulation (Hilbert)
//! -> 1 kHz sync correlator -> flywheel frame clock -> spiral splat renderer.
//!
//! The flywheel guarantees a steady output cadence: exactly one frame is
//! produced per frame period of input audio, whether or not a sync pulse was
//! found. Coherent input locks the flywheel and decodes the picture; any other
//! input (noise, music, a blank tape) free-runs at 10 fps and is rendered
//! through the very same demodulator, giving swirly spiral noise.

use crate::dsp::{hann_taps, hilbert_taps, Fir, Resampler};
use crate::protocol::*;
use std::collections::VecDeque;
use std::f32::consts::TAU;

pub const HILBERT_LEN: usize = 63;
const SMOOTH_LEN: usize = 5;
const SUB: usize = 2;

/// Decoder tuning knobs.
#[derive(Clone, Debug)]
pub struct DecoderParams {
    /// Gaussian splat radius (sigma, pixels) for luma.
    pub luma_sigma: f32,
    /// Gaussian splat radius (sigma, pixels) for chroma.
    pub chroma_sigma: f32,
    /// Phosphor persistence: 0 = none, 0.9 = long trails.
    pub persistence: f32,
    /// Use the in-band calibration chirp to equalise tape gain/HF loss/saturation.
    pub use_calibration: bool,
    /// Chroma saturation multiplier applied after decoding.
    pub saturation: f32,
}

impl Default for DecoderParams {
    fn default() -> Self {
        Self {
            luma_sigma: 2.0,
            chroma_sigma: 4.5,
            persistence: 0.0,
            use_calibration: true,
            saturation: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SyncEvent {
    pos: f64,
    amp: [f32; 2],
    speed: f32,
}

/// Sliding 1 kHz correlator.
struct SyncDetector {
    table: Vec<(f32, f32)>,
    terms: VecDeque<[(f64, f64); 2]>,
    energies: VecDeque<f64>,
    s_all: [(f64, f64); 2],
    s_new: [(f64, f64); 2],
    energy: f64,
    run: Vec<(u64, f64, [f32; 2], f32)>,
    recompute: usize,
}

impl SyncDetector {
    fn new() -> Self {
        let per = (SAMPLE_RATE as f32 / SYNC_FREQ) as usize;
        let table = (0..per)
            .map(|n| {
                let ph = TAU * n as f32 / per as f32;
                (ph.cos(), -ph.sin())
            })
            .collect();
        Self {
            table,
            terms: VecDeque::with_capacity(SYNC_LEN + 1),
            energies: VecDeque::with_capacity(SYNC_LEN + 1),
            s_all: [(0.0, 0.0); 2],
            s_new: [(0.0, 0.0); 2],
            energy: 0.0,
            run: Vec::new(),
            recompute: 0,
        }
    }

    fn push(&mut self, n: u64, l: f32, r: f32) -> Option<SyncEvent> {
        let (c, s) = self.table[(n % self.table.len() as u64) as usize];
        let t = [
            ((l * c) as f64, (l * s) as f64),
            ((r * c) as f64, (r * s) as f64),
        ];
        let e = (l * l + r * r) as f64;
        for ch in 0..2 {
            self.s_all[ch].0 += t[ch].0;
            self.s_all[ch].1 += t[ch].1;
            self.s_new[ch].0 += t[ch].0;
            self.s_new[ch].1 += t[ch].1;
        }
        self.energy += e;
        self.terms.push_back(t);
        self.energies.push_back(e);
        let half = SYNC_LEN / 2;
        if self.terms.len() > half {
            let o = self.terms[self.terms.len() - 1 - half];
            for ch in 0..2 {
                self.s_new[ch].0 -= o[ch].0;
                self.s_new[ch].1 -= o[ch].1;
            }
        }
        if self.terms.len() > SYNC_LEN {
            let o = self.terms.pop_front().unwrap();
            let oe = self.energies.pop_front().unwrap();
            for ch in 0..2 {
                self.s_all[ch].0 -= o[ch].0;
                self.s_all[ch].1 -= o[ch].1;
            }
            self.energy -= oe;
        }
        self.recompute += 1;
        if self.recompute >= 4096 {
            self.recompute = 0;
            self.resum();
        }
        if self.terms.len() < SYNC_LEN {
            return None;
        }

        let p_l = self.s_all[0].0.powi(2) + self.s_all[0].1.powi(2);
        let p_r = self.s_all[1].0.powi(2) + self.s_all[1].1.powi(2);
        let p = p_l + p_r;
        let ratio = 2.0 * p / (SYNC_LEN as f64 * self.energy.max(1e-12));
        let loud = self.energy / SYNC_LEN as f64 > 2e-4;
        if ratio > 0.55 && loud {
            let amp = [
                (2.0 * p_l.sqrt() / SYNC_LEN as f64) as f32,
                (2.0 * p_r.sqrt() / SYNC_LEN as f64) as f32,
            ];
            // frequency from phase advance between the two half windows
            let mut cr = 0.0;
            let mut ci = 0.0;
            for ch in 0..2 {
                let (nr, ni) = self.s_new[ch];
                let (or_, oi) = (self.s_all[ch].0 - nr, self.s_all[ch].1 - ni);
                cr += nr * or_ + ni * oi;
                ci += ni * or_ - nr * oi;
            }
            let dphi = ci.atan2(cr) as f32;
            let f = SYNC_FREQ + dphi * SAMPLE_RATE as f32 / (TAU * half as f32);
            self.run.push((n, p, amp, f / SYNC_FREQ));
            None
        } else if !self.run.is_empty() {
            let run = std::mem::take(&mut self.run);
            if run.len() < 48 {
                return None;
            }
            let pmax = run.iter().map(|x| x.1).fold(0.0, f64::max);
            let (mut wsum, mut nsum) = (0.0, 0.0);
            let mut best = run[0];
            for x in &run {
                if x.1 > 0.85 * pmax {
                    let w = x.1 - 0.85 * pmax;
                    wsum += w;
                    nsum += w * x.0 as f64;
                }
                if x.1 >= best.1 {
                    best = *x;
                }
            }
            let n_peak = nsum / wsum.max(1e-12);
            let speed = best.3.clamp(0.9, 1.1);
            let ls = SYNC_LEN as f64 / speed as f64;
            let pos = n_peak - (SYNC_LEN as f64 - 1.0) / 2.0 - (ls - 1.0) / 2.0;
            Some(SyncEvent {
                pos,
                amp: best.2,
                speed,
            })
        } else {
            None
        }
    }

    fn resum(&mut self) {
        self.s_all = [(0.0, 0.0); 2];
        self.s_new = [(0.0, 0.0); 2];
        self.energy = self.energies.iter().sum();
        let len = self.terms.len();
        let half = SYNC_LEN / 2;
        for (i, t) in self.terms.iter().enumerate() {
            for ch in 0..2 {
                self.s_all[ch].0 += t[ch].0;
                self.s_all[ch].1 += t[ch].1;
                if i + half >= len {
                    self.s_new[ch].0 += t[ch].0;
                    self.s_new[ch].1 += t[ch].1;
                }
            }
        }
    }
}

/// Per-channel quadrature demodulator.
struct Demod {
    hil: Fir,
    prev: (f32, f32),
    sm_f: Fir,
    sm_a: Fir,
}

impl Demod {
    fn new() -> Self {
        Self {
            hil: Fir::new(hilbert_taps(HILBERT_LEN)),
            prev: (0.0, 0.0),
            sm_f: Fir::new(hann_taps(SMOOTH_LEN)),
            sm_a: Fir::new(hann_taps(SMOOTH_LEN)),
        }
    }
    #[inline]
    fn push(&mut self, x: f32) -> (f32, f32) {
        let q = self.hil.push(x);
        let i = self.hil.delayed();
        let (pi, pq) = self.prev;
        // z * conj(prev)
        let re = i * pi + q * pq;
        let im = q * pi - i * pq;
        self.prev = (i, q);
        let f = im.atan2(re) * SAMPLE_RATE as f32 / TAU;
        let a = (i * i + q * q).sqrt();
        (self.sm_f.push(f), self.sm_a.push(a))
    }
}

const NB: usize = 8;
const LUT_N: usize = 64;
const LUT_F0: f32 = 2_000.0;
const LUT_F1: f32 = 14_000.0;

#[inline]
fn bin_of(f: f32) -> usize {
    (((f - CAL_F_LO) / (CAL_F_HI - CAL_F_LO)) * NB as f32).clamp(0.0, (NB - 1) as f32) as usize
}

/// Total latency (in samples) between input and demodulated amplitude output.
const DEMOD_DELAY: usize = HILBERT_LEN / 2 + SMOOTH_LEN / 2;

pub struct Decoder {
    pub params: DecoderParams,
    resampler: Option<Resampler>,
    rs_buf: Vec<(f32, f32)>,
    n: u64,
    sync: SyncDetector,
    demod: [Demod; 2],
    // demodulated stream; element k is abs time `base + k`
    base: u64,
    freq: [Vec<f32>; 2],
    amp: [Vec<f32>; 2],
    // flywheel
    next_start: f64,
    period: f64,
    lock: u32,
    last_sync: Option<f64>,
    unlocked_frames: u32,
    syncs: VecDeque<SyncEvent>,
    // calibration
    g1: [f32; 2],
    cal_hi: [[f32; NB]; 2],
    cal_lo: [[f32; NB]; 2],
    lut: [Vec<(f32, f32)>; 2],
    cal_frames: u32,
    // render
    coords: Vec<((f32, f32), (f32, f32))>,
    acc: Accum,
    prev_frame: Vec<f32>,
    /// Decoded frames, RGB, W * H * 3, oldest first.
    pub frames: VecDeque<Vec<u8>>,
    /// Total frames produced since creation.
    pub frames_out: u64,
    /// Frames rendered while locked to a sync.
    pub frames_locked: u64,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self::with_rate(SAMPLE_RATE as f32)
    }

    /// Create a decoder for input at `sample_rate` Hz.
    pub fn with_rate(sample_rate: f32) -> Self {
        let resampler = Resampler::new(sample_rate, SAMPLE_RATE as f32);
        let resampler = if resampler.is_identity() {
            None
        } else {
            Some(resampler)
        };
        let n_sub = DATA_LEN * SUB;
        let coords = (0..n_sub)
            .map(|k| spiral_point(progress((k as f32 + 0.5) / SUB as f32)))
            .collect();
        Self {
            params: DecoderParams::default(),
            resampler,
            rs_buf: Vec::new(),
            n: 0,
            sync: SyncDetector::new(),
            demod: [Demod::new(), Demod::new()],
            base: 0,
            freq: [Vec::new(), Vec::new()],
            amp: [Vec::new(), Vec::new()],
            next_start: 0.0,
            period: N as f64,
            lock: 0,
            last_sync: None,
            unlocked_frames: 0,
            syncs: VecDeque::new(),
            g1: [1.0; 2],
            cal_hi: [[1.0; NB]; 2],
            cal_lo: [[CAL_LOW_AMP; NB]; 2],
            lut: [vec![(1.0, 1.0); LUT_N], vec![(1.0, 1.0); LUT_N]],
            cal_frames: 0,
            coords,
            acc: Accum::new(),
            prev_frame: vec![0.0; W * H * 3],
            frames: VecDeque::new(),
            frames_out: 0,
            frames_locked: 0,
        }
    }

    /// True if the flywheel is currently locked to sync pulses.
    pub fn is_locked(&self) -> bool {
        self.lock > 0
    }

    /// Current tape speed estimate (1.0 = nominal).
    pub fn speed(&self) -> f32 {
        (N as f64 / self.period) as f32
    }

    /// Feed interleaved stereo samples.
    pub fn process_interleaved(&mut self, data: &[f32]) {
        for c in data.chunks_exact(2) {
            self.push((c[0], c[1]));
        }
        self.run_scheduler();
    }

    /// Feed stereo sample pairs.
    pub fn process_samples(&mut self, samples: &[(f32, f32)]) {
        for &s in samples {
            self.push(s);
        }
        self.run_scheduler();
    }

    /// Calibration state, for diagnostics.
    pub fn debug(&self) -> String {
        format!(
            "lock={} speed={:.4} g1={:.3?} hi={:.2?} lo={:.2?}",
            self.lock,
            self.speed(),
            self.g1,
            self.cal_hi[0],
            self.cal_lo[0]
        )
    }

    /// End of stream: emit the frame still being received (pads with silence).
    pub fn flush(&mut self) {
        let target = self.frames_out + 1;
        let pad = vec![(0.0f32, 0.0f32); 64];
        let mut guard = 0;
        while self.frames_out < target && guard < 200 {
            for &s in &pad {
                self.push48(s);
            }
            self.run_scheduler();
            guard += 1;
        }
    }

    /// Oldest decoded frame, if any.
    pub fn pop_frame(&mut self) -> Option<Vec<u8>> {
        self.frames.pop_front()
    }

    fn push(&mut self, s: (f32, f32)) {
        if let Some(rs) = &mut self.resampler {
            let mut buf = std::mem::take(&mut self.rs_buf);
            buf.clear();
            rs.push(s, &mut buf);
            for &x in &buf {
                self.push48(x);
            }
            self.rs_buf = buf;
        } else {
            self.push48(s);
        }
    }

    fn push48(&mut self, (l, r): (f32, f32)) {
        let l = if l.is_finite() { l } else { 0.0 };
        let r = if r.is_finite() { r } else { 0.0 };
        if let Some(ev) = self.sync.push(self.n, l, r) {
            self.syncs.push_back(ev);
        }
        let (fl, al) = self.demod[0].push(l);
        let (fr, ar) = self.demod[1].push(r);
        if self.n >= DEMOD_DELAY as u64 {
            self.freq[0].push(fl);
            self.freq[1].push(fr);
            self.amp[0].push(al);
            self.amp[1].push(ar);
        }
        self.n += 1;
        // Avoid unbounded growth when a scheduler run is far away.
        if self.freq[0].len() > 8 * N {
            self.run_scheduler();
        }
    }

    /// Absolute index one past the last demodulated sample.
    fn demod_end(&self) -> f64 {
        self.base as f64 + self.freq[0].len() as f64
    }

    fn run_scheduler(&mut self) {
        loop {
            let end = self.next_start + self.period + 4.0;
            if self.demod_end() < end {
                break;
            }
            self.schedule_frame();
        }
        // drop consumed demod samples
        let keep_from = (self.next_start - 64.0).max(0.0) as u64;
        if keep_from > self.base {
            let k = ((keep_from - self.base) as usize).min(self.freq[0].len());
            for ch in 0..2 {
                self.freq[ch].drain(..k);
                self.amp[ch].drain(..k);
            }
            self.base += k as u64;
        }
    }

    fn schedule_frame(&mut self) {
        let tol = if self.lock > 0 {
            0.06 * self.period
        } else {
            0.02 * self.period
        };
        let mut start = self.next_start;
        let mut matched: Option<SyncEvent> = None;
        let mut early: Option<SyncEvent> = None;
        for ev in &self.syncs {
            let d = ev.pos - self.next_start;
            if d.abs() <= tol {
                if matched.map_or(true, |m| (m.pos - self.next_start).abs() > d.abs()) {
                    matched = Some(*ev);
                }
            } else if d > tol && d < self.period - tol && early.is_none() {
                early = Some(*ev);
            }
        }
        let mut next_override = None;
        if let Some(ev) = matched {
            start = ev.pos;
            if let Some(prev) = self.last_sync {
                let interval = ev.pos - prev;
                let frames = (interval / self.period).round().max(1.0);
                let est = interval / frames;
                if (est / N as f64 - 1.0).abs() < 0.1 {
                    let a = if self.lock >= 3 { 0.25 } else { 0.6 };
                    self.period += a * (est - self.period);
                }
            } else {
                self.period = N as f64 / ev.speed as f64;
            }
            self.last_sync = Some(ev.pos);
            self.lock = (self.lock + 1).min(5);
            self.unlocked_frames = 0;
            let a = 0.5;
            for ch in 0..2 {
                if ev.amp[ch] > 0.02 {
                    self.g1[ch] += a * (ev.amp[ch] - self.g1[ch]);
                }
            }
        } else {
            self.lock = self.lock.saturating_sub(1);
            self.unlocked_frames += 1;
            if self.lock == 0 {
                if let Some(ev) = early {
                    // Re-acquire: render this (partial) frame, restart at the sync.
                    next_override = Some(ev.pos);
                }
            }
            if self.unlocked_frames > 10 {
                // relax calibration towards nominal when nothing is locked
                self.period += 0.1 * (N as f64 - self.period);
                for ch in 0..2 {
                    self.g1[ch] += 0.1 * (1.0 - self.g1[ch]);
                    for b in 0..NB {
                        self.cal_hi[ch][b] += 0.1 * (1.0 - self.cal_hi[ch][b]);
                        self.cal_lo[ch][b] += 0.1 * (CAL_LOW_AMP - self.cal_lo[ch][b]);
                    }
                }
                self.last_sync = None;
                self.cal_frames = 0;
            }
        }

        let locked_frame = matched.is_some();
        if locked_frame && self.params.use_calibration {
            self.measure_calibration(start);
        }
        self.build_lut();

        self.render(start, locked_frame);
        self.next_start = next_override.unwrap_or(start + self.period);
        let cutoff = self.next_start - 0.5 * self.period;
        self.syncs.retain(|e| e.pos > cutoff);
    }

    fn measure_calibration(&mut self, start: f64) {
        let s = self.speed() as f64;
        let alpha = if self.cal_frames < 2 { 1.0 } else { 0.35 };
        self.cal_frames += 1;
        for ch in 0..2 {
            let mut sum = [[0.0f32; NB]; 2];
            let mut cnt = [[0u32; NB]; 2];
            for c in 0..CAL_LEN {
                let half = c / CAL_HALF;
                let cc = c % CAL_HALF;
                if cc < 6 || cc >= CAL_HALF - 6 {
                    continue;
                }
                let t = start + (SYNC_LEN + c) as f64 / s;
                // Bin by the *measured* frequency: channel group delay then
                // affects frequency and amplitude identically.
                let f = self.sample(&self.freq[ch], t + 0.5) / s as f32;
                if !(CAL_F_LO..=CAL_F_HI).contains(&f) {
                    continue;
                }
                let b = bin_of(f);
                sum[half][b] += self.sample(&self.amp[ch], t);
                cnt[half][b] += 1;
            }
            for b in 0..NB {
                if cnt[0][b] > 0 {
                    let m = sum[0][b] / cnt[0][b] as f32;
                    self.cal_hi[ch][b] += alpha * (m - self.cal_hi[ch][b]);
                }
                if cnt[1][b] > 0 {
                    let m = sum[1][b] / cnt[1][b] as f32;
                    self.cal_lo[ch][b] += alpha * (m - self.cal_lo[ch][b]);
                }
            }
        }
    }

    /// Build per-frequency (gain, 1/gamma) tables from the calibration bins.
    fn build_lut(&mut self) {
        for ch in 0..2 {
            for i in 0..LUT_N {
                let f = LUT_F0 + (LUT_F1 - LUT_F0) * i as f32 / (LUT_N - 1) as f32;
                let x = ((f - CAL_F_LO) / (CAL_F_HI - CAL_F_LO) * NB as f32 - 0.5)
                    .clamp(0.0, (NB - 1) as f32);
                let b0 = x.floor() as usize;
                let b1 = (b0 + 1).min(NB - 1);
                let fr = x - b0 as f32;
                let hi = (self.cal_hi[ch][b0] * (1.0 - fr) + self.cal_hi[ch][b1] * fr).max(1e-3);
                let lo = (self.cal_lo[ch][b0] * (1.0 - fr) + self.cal_lo[ch][b1] * fr).max(1e-4);
                let gamma = ((lo / hi).ln() / CAL_LOW_AMP.ln()).clamp(0.4, 1.6);
                self.lut[ch][i] = (hi, 1.0 / gamma);
            }
        }
    }

    /// Undo the channel's gain curve: observed amplitude -> transmitted amplitude.
    #[inline]
    fn equalize(&self, ch: usize, f: f32, a: f32) -> f32 {
        let x =
            ((f - LUT_F0) / (LUT_F1 - LUT_F0) * (LUT_N - 1) as f32).clamp(0.0, (LUT_N - 1) as f32);
        let (g, inv_gamma) = self.lut[ch][x as usize];
        (a.max(0.0) / g).powf(inv_gamma)
    }

    #[inline]
    fn sample(&self, buf: &[f32], t: f64) -> f32 {
        let x = t - self.base as f64;
        if x < 0.0 {
            return buf.first().copied().unwrap_or(0.0);
        }
        let i = x.floor() as usize;
        if i + 1 >= buf.len() {
            return buf.last().copied().unwrap_or(0.0);
        }
        let f = (x - i as f64) as f32;
        buf[i] * (1.0 - f) + buf[i + 1] * f
    }

    fn render(&mut self, start: f64, locked: bool) {
        let s = self.speed();
        let sd = s as f64;
        // Unlocked (noise) mode: no calibration reference exists, so chroma
        // shows loudness relative to this frame's mean -> colourful swirls.
        let mut mean = [0.0f32; 2];
        if !locked {
            let n = self.coords.len() as f32;
            for k in 0..self.coords.len() {
                let t = start + (DATA_START as f64 + (k as f64 + 0.5) / SUB as f64) / sd;
                mean[0] += self.sample(&self.amp[0], t) / n;
                mean[1] += self.sample(&self.amp[1], t) / n;
            }
        }
        self.acc
            .clear(self.params.luma_sigma, self.params.chroma_sigma);
        for k in 0..self.coords.len() {
            let j = DATA_START as f64 + (k as f64 + 0.5) / SUB as f64;
            let t = start + j / sd;
            let ((xa, ya), (xb, yb)) = self.coords[k];
            // freq estimate at index n describes time n - 0.5
            let fl = self.sample(&self.freq[0], t + 0.5) / s;
            let fr = self.sample(&self.freq[1], t + 0.5) / s;
            let al = self.sample(&self.amp[0], t);
            let ar = self.sample(&self.amp[1], t);
            let yl = (fl - F_MIN) / (F_MAX - F_MIN);
            let yr = (fr - F_MIN) / (F_MAX - F_MIN);
            let (u, v) = if locked {
                (
                    (self.equalize(0, fl, al) - A_MIN) / (A_MAX - A_MIN),
                    (self.equalize(1, fr, ar) - A_MIN) / (A_MAX - A_MIN),
                )
            } else {
                let rel =
                    |a: f32, m: f32| 0.5 + 0.48 * (2.0 * ((a + 1e-3) / (m + 1e-3)).ln()).tanh();
                (rel(al, mean[0]), rel(ar, mean[1]))
            };
            self.acc.splat_luma(xa, ya, yl.clamp(-0.1, 1.1));
            self.acc.splat_luma(xb, yb, yr.clamp(-0.1, 1.1));
            if k % 2 == 0 {
                self.acc.splat_chroma(xa, ya, 0, u.clamp(-0.1, 1.1));
                self.acc.splat_chroma(xb, yb, 1, v.clamp(-0.1, 1.1));
            }
        }
        let (y, u, v) = self.acc.resolve();
        let persist = self.params.persistence.clamp(0.0, 0.98);
        let sat = self.params.saturation;
        let mut out = vec![0u8; W * H * 3];
        for i in 0..W * H {
            let uu = 0.5 + (u[i] - 0.5) * sat;
            let vv = 0.5 + (v[i] - 0.5) * sat;
            let (r, g, b) = yuv_to_rgb(y[i], uu, vv);
            for (c, val) in [r, g, b].into_iter().enumerate() {
                let p = &mut self.prev_frame[i * 3 + c];
                *p = *p * persist + val as f32 * (1.0 - persist);
                out[i * 3 + c] = (*p + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
        if self.frames.len() >= 32 {
            self.frames.pop_front();
        }
        self.frames.push_back(out);
        self.frames_out += 1;
        if locked {
            self.frames_locked += 1;
        }
    }
}

/// Gaussian splat accumulator with hole filling.
struct Accum {
    sig: (f32, f32),
    y: Vec<f32>,
    yw: Vec<f32>,
    c: [Vec<f32>; 2],
    cw: [Vec<f32>; 2],
}

impl Accum {
    fn new() -> Self {
        Self {
            sig: (1.0, 1.0),
            y: vec![0.0; W * H],
            yw: vec![0.0; W * H],
            c: [vec![0.0; CW * CH], vec![0.0; CW * CH]],
            cw: [vec![0.0; CW * CH], vec![0.0; CW * CH]],
        }
    }
    fn clear(&mut self, ls: f32, cs: f32) {
        self.sig = (ls.max(0.3), cs.max(0.3));
        self.y.fill(0.0);
        self.yw.fill(0.0);
        for ch in 0..2 {
            self.c[ch].fill(0.0);
            self.cw[ch].fill(0.0);
        }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn splat(
        buf: &mut [f32],
        wbuf: &mut [f32],
        w: usize,
        h: usize,
        x: f32,
        y: f32,
        val: f32,
        sigma: f32,
    ) {
        let r = (sigma * 2.0).ceil() as i32;
        let cx = x.round() as i32;
        let cy = y.round() as i32;
        let inv = -1.0 / (2.0 * sigma * sigma);
        let mut wx = [0.0f32; 33];
        let mut wy = [0.0f32; 33];
        let r = r.min(16);
        for i in -r..=r {
            let dx = (cx + i) as f32 - x;
            let dy = (cy + i) as f32 - y;
            wx[(i + r) as usize] = (dx * dx * inv).exp();
            wy[(i + r) as usize] = (dy * dy * inv).exp();
        }
        for j in -r..=r {
            let py = cy + j;
            if py < 0 || py >= h as i32 {
                continue;
            }
            let wyv = wy[(j + r) as usize];
            let row = py as usize * w;
            for i in -r..=r {
                let px = cx + i;
                if px < 0 || px >= w as i32 {
                    continue;
                }
                let w = wyv * wx[(i + r) as usize];
                buf[row + px as usize] += w * val;
                wbuf[row + px as usize] += w;
            }
        }
    }
    fn splat_luma(&mut self, x: f32, y: f32, v: f32) {
        Self::splat(&mut self.y, &mut self.yw, W, H, x, y, v, self.sig.0);
    }
    fn splat_chroma(&mut self, x: f32, y: f32, ch: usize, v: f32) {
        // chroma lives on a half-resolution grid
        Self::splat(
            &mut self.c[ch],
            &mut self.cw[ch],
            CW,
            CH,
            x * 0.5,
            y * 0.5,
            v,
            self.sig.1 * 0.5,
        );
    }

    fn resolve(&mut self) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let y = normalize_fill(&self.y, &self.yw, W, H, 0.0);
        let u = upsample2(&normalize_fill(&self.c[0], &self.cw[0], CW, CH, 0.5));
        let v = upsample2(&normalize_fill(&self.c[1], &self.cw[1], CW, CH, 0.5));
        (y, u, v)
    }
}

/// value/weight with push-pull filling of uncovered pixels.
const CW: usize = W / 2;
const CH: usize = H / 2;

/// Bilinear 2x upsample from the half-res chroma grid.
fn upsample2(src: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; W * H];
    for y in 0..H {
        let sy = ((y as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (CH - 1) as f32);
        let y0 = sy as usize;
        let y1 = (y0 + 1).min(CH - 1);
        let fy = sy - y0 as f32;
        for x in 0..W {
            let sx = ((x as f32 + 0.5) * 0.5 - 0.5).clamp(0.0, (CW - 1) as f32);
            let x0 = sx as usize;
            let x1 = (x0 + 1).min(CW - 1);
            let fx = sx - x0 as f32;
            let a = src[y0 * CW + x0] * (1.0 - fx) + src[y0 * CW + x1] * fx;
            let b = src[y1 * CW + x0] * (1.0 - fx) + src[y1 * CW + x1] * fx;
            out[y * W + x] = a * (1.0 - fy) + b * fy;
        }
    }
    out
}

fn normalize_fill(val: &[f32], wt: &[f32], w0: usize, h0: usize, default: f32) -> Vec<f32> {
    // pyramid of (sum, weight)
    let mut levels: Vec<(usize, usize, Vec<f32>, Vec<f32>)> =
        vec![(w0, h0, val.to_vec(), wt.to_vec())];
    loop {
        let (w, h, v, wt) = levels.last().unwrap();
        if *w <= 1 && *h <= 1 {
            break;
        }
        let (nw, nh) = ((w + 1) / 2, (h + 1) / 2);
        let mut nv = vec![0.0; nw * nh];
        let mut nwt = vec![0.0; nw * nh];
        for y in 0..*h {
            for x in 0..*w {
                let i = (y / 2) * nw + x / 2;
                // clamp weights so dense areas don't dominate coarse levels
                let ww = wt[y * w + x].min(1.0);
                let s = if wt[y * w + x] > 0.0 {
                    v[y * w + x] / wt[y * w + x]
                } else {
                    0.0
                };
                nv[i] += s * ww;
                nwt[i] += ww;
            }
        }
        levels.push((nw, nh, nv, nwt));
    }
    // pull: from coarse to fine
    let mut coarse: Vec<f32> = {
        let (_, _, v, w) = levels.last().unwrap();
        vec![if w[0] > 0.0 { v[0] / w[0] } else { default }]
    };
    let mut cw = 1usize;
    for li in (0..levels.len() - 1).rev() {
        let (w, h, v, wt) = &levels[li];
        let mut out = vec![0.0; w * h];
        for y in 0..*h {
            for x in 0..*w {
                let i = y * w + x;
                let up = coarse[(y / 2) * cw + x / 2];
                let ww = wt[i];
                out[i] = if ww <= 0.0 {
                    up
                } else {
                    let own = v[i] / ww;
                    let a = ww.min(1.0);
                    own * a + up * (1.0 - a)
                };
            }
        }
        coarse = out;
        cw = *w;
    }
    coarse
}
