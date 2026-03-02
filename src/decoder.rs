use crate::protocol::*;
use crate::encoder::{F_MIN, F_MAX, A_MIN, A_MAX};

pub struct Decoder {
    // Sync state
    sync_consecutive: usize,
    in_sync: bool,
    sample_count: usize,
    
    // Demodulation state
    last_l: f32,
    last_r: f32,
    last_zc_l: f32,
    last_zc_r: f32,
    current_freq_l: f32,
    current_freq_r: f32,
    state_l: i8,
    state_r: i8,
    
    env_l: f32,
    env_r: f32,
    
    // Canvas Buffers (Independent for Y, U, V to avoid color banding)
    y_buf: Vec<f32>,
    u_buf: Vec<f32>,
    v_buf: Vec<f32>,
    y_mask: Vec<bool>,
    u_mask: Vec<bool>,
    v_mask: Vec<bool>,
    
    pub frames: Vec<Vec<u8>>,
    global_time: f32,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            sync_consecutive: 0,
            in_sync: false,
            sample_count: 0,
            last_l: 0.0,
            last_r: 0.0,
            last_zc_l: 0.0,
            last_zc_r: 0.0,
            current_freq_l: 0.0,
            current_freq_r: 0.0,
            state_l: 0,
            state_r: 0,
            env_l: 0.0,
            env_r: 0.0,
            y_buf: vec![0.0; W * H],
            u_buf: vec![0.5; W * H],
            v_buf: vec![0.5; W * H],
            y_mask: vec![false; W * H],
            u_mask: vec![false; W * H],
            v_mask: vec![false; W * H],
            frames: Vec::new(),
            global_time: 0.0,
        }
    }

    pub fn process_samples(&mut self, samples: &[(f32, f32)]) {
        let dt = 1.0 / AUDIO_RATE as f32;
        let hyst = 0.05; // 5% hysteresis to reject noise
        
        for &(l, r) in samples {
            self.global_time += dt;
            
            // 1. Robust Frequency Tracking with Hysteresis
            let new_state_l = if l > hyst { 1 } else if l < -hyst { -1 } else { self.state_l };
            if self.state_l == -1 && new_state_l == 1 {
                let period = self.global_time - self.last_zc_l;
                if period > 0.0 { self.current_freq_l = 1.0 / period; }
                self.last_zc_l = self.global_time;
            }
            self.state_l = new_state_l;

            let new_state_r = if r > hyst { 1 } else if r < -hyst { -1 } else { self.state_r };
            if self.state_r == -1 && new_state_r == 1 {
                let period = self.global_time - self.last_zc_r;
                if period > 0.0 { self.current_freq_r = 1.0 / period; }
                self.last_zc_r = self.global_time;
            }
            self.state_r = new_state_r;
            
            self.last_l = l;
            self.last_r = r;
            
            // 2. Envelope Tracking (True Peak)
            if l.abs() > self.env_l {
                self.env_l = l.abs();
            } else {
                self.env_l *= 0.999;
            }
            if r.abs() > self.env_r {
                self.env_r = r.abs();
            } else {
                self.env_r *= 0.999;
            }
            
            // 3. Sync Pulse Detection
            let is_sync_sig = self.current_freq_l > 750.0 && self.current_freq_l < 1250.0 && self.env_l > 0.4;
            if is_sync_sig {
                self.sync_consecutive += 1;
            } else {
                if self.sync_consecutive > 60 { // Relaxed for tape wow/flutter (was 100)
                    self.emit_frame(); // Emit any previous frame
                    self.in_sync = true;
                    self.sample_count = 0;
                    self.env_l = 0.0;
                    self.env_r = 0.0;
                }
                self.sync_consecutive = 0;
            }
            
            // 4. Data Collection
            if self.in_sync {
                if self.sample_count < (N - SYNC_SAMPLES) {
                    let i = self.sample_count + SYNC_SAMPLES;
                    let (coord_a, coord_b) = get_coordinates(i);
                    
                    let y_a = ((self.current_freq_l - F_MIN) / (F_MAX - F_MIN)).clamp(0.0, 1.0);
                    let y_b = ((self.current_freq_r - F_MIN) / (F_MAX - F_MIN)).clamp(0.0, 1.0);
                    let u_a = ((self.env_l - A_MIN) / (A_MAX - A_MIN)).clamp(0.0, 1.0);
                    let v_b = ((self.env_r - A_MIN) / (A_MAX - A_MIN)).clamp(0.0, 1.0);
                    
                    let (xa, ya) = (coord_a.0.round() as usize, coord_a.1.round() as usize);
                    let (xb, yb) = (coord_b.0.round() as usize, coord_b.1.round() as usize);
                    
                    if xa < W && ya < H {
                        let idx = ya * W + xa;
                        self.y_buf[idx] = y_a;
                        self.u_buf[idx] = u_a;
                        self.y_mask[idx] = true;
                        self.u_mask[idx] = true;
                    }
                    if xb < W && yb < H {
                        let idx = yb * W + xb;
                        self.y_buf[idx] = y_b;
                        self.v_buf[idx] = v_b;
                        self.y_mask[idx] = true;
                        self.v_mask[idx] = true;
                    }
                    self.sample_count += 1;
                }
            }
        }
    }
    
    fn emit_frame(&mut self) {
        if self.sample_count < 100 { return; }

        let mut final_y = self.y_buf.clone();
        let mut final_u = self.u_buf.clone();
        let mut final_v = self.v_buf.clone();
        
        // 1. FAST VORONOI FILL (Distance Transform)
        // We perform the fill independently for Y, U, and V!
        // This ensures the "missing" color from one spiral gets pulled naturally
        // from the other spiral, completely eliminating the "alternating color rings".
        fast_voronoi_fill(&mut final_y, &self.y_mask, W, H);
        fast_voronoi_fill(&mut final_u, &self.u_mask, W, H);
        fast_voronoi_fill(&mut final_v, &self.v_mask, W, H);

        // 2. SMOOTHING FILTER
        // The Voronoi diagram creates "cells" with sharp edges.
        // We apply a 2-pass box blur to melt it into a seamless gradient.
        for _ in 0..2 {
            let mut next_y = final_y.clone();
            let mut next_u = final_u.clone();
            let mut next_v = final_v.clone();
            
            for y in 0..H {
                for x in 0..W {
                    let mut ys = 0.0; let mut us = 0.0; let mut vs = 0.0;
                    let mut c = 0.0;
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            if nx >= 0 && nx < W as i32 && ny >= 0 && ny < H as i32 {
                                let nidx = (ny as usize) * W + (nx as usize);
                                ys += final_y[nidx];
                                us += final_u[nidx];
                                vs += final_v[nidx];
                                c += 1.0;
                            }
                        }
                    }
                    let idx = y * W + x;
                    next_y[idx] = ys/c;
                    next_u[idx] = us/c;
                    next_v[idx] = vs/c;
                }
            }
            final_y = next_y;
            final_u = next_u;
            final_v = next_v;
        }

        // 3. Final RGB conversion
        let mut rgb_frame = vec![0u8; W * H * 3];
        for i in 0..(W * H) {
            let (r, g, b) = yuv_to_rgb(final_y[i], final_u[i], final_v[i]);
            rgb_frame[i * 3] = r;
            rgb_frame[i * 3 + 1] = g;
            rgb_frame[i * 3 + 2] = b;
        }
        
        self.frames.push(rgb_frame);
        
        // Reset state
        self.y_buf.fill(0.0);
        self.u_buf.fill(0.5);
        self.v_buf.fill(0.5);
        self.y_mask.fill(false);
        self.u_mask.fill(false);
        self.v_mask.fill(false);
        self.in_sync = false;
        self.sample_count = 0;
    }
}

