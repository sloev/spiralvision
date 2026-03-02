#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;
    #[test]
    fn test_yuv() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let mut rgb_frame = vec![0u8; W * H * 3];
        for i in 0..(W * H) {
            rgb_frame[i * 3] = 255; rgb_frame[i * 3 + 1] = 255; rgb_frame[i * 3 + 2] = 255;
        }
        let audio_samples = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples);
        let audio_samples2 = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples2);
        
        let mid_idx = H / 2 * W + W / 2;
        println!("Y: {}, U: {}, V: {}", decoder.y_buf[mid_idx], decoder.u_buf[mid_idx], decoder.v_buf[mid_idx]);
    }
}
