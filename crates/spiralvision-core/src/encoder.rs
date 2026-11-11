//! Video frame -> stereo audio.

use crate::image::{gaussian_blur, Planes};
use crate::protocol::*;
use std::f64::consts::TAU;

/// Pre-filter strength (Gaussian sigma in pixels) for luma and chroma. Matches
/// what the spiral can actually carry so the encoder does not alias.
pub const LUMA_PREBLUR: f32 = 1.2;
pub const CHROMA_PREBLUR: f32 = 3.0;

pub struct Encoder {
    phi: [f64; 2],
    coords: Vec<((f32, f32), (f32, f32))>,
    planes: Planes,
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Encoder {
    pub fn new() -> Self {
        let coords = (0..DATA_LEN)
            .map(|j| spiral_point(progress(j as f32 + 0.5)))
            .collect();
        Self {
            phi: [0.0; 2],
            coords,
            planes: Planes::new(W, H),
        }
    }

    /// Encode one W x H frame with `bpp` bytes per pixel (3 = RGB, 4 = RGBA).
    /// Returns `N` stereo samples.
    pub fn encode_frame_bpp(&mut self, img: &[u8], bpp: usize) -> Vec<(f32, f32)> {
        let mut out = Vec::with_capacity(N);
        self.encode_into(img, bpp, &mut out);
        out
    }

    /// Encode one RGB frame (W * H * 3 bytes).
    pub fn encode_frame(&mut self, rgb: &[u8]) -> Vec<(f32, f32)> {
        self.encode_frame_bpp(rgb, 3)
    }

    pub fn encode_into(&mut self, img: &[u8], bpp: usize, out: &mut Vec<(f32, f32)>) {
        assert!(img.len() >= W * H * bpp, "frame must be {W}x{H}");
        self.planes.load(img, bpp);
        gaussian_blur(&mut self.planes.y, W, H, crate::tune("PY", LUMA_PREBLUR));
        gaussian_blur(&mut self.planes.u, W, H, crate::tune("PC", CHROMA_PREBLUR));
        gaussian_blur(&mut self.planes.v, W, H, crate::tune("PC", CHROMA_PREBLUR));

        let dt = 1.0 / SAMPLE_RATE as f64;
        for _ in 0..SYNC_LEN {
            out.push(self.tone(SYNC_FREQ as f64 * dt, 1.0, 1.0));
        }
        for c in 0..CAL_LEN {
            let (f, a) = cal_point(c);
            out.push(self.tone(f as f64 * dt, a, a));
        }
        let pick = |j: usize, pl: &Planes, coords: &[((f32, f32), (f32, f32))]| {
            let ((xa, ya), (xb, yb)) = coords[j];
            (
                pl.sample_y(xa, ya),
                pl.sample_y(xb, yb),
                pl.sample_u(xa, ya),
                pl.sample_v(xb, yb),
            )
        };
        for i in 0..(DATA_LEN + 2 * GUARD) {
            let j = i.saturating_sub(GUARD).min(DATA_LEN - 1);
            let (ly, ry, u, v) = pick(j, &self.planes, &self.coords);
            let fl = F_MIN + ly * (F_MAX - F_MIN);
            let fr = F_MIN + ry * (F_MAX - F_MIN);
            self.phi[0] = (self.phi[0] + TAU * fl as f64 * dt) % TAU;
            self.phi[1] = (self.phi[1] + TAU * fr as f64 * dt) % TAU;
            let el = A_MIN + u * (A_MAX - A_MIN);
            let er = A_MIN + v * (A_MAX - A_MIN);
            out.push((el * self.phi[0].sin() as f32, er * self.phi[1].sin() as f32));
        }
        debug_assert_eq!(out.len() % N, 0);
    }

    fn tone(&mut self, cycles: f64, al: f32, ar: f32) -> (f32, f32) {
        // Phases stay continuous per channel (no clicks on tape).
        self.phi[0] = (self.phi[0] + TAU * cycles) % TAU;
        self.phi[1] = (self.phi[1] + TAU * cycles) % TAU;
        (al * self.phi[0].sin() as f32, ar * self.phi[1].sin() as f32)
    }
}
