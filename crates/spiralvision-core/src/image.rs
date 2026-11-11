//! Image plane helpers.

use crate::protocol::rgb_to_yuv;

pub struct Planes {
    pub w: usize,
    pub h: usize,
    pub y: Vec<f32>,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
}

impl Planes {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            y: vec![0.0; w * h],
            u: vec![0.5; w * h],
            v: vec![0.5; w * h],
        }
    }

    pub fn load(&mut self, img: &[u8], bpp: usize) {
        for i in 0..self.w * self.h {
            let (y, u, v) = rgb_to_yuv(img[i * bpp], img[i * bpp + 1], img[i * bpp + 2]);
            self.y[i] = y;
            self.u[i] = u;
            self.v[i] = v;
        }
    }

    #[inline]
    fn bilinear(&self, p: &[f32], x: f32, y: f32) -> f32 {
        let x0 = (x.floor() as usize).min(self.w - 1);
        let y0 = (y.floor() as usize).min(self.h - 1);
        let x1 = (x0 + 1).min(self.w - 1);
        let y1 = (y0 + 1).min(self.h - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let a = p[y0 * self.w + x0] * (1.0 - fx) + p[y0 * self.w + x1] * fx;
        let b = p[y1 * self.w + x0] * (1.0 - fx) + p[y1 * self.w + x1] * fx;
        a * (1.0 - fy) + b * fy
    }
    pub fn sample_y(&self, x: f32, y: f32) -> f32 {
        self.bilinear(&self.y, x, y)
    }
    pub fn sample_u(&self, x: f32, y: f32) -> f32 {
        self.bilinear(&self.u, x, y)
    }
    pub fn sample_v(&self, x: f32, y: f32) -> f32 {
        self.bilinear(&self.v, x, y)
    }
}

/// Separable Gaussian blur in place, clamped edges.
pub fn gaussian_blur(p: &mut [f32], w: usize, h: usize, sigma: f32) {
    if sigma <= 0.0 {
        return;
    }
    let r = (sigma * 3.0).ceil() as i32;
    let mut k: Vec<f32> = (-r..=r)
        .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let s: f32 = k.iter().sum();
    k.iter_mut().for_each(|x| *x /= s);
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        let row = &p[y * w..(y + 1) * w];
        for x in 0..w {
            let mut acc = 0.0;
            for (i, &kv) in k.iter().enumerate() {
                let xx = (x as i32 + i as i32 - r).clamp(0, w as i32 - 1) as usize;
                acc += kv * row[xx];
            }
            tmp[y * w + x] = acc;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for (i, &kv) in k.iter().enumerate() {
                let yy = (y as i32 + i as i32 - r).clamp(0, h as i32 - 1) as usize;
                acc += kv * tmp[yy * w + x];
            }
            p[y * w + x] = acc;
        }
    }
}

/// Peak signal-to-noise ratio between two RGB(A) images, in dB.
pub fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let mse: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| {
            let d = x as f64 - y as f64;
            d * d
        })
        .sum::<f64>()
        / a.len() as f64;
    if mse == 0.0 {
        99.0
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}
