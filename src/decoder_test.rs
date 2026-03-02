#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    #[test]
    fn test_encode_decode() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        
        let mut rgb_frame = vec![0u8; W * H * 3];
        // Fill with some color
        for i in 0..(W * H) {
            rgb_frame[i * 3] = 255;
            rgb_frame[i * 3 + 1] = 128;
            rgb_frame[i * 3 + 2] = 64;
        }
        
        let audio_samples = encoder.encode_frame(&rgb_frame);
        assert_eq!(audio_samples.len(), N);
        
        // Pass through decoder
        decoder.process_samples(&audio_samples);
        
        // Now process another frame to trigger the end of the first frame's sync (or just check if it emitted from the safety catch)
        let audio_samples2 = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples2);
        
        println!("Emitted frames: {}", decoder.frames.len());
        assert!(decoder.frames.len() > 0, "Decoder failed to emit any frames!");
    }
}
