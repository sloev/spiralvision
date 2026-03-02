#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    #[test]
    fn test_checkerboard() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let mut rgb_frame = vec![0u8; W * H * 3];
        // Fill with white
        for i in 0..(W * H) {
            rgb_frame[i * 3] = 255;
            rgb_frame[i * 3 + 1] = 0;
            rgb_frame[i * 3 + 2] = 0; // Solid red
        }
        
        let audio_samples = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples);
        let audio_samples2 = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples2);
        
        let out_frame = decoder.frames.pop().unwrap();
        // Check middle pixel
        let mid_idx = (H / 2 * W + W / 2) * 3;
        println!("Mid pixel: {}, {}, {}", out_frame[mid_idx], out_frame[mid_idx+1], out_frame[mid_idx+2]);
        
        let mut black_count = 0;
        let mut red_count = 0;
        for i in 0..(W*H) {
            if out_frame[i*3] < 50 && out_frame[i*3+1] < 50 && out_frame[i*3+2] < 50 {
                black_count += 1;
            }
            if out_frame[i*3] > 200 && out_frame[i*3+1] < 50 && out_frame[i*3+2] < 50 {
                red_count += 1;
            }
        }
        println!("Black pixels: {}, Red pixels: {}", black_count, red_count);
    }
}
