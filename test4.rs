#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    #[test]
    fn test_uv_neutral() {
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
        
        let mut pure_white = 0;
        let mut tinted = 0;
        for i in 0..(W*H) {
            let r = out_frame[i*3];
            let g = out_frame[i*3+1];
            let b = out_frame[i*3+2];
            if r > 240 && g > 240 && b > 240 {
                pure_white += 1;
            } else if r > 200 {
                tinted += 1;
            }
        }
        println!("Pure white: {}, Tinted red: {}", pure_white, tinted);
    }
}
