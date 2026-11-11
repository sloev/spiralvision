//! Compact cassette channel simulator: speed error, wow & flutter, HF loss,
//! azimuth skew, saturation, hiss, hum and dropouts. Used for tests, the demo
//! videos and the "TAPE WEAR" knob in the apps.

use crate::dsp::{cubic, Biquad, Rng};
use std::f32::consts::TAU;

#[derive(Clone, Debug)]
pub struct TapeParams {
    /// Playback speed relative to recording (1.0 = perfect).
    pub speed: f32,
    /// Peak wow deviation (fraction) at ~0.5 Hz.
    pub wow: f32,
    /// Peak flutter deviation (fraction) at ~9 Hz.
    pub flutter: f32,
    /// -3 dB point of the head/tape HF loss (Hz).
    pub hf_cutoff: f32,
    /// Output gain.
    pub gain: f32,
    /// Right channel gain relative to left.
    pub balance: f32,
    /// Right-channel delay in samples (head azimuth error).
    pub azimuth: f32,
    /// Saturation drive (0 = clean).
    pub drive: f32,
    /// Hiss RMS level.
    pub hiss: f32,
    /// 50 Hz hum level.
    pub hum: f32,
    /// Average dropouts per second.
    pub dropouts: f32,
}

impl TapeParams {
    /// A single "wear" knob: 0 = perfect wire, 1 = a well-loved ferric tape
    /// in a cheap deck, 2 = gas-station bargain bin.
    pub fn wear(amount: f32) -> Self {
        let a = amount.max(0.0);
        Self {
            speed: 1.0 - 0.012 * a,
            wow: 0.0015 * a,
            flutter: 0.0008 * a,
            hf_cutoff: if a > 0.0 {
                16000.0 / (1.0 + 0.45 * a)
            } else {
                22000.0
            },
            gain: 1.0 - 0.25 * a.min(2.0),
            balance: 1.0 - 0.08 * a,
            azimuth: 0.35 * a,
            drive: 0.3 * a,
            hiss: 0.008 * a,
            hum: 0.004 * a,
            dropouts: 0.15 * a,
        }
    }
}

pub struct TapeSim {
    pub p: TapeParams,
    sr: f32,
    rng: Rng,
    hist: Vec<(f32, f32)>,
    hist_base: u64,
    n_in: u64,
    pos: f64,
    t: f64,
    lp: [Biquad; 4],
    hp: [Biquad; 2],
    hiss_lp: [Biquad; 2],
    drop_left: u32,
    drop_depth: f32,
    flutter_noise: f32,
}

impl TapeSim {
    pub fn new(p: TapeParams, sample_rate: f32, seed: u64) -> Self {
        let lp = |_: usize| Biquad::lowpass(p.hf_cutoff, sample_rate, 0.6);
        Self {
            sr: sample_rate,
            rng: Rng::new(seed),
            hist: Vec::new(),
            hist_base: 0,
            n_in: 0,
            pos: 1.0,
            t: 0.0,
            lp: [lp(0), lp(1), lp(2), lp(3)],
            hp: [
                Biquad::highpass(30.0, sample_rate, 0.7),
                Biquad::highpass(30.0, sample_rate, 0.7),
            ],
            hiss_lp: [
                Biquad::lowpass(9000.0, sample_rate, 0.7),
                Biquad::lowpass(9000.0, sample_rate, 0.7),
            ],
            drop_left: 0,
            drop_depth: 1.0,
            flutter_noise: 0.0,
            p,
        }
    }

    /// Process a block. Output length differs from input when speed != 1.
    pub fn process(&mut self, input: &[(f32, f32)]) -> Vec<(f32, f32)> {
        self.hist.extend_from_slice(input);
        self.n_in += input.len() as u64;
        let mut out = Vec::with_capacity((input.len() as f32 / self.p.speed.max(0.5)) as usize + 4);
        loop {
            // read position (absolute input index) with azimuth on R
            let need = self.pos + self.p.azimuth as f64 + 3.0;
            if need >= self.n_in as f64 {
                break;
            }
            let l = self.read(self.pos, 0);
            let r = self.read(self.pos - self.p.azimuth as f64, 1);
            out.push(self.post(l, r));
            self.t += 1.0 / self.sr as f64;
            self.flutter_noise = 0.995 * self.flutter_noise + 0.005 * self.rng.normal();
            let t = self.t as f32;
            let dev = self.p.wow * (TAU * 0.55 * t).sin()
                + self.p.flutter * (TAU * 9.3 * t).sin()
                + self.p.flutter * 2.0 * self.flutter_noise;
            self.pos += (self.p.speed * (1.0 + dev)) as f64;
        }
        // trim history
        let keep_from = (self.pos.floor() as i64 - 8).max(0) as u64;
        if keep_from > self.hist_base {
            let k = ((keep_from - self.hist_base) as usize).min(self.hist.len());
            self.hist.drain(..k);
            self.hist_base += k as u64;
        }
        out
    }

    fn read(&self, pos: f64, ch: usize) -> f32 {
        let i = pos.floor() as i64;
        let f = (pos - i as f64) as f32;
        let g = |k: i64| -> f32 {
            let idx = k - self.hist_base as i64;
            if idx < 0 || idx as usize >= self.hist.len() {
                0.0
            } else {
                let s = self.hist[idx as usize];
                if ch == 0 {
                    s.0
                } else {
                    s.1
                }
            }
        };
        cubic(g(i - 1), g(i), g(i + 1), g(i + 2), f)
    }

    fn post(&mut self, l: f32, r: f32) -> (f32, f32) {
        let p = &self.p;
        // record: saturation
        let sat = |x: f32| {
            if p.drive > 0.0 {
                (x * (1.0 + p.drive)).tanh() / (1.0 + p.drive).tanh()
            } else {
                x
            }
        };
        let mut l = sat(l);
        let mut r = sat(r);
        // dropouts
        if self.drop_left == 0 && self.rng.uniform() < p.dropouts / self.sr {
            self.drop_left = (self.sr * (0.004 + 0.02 * self.rng.uniform())) as u32;
            self.drop_depth = 0.1 + 0.5 * self.rng.uniform();
        }
        if self.drop_left > 0 {
            self.drop_left -= 1;
            l *= self.drop_depth;
            r *= self.drop_depth;
        }
        // playback: HF loss (two 2-pole sections per channel) and low cut
        l = self.lp[0].process(l);
        l = self.lp[1].process(l);
        r = self.lp[2].process(r);
        r = self.lp[3].process(r);
        l = self.hp[0].process(l);
        r = self.hp[1].process(r);
        let hum = p.hum * (TAU * 50.0 * self.t as f32).sin();
        let hl = self.hiss_lp[0].process(self.rng.normal()) * p.hiss * 1.5;
        let hr = self.hiss_lp[1].process(self.rng.normal()) * p.hiss * 1.5;
        ((l * p.gain + hl + hum), (r * p.gain * p.balance + hr + hum))
    }
}
