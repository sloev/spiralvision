#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    #[test]
    fn test_timing() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let mut rgb_frame = vec![0u8; W * H * 3];
        let audio_samples = encoder.encode_frame(&rgb_frame);
        
        let start = std::time::Instant::now();
        decoder.process_samples(&audio_samples);
        let audio_samples2 = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples2);
        println!("Decode 2 frames took: {:?}", start.elapsed());
    }
}
