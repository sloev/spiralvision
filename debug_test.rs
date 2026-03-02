#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::Encoder;

    #[test]
    fn test_sync() {
        let mut encoder = Encoder::new();
        let mut decoder = Decoder::new();
        let mut rgb_frame = vec![0u8; W * H * 3];
        let audio_samples = encoder.encode_frame(&rgb_frame);
        
        let mut max_sync = 0;
        for (i, &(l, r)) in audio_samples.iter().enumerate().take(300) {
            decoder.process_samples(&[(l, r)]);
            if decoder.sync_consecutive > max_sync { max_sync = decoder.sync_consecutive; }
            if i % 20 == 0 {
                println!("i: {}, freq: {:.1}, env: {:.2}, in_sync: {}, sync_con: {}", i, decoder.current_freq_l, decoder.env_l, decoder.in_sync, decoder.sync_consecutive);
            }
        }
        println!("Max sync consecutive: {}", max_sync);
        
        // Ensure frame starts
        assert!(decoder.in_sync, "Failed to enter sync!");
    }
}
