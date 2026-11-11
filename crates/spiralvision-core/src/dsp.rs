//! Small DSP building blocks (no external dependencies so the core compiles to WASM).

use std::f32::consts::PI;

/// Windowed (Blackman) type-III Hilbert transformer taps, odd length.
pub fn hilbert_taps(len: usize) -> Vec<f32> {
    assert!(len % 2 == 1);
    let c = (len / 2) as i32;
    (0..len as i32)
        .map(|n| {
            let m = n - c;
            if m % 2 == 0 {
                0.0
            } else {
                let w = 0.42 - 0.5 * (2.0 * PI * n as f32 / (len - 1) as f32).cos()
                    + 0.08 * (4.0 * PI * n as f32 / (len - 1) as f32).cos();
                2.0 / (PI * m as f32) * w
            }
        })
        .collect()
}

/// Normalised Hann smoothing kernel, odd length.
pub fn hann_taps(len: usize) -> Vec<f32> {
    assert!(len % 2 == 1);
    let mut t: Vec<f32> = (0..len)
        .map(|n| 0.5 - 0.5 * (2.0 * PI * (n + 1) as f32 / (len + 1) as f32).cos())
        .collect();
    let s: f32 = t.iter().sum();
    t.iter_mut().for_each(|x| *x /= s);
    t
}

/// Streaming FIR with a circular history.
#[derive(Clone)]
pub struct Fir {
    taps: Vec<f32>,
    hist: Vec<f32>,
    pos: usize,
}

impl Fir {
    pub fn new(taps: Vec<f32>) -> Self {
        let n = taps.len();
        Self {
            taps,
            hist: vec![0.0; n],
            pos: 0,
        }
    }
    pub fn delay(&self) -> usize {
        self.taps.len() / 2
    }
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        let n = self.taps.len();
        self.hist[self.pos] = x;
        let mut acc = 0.0;
        let mut idx = self.pos;
        for &t in &self.taps {
            acc += t * self.hist[idx];
            idx = if idx == 0 { n - 1 } else { idx - 1 };
        }
        self.pos = (self.pos + 1) % n;
        acc
    }
    /// Sample delayed by the filter's group delay (for the in-phase branch).
    #[inline]
    pub fn delayed(&self) -> f32 {
        let n = self.taps.len();
        // last written index is pos-1
        let last = (self.pos + n - 1) % n;
        self.hist[(last + n - self.delay()) % n]
    }
}

/// Streaming resampler (Catmull-Rom) for arbitrary rate ratios. Stereo.
#[derive(Clone)]
pub struct Resampler {
    step: f64,
    pos: f64,
    hist: [(f32, f32); 4],
    // simple 2-pole lowpass applied before decimation
    lp: Option<[Biquad; 2]>,
}

impl Resampler {
    pub fn new(in_rate: f32, out_rate: f32) -> Self {
        let step = in_rate as f64 / out_rate as f64;
        let lp = if in_rate > out_rate * 1.01 {
            let fc = 0.45 * out_rate;
            Some([
                Biquad::lowpass(fc, in_rate, 0.707),
                Biquad::lowpass(fc, in_rate, 0.707),
            ])
        } else {
            None
        };
        Self {
            step,
            pos: 0.0,
            hist: [(0.0, 0.0); 4],
            lp,
        }
    }
    pub fn is_identity(&self) -> bool {
        (self.step - 1.0).abs() < 1e-9
    }
    pub fn set_step(&mut self, step: f64) {
        self.step = step;
    }
    /// Push one input frame, append any produced output frames to `out`.
    pub fn push(&mut self, mut s: (f32, f32), out: &mut Vec<(f32, f32)>) {
        if let Some(lp) = &mut self.lp {
            s = (lp[0].process(s.0), lp[1].process(s.1));
        }
        self.hist = [self.hist[1], self.hist[2], self.hist[3], s];
        // output positions are relative to hist[1]..hist[2]
        while self.pos < 1.0 {
            let t = self.pos as f32;
            let [a, b, c, d] = self.hist;
            out.push((cubic(a.0, b.0, c.0, d.0, t), cubic(a.1, b.1, c.1, d.1, t)));
            self.pos += self.step;
        }
        self.pos -= 1.0;
    }
}

#[inline]
pub fn cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
    let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
    let c = -0.5 * p0 + 0.5 * p2;
    ((a * t + b) * t + c) * t + p1
}

/// RBJ biquad.
#[derive(Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn from(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }
    pub fn lowpass(fc: f32, fs: f32, q: f32) -> Self {
        let w = 2.0 * PI * (fc / fs).min(0.49);
        let (s, c) = w.sin_cos();
        let al = s / (2.0 * q);
        Self::from(
            (1.0 - c) / 2.0,
            1.0 - c,
            (1.0 - c) / 2.0,
            1.0 + al,
            -2.0 * c,
            1.0 - al,
        )
    }
    pub fn highpass(fc: f32, fs: f32, q: f32) -> Self {
        let w = 2.0 * PI * (fc / fs).min(0.49);
        let (s, c) = w.sin_cos();
        let al = s / (2.0 * q);
        Self::from(
            (1.0 + c) / 2.0,
            -(1.0 + c),
            (1.0 + c) / 2.0,
            1.0 + al,
            -2.0 * c,
            1.0 - al,
        )
    }
    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// Tiny deterministic PRNG (xorshift64*), good enough for noise and tests.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1)
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    /// Uniform in [0, 1).
    #[inline]
    pub fn uniform(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// Approximately normal(0, 1).
    #[inline]
    pub fn normal(&mut self) -> f32 {
        let mut s = 0.0;
        for _ in 0..4 {
            s += self.uniform();
        }
        (s - 2.0) * 1.732
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hilbert_response_flat_in_band() {
        let taps = hilbert_taps(crate::decoder::HILBERT_LEN);
        for f in [2500.0f32, 3000.0, 6000.0, 12000.0, 13000.0] {
            let w = 2.0 * PI * f / 48000.0;
            let c = (taps.len() / 2) as f32;
            let (mut re, mut im) = (0.0, 0.0);
            for (n, &t) in taps.iter().enumerate() {
                let ph = w * (n as f32 - c);
                re += t * ph.cos();
                im += t * ph.sin();
            }
            let mag = (re * re + im * im).sqrt();
            assert!((mag - 1.0).abs() < 0.02, "hilbert |H({f})| = {mag}");
        }
    }

    #[test]
    fn resampler_preserves_tone() {
        let mut r = Resampler::new(44100.0, 48000.0);
        let mut out = Vec::new();
        for n in 0..44100 {
            let x = (2.0 * PI * 1000.0 * n as f32 / 44100.0).sin();
            r.push((x, x), &mut out);
        }
        assert!((out.len() as i32 - 48000).abs() < 3);
        // count rising zero crossings -> ~1000
        let zc = out
            .windows(2)
            .filter(|w| w[0].0 <= 0.0 && w[1].0 > 0.0)
            .count();
        assert!((zc as i32 - 1000).abs() <= 2, "zc={zc}");
    }
}
