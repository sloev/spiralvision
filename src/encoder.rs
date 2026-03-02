use std::f32::consts::PI;
use crate::protocol::*;

pub const F_MIN: f32 = 3000.0;
pub const F_MAX: f32 = 12000.0;
pub const A_MIN: f32 = 0.1;
pub const A_MAX: f32 = 1.0;

pub struct Encoder {
    phi_a: f32,
    phi_b: f32,
}

impl Encoder {
    pub fn new() -> Self {
        Self {
            phi_a: 0.0,
            phi_b: 0.0,
        }
    }

    pub fn encode_frame(&mut self, rgb_frame: &[u8]) -> Vec<(f32, f32)> {
        // rgb_frame is expected to be W * H * 3
        let mut audio_samples = Vec::with_capacity(N);
        
        let mut yuv_frame = vec![(0.0, 0.0, 0.0); W * H];
        for i in 0..(W * H) {
            let r = rgb_frame[i * 3];
            let g = rgb_frame[i * 3 + 1];
            let b = rgb_frame[i * 3 + 2];
            yuv_frame[i] = rgb_to_yuv(r, g, b);
        }

        let dt = 1.0 / AUDIO_RATE as f32;

        for i in 0..N {
            if i < SYNC_SAMPLES {
                let f_sync = 1000.0;
                self.phi_a += 2.0 * PI * f_sync * dt;
                self.phi_b += 2.0 * PI * f_sync * dt;
                
                // Wrap phases
                if self.phi_a > 2.0 * PI { self.phi_a -= 2.0 * PI; }
                if self.phi_b > 2.0 * PI { self.phi_b -= 2.0 * PI; }

                audio_samples.push((self.phi_a.sin(), self.phi_b.sin()));
            } else {
                let (coord_a, coord_b) = get_coordinates(i);
                
                let yuv_a = bilinear_sample(&yuv_frame, coord_a.0, coord_a.1);
                let yuv_b = bilinear_sample(&yuv_frame, coord_b.0, coord_b.1);
                
                let f_a = F_MIN + yuv_a.0 * (F_MAX - F_MIN);
                let f_b = F_MIN + yuv_b.0 * (F_MAX - F_MIN);
                
                self.phi_a += 2.0 * PI * f_a * dt;
                self.phi_b += 2.0 * PI * f_b * dt;
                
                // Wrap phase
                if self.phi_a > 2.0 * PI { self.phi_a -= 2.0 * PI; }
                if self.phi_b > 2.0 * PI { self.phi_b -= 2.0 * PI; }
                
                // Envelope
                // In SpiraVision, we map [0,1] to [0.1, 1.0].
                // The true peak tracker accurately reflects this.
                let env_a = A_MIN + yuv_a.1 * (A_MAX - A_MIN);
                let env_b = A_MIN + yuv_b.2 * (A_MAX - A_MIN);
                
                let sig_l = env_a * self.phi_a.sin();
                let sig_r = env_b * self.phi_b.sin();
                
                audio_samples.push((sig_l, sig_r));
            }
        }
        
        audio_samples
    }
}
