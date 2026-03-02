#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    #[test]
    fn test_full_frame() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let mut rgb_frame = vec![0u8; W * H * 3];
        let audio_samples = encoder.encode_frame(&rgb_frame);
        
        decoder.process_samples(&audio_samples);
        println!("Frames emitted after 1 audio chunk: {}", decoder.frames.len());
        
        let audio_samples2 = encoder.encode_frame(&rgb_frame);
        decoder.process_samples(&audio_samples2);
        println!("Frames emitted after 2 audio chunks: {}", decoder.frames.len());
        
        assert!(decoder.frames.len() > 0, "No frames emitted!");
    }
}
