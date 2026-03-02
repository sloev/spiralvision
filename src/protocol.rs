pub const W: usize = 320;
pub const H: usize = 240;
pub const N: usize = 4800;
pub const K_WIND: f32 = 150.0;
pub const AUDIO_RATE: u32 = 48000;
pub const SYNC_SAMPLES: usize = 240; // 5ms * 48kHz

pub fn yuv_to_rgb(y: f32, u: f32, v: f32) -> (u8, u8, u8) {
    let y = y.clamp(0.0, 1.0);
    let u = u.clamp(0.0, 1.0) - 0.5;
    let v = v.clamp(0.0, 1.0) - 0.5;

    let r = (y + 1.13983 * v) * 255.0;
    let g = (y - 0.39465 * u - 0.58060 * v) * 255.0;
    let b = (y + 2.03211 * u) * 255.0;

    (
        r.clamp(0.0, 255.0) as u8,
        g.clamp(0.0, 255.0) as u8,
        b.clamp(0.0, 255.0) as u8,
    )
}

pub fn rgb_to_yuv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let u = -0.14713 * r - 0.28886 * g + 0.436 * b + 0.5;
    let v = 0.615 * r - 0.51499 * g - 0.10001 * b + 0.5;

    (y.clamp(0.0, 1.0), u.clamp(0.0, 1.0), v.clamp(0.0, 1.0))
}

pub fn get_coordinates(i: usize) -> ((f32, f32), (f32, f32)) {
    // p is normalized progress
    let p = (i - SYNC_SAMPLES) as f32 / (N - SYNC_SAMPLES) as f32;
    let p = p.clamp(0.0, 1.0);
    
    let theta = K_WIND * p.sqrt();
    let aspect = W as f32 / H as f32;
    let a = 1.0 + (aspect - 1.0) * p;
    let n = 2.0 + 8.0 * p;
    
    let cos_t = theta.cos();
    let sin_t = theta.sin();
    
    let base_term1 = (cos_t.abs() / a).powf(n);
    let base_term2 = sin_t.abs().powf(n);
    let r_base = (base_term1 + base_term2).powf(-1.0 / n);
    
    let r = p * (H as f32 / 2.0) * r_base;
    
    // Left Spiral (A)
    let xa = (W as f32 / 2.0) + r * a * cos_t;
    let ya = (H as f32 / 2.0) + r * sin_t;
    
    // Right Spiral (B) - Phase shifted by PI
    let xb = (W as f32 / 2.0) + r * a * (theta + std::f32::consts::PI).cos();
    let yb = (H as f32 / 2.0) + r * (theta + std::f32::consts::PI).sin();
    
    ((xa.clamp(0.0, (W-1) as f32), ya.clamp(0.0, (H-1) as f32)), 
     (xb.clamp(0.0, (W-1) as f32), yb.clamp(0.0, (H-1) as f32)))
}

pub fn bilinear_sample(grid: &[(f32, f32, f32)], x: f32, y: f32) -> (f32, f32, f32) {
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(W - 1);
    let y1 = (y0 + 1).min(H - 1);
    
    let fx = x - x.floor();
    let fy = y - y.floor();
    
    let p00 = grid[y0 * W + x0];
    let p10 = grid[y0 * W + x1];
    let p01 = grid[y1 * W + x0];
    let p11 = grid[y1 * W + x1];
    
    let i0 = p00.0 * (1.0 - fx) + p10.0 * fx;
    let i1 = p01.0 * (1.0 - fx) + p11.0 * fx;
    let y_val = i0 * (1.0 - fy) + i1 * fy;
    
    let i0 = p00.1 * (1.0 - fx) + p10.1 * fx;
    let i1 = p01.1 * (1.0 - fx) + p11.1 * fx;
    let u_val = i0 * (1.0 - fy) + i1 * fy;
    
    let i0 = p00.2 * (1.0 - fx) + p10.2 * fx;
    let i1 = p01.2 * (1.0 - fx) + p11.2 * fx;
    let v_val = i0 * (1.0 - fy) + i1 * fy;
    
    (y_val, u_val, v_val)
}
