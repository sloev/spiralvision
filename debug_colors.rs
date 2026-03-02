#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    #[test]
    fn test_colors() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let mut rgb_frame = vec![0u8; W * H * 3];
        // Fill with white
        for i in 0..(W * H) {
            rgb_frame[i * 3] = 255;
            rgb_frame[i * 3 + 1] = 255;
            rgb_frame[i * 3 + 2] = 255;
        }
        
        let audio_samples = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples);
        let audio_samples2 = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples2);
        
        let out_frame = decoder.frames.pop().unwrap();
        // Check middle pixel
        let mid_idx = (H / 2 * W + W / 2) * 3;
        println!("Mid pixel: {}, {}, {}", out_frame[mid_idx], out_frame[mid_idx+1], out_frame[mid_idx+2]);
        assert!(out_frame[mid_idx] > 200); // Should be white
    }
}