// Ultra-fast 2-Pass Distance Transform (Voronoi)
fn fast_voronoi_fill(buf: &mut [f32], mask: &[bool], w: usize, h: usize) {
    let mut nearest = vec![(-10000, -10000); w * h];
    
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] {
                nearest[y * w + x] = (x as i32, y as i32);
            }
        }
    }
    
    // Pass 1: Top-Left to Bottom-Right
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let p = nearest[idx];
            let mut best_dist = dist_sq(x as i32, y as i32, p.0, p.1);
            let mut best_p = p;
            
            let neighbors = [
                (x.wrapping_sub(1), y),
                (x, y.wrapping_sub(1)),
                (x.wrapping_sub(1), y.wrapping_sub(1)),
                (x + 1, y.wrapping_sub(1))
            ];
            
            for &(nx, ny) in &neighbors {
                if nx < w && ny < h {
                    let n_idx = ny * w + nx;
                    let np = nearest[n_idx];
                    let d = dist_sq(x as i32, y as i32, np.0, np.1);
                    if d < best_dist {
                        best_dist = d;
                        best_p = np;
                    }
                }
            }
            nearest[idx] = best_p;
        }
    }
    
    // Pass 2: Bottom-Right to Top-Left
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let idx = y * w + x;
            let p = nearest[idx];
            let mut best_dist = dist_sq(x as i32, y as i32, p.0, p.1);
            let mut best_p = p;
            
            let neighbors = [
                (x + 1, y),
                (x, y + 1),
                (x + 1, y + 1),
                (x.wrapping_sub(1), y + 1)
            ];
            
            for &(nx, ny) in &neighbors {
                if nx < w && ny < h {
                    let n_idx = ny * w + nx;
                    let np = nearest[n_idx];
                    let d = dist_sq(x as i32, y as i32, np.0, np.1);
                    if d < best_dist {
                        best_dist = d;
                        best_p = np;
                    }
                }
            }
            nearest[idx] = best_p;
        }
    }
    
    // Apply nearest values
    let mut temp = buf.to_vec();
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let p = nearest[idx];
            if p.0 >= 0 && p.0 < w as i32 && p.1 >= 0 && p.1 < h as i32 {
                temp[idx] = buf[(p.1 as usize) * w + (p.0 as usize)];
            }
        }
    }
    buf.copy_from_slice(&temp);
}

fn dist_sq(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    let dx = x1 - x2;
    let dy = y1 - y2;
    dx * dx + dy * dy
}
