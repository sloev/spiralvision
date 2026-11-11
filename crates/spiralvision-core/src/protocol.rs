//! SpiralVision-10 rev B protocol constants, colour conversion and spiral geometry.
//!
//! See `spec.md` in the repository root for the full description.

/// Frame width in pixels.
pub const W: usize = 320;
/// Frame height in pixels.
pub const H: usize = 240;
/// Frames per second.
pub const FPS: u32 = 10;
/// Protocol sample rate. Other rates are resampled by the decoder.
pub const SAMPLE_RATE: u32 = 48_000;
/// Samples per frame (100 ms).
pub const N: usize = (SAMPLE_RATE / FPS) as usize;

/// Sync pulse: 1 kHz, full amplitude, both channels.
pub const SYNC_FREQ: f32 = 1_000.0;
pub const SYNC_LEN: usize = 240;
/// Calibration chirp: an up-sweep at full amplitude followed by a down-sweep
/// at `CAL_LOW_AMP`. The decoder measures the tape's gain at every frequency
/// and level from it, so AM chroma survives HF loss, gain error and
/// saturation.
pub const CAL_LEN: usize = 192;
pub const CAL_HALF: usize = CAL_LEN / 2;
pub const CAL_F_LO: f32 = 2_500.0;
pub const CAL_F_HI: f32 = 13_000.0;
pub const CAL_LOW_AMP: f32 = 0.35;
/// Guard samples before and after the picture data, repeating the first/last
/// picture value so filter transients never reach the image.
pub const GUARD: usize = 24;
/// First sample carrying picture data.
pub const DATA_START: usize = SYNC_LEN + CAL_LEN + GUARD;
/// Picture samples per channel per frame.
pub const DATA_LEN: usize = N - DATA_START - GUARD;

/// Frequency and amplitude of calibration sample `c` in [0, CAL_LEN).
#[inline]
pub fn cal_point(c: usize) -> (f32, f32) {
    if c < CAL_HALF {
        let t = (c as f32 + 0.5) / CAL_HALF as f32;
        (CAL_F_LO + (CAL_F_HI - CAL_F_LO) * t, 1.0)
    } else {
        let t = ((c - CAL_HALF) as f32 + 0.5) / CAL_HALF as f32;
        (CAL_F_HI - (CAL_F_HI - CAL_F_LO) * t, CAL_LOW_AMP)
    }
}

/// FM luma range.
pub const F_MIN: f32 = 3_000.0;
pub const F_MAX: f32 = 12_000.0;
/// AM chroma range. Never below `A_MIN` so the FM carrier survives.
pub const A_MIN: f32 = 0.1;
pub const A_MAX: f32 = 1.0;

/// Spiral winding: total angle swept by each arm, in radians.
pub const K_WIND: f32 = 80.0;
/// Superellipse exponent at the outer edge (2 = circle, large = rectangle).
pub const N_EDGE: f32 = 14.0;
/// Outer radius overshoot so the rounded superellipse reaches the corners.
pub const EDGE_SCALE: f32 = 1.03;

/// Full-range BT.601 RGB -> YUV, all components in [0, 1] (U, V centred on 0.5).
#[inline]
pub fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let u = -0.14713 * r - 0.28886 * g + 0.436 * b + 0.5;
    let v = 0.615 * r - 0.51499 * g - 0.10001 * b + 0.5;
    (y.clamp(0.0, 1.0), u.clamp(0.0, 1.0), v.clamp(0.0, 1.0))
}

#[inline]
pub fn yuv_to_rgb(y: f32, u: f32, v: f32) -> (u8, u8, u8) {
    let y = y.clamp(0.0, 1.0);
    let u = u.clamp(0.0, 1.0) - 0.5;
    let v = v.clamp(0.0, 1.0) - 0.5;
    let r = (y + 1.13983 * v) * 255.0;
    let g = (y - 0.39465 * u - 0.58060 * v) * 255.0;
    let b = (y + 2.03211 * u) * 255.0;
    (
        (r + 0.5).clamp(0.0, 255.0) as u8,
        (g + 0.5).clamp(0.0, 255.0) as u8,
        (b + 0.5).clamp(0.0, 255.0) as u8,
    )
}

/// Pixel coordinates of both spiral arms for normalised progress `p` in [0, 1].
///
/// The arms are Archimedean (constant line spacing, constant sample density)
/// and morph from a circle at the centre to a 4:3 superellipse at the edge.
/// Arm B is arm A rotated by PI.
pub fn spiral_point(p: f32) -> ((f32, f32), (f32, f32)) {
    let q = p.clamp(0.0, 1.0).sqrt(); // normalised radius
    let theta = crate::tune("K", K_WIND) * q;
    let aspect = W as f32 / H as f32;
    let a = 1.0 + (aspect - 1.0) * q;
    let n = 2.0 + (N_EDGE - 2.0) * q;
    let (s, c) = theta.sin_cos();
    let r_base = ((c.abs() / a).powf(n) + s.abs().powf(n)).powf(-1.0 / n);
    let scale = q * (H as f32 / 2.0) * EDGE_SCALE;
    let u = r_base * c * scale;
    let v = r_base * s * scale;
    let cx = (W as f32 - 1.0) / 2.0;
    let cy = (H as f32 - 1.0) / 2.0;
    let clamp = |x: f32, y: f32| (x.clamp(0.0, (W - 1) as f32), y.clamp(0.0, (H - 1) as f32));
    (clamp(cx + u, cy + v), clamp(cx - u, cy - v))
}

/// Progress for data sample `j` (0-based within the data section, may be fractional).
#[inline]
pub fn progress(j: f32) -> f32 {
    (j / DATA_LEN as f32).clamp(0.0, 1.0)
}
