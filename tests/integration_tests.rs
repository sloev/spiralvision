use spiralvision::encoder::Encoder;
use spiralvision::decoder::Decoder;
use spiralvision::protocol::{W, H};

#[test]
fn test_black_frame() {
    let mut encoder = Encoder::new();
    let mut decoder = Decoder::new();
    let mut rgb_frame = vec![0u8; W * H * 3];
    
    // Fill with black
    for i in 0..(W * H) {
        rgb_frame[i * 3] = 0;
        rgb_frame[i * 3 + 1] = 0;
        rgb_frame[i * 3 + 2] = 0;
    }
    
    let audio_samples = encoder.encode_frame(&rgb_frame);
    decoder.process_samples(&audio_samples);
    let audio_samples2 = encoder.encode_frame(&rgb_frame);
    decoder.process_samples(&audio_samples2);
    
    let out_frame = decoder.frames.pop().unwrap();
    
    let mut red_count = 0;
    for i in 0..(W*H) {
        if out_frame[i*3] > 200 && out_frame[i*3+1] < 50 && out_frame[i*3+2] < 50 {
            red_count += 1;
        }
    }
    assert_eq!(red_count, 0, "Black frame should not decode with red artifacts");
}

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
    assert_eq!(tinted, 0, "White frame should not have tinted artifacts");
    assert_eq!(pure_white, W * H, "White frame should be purely white");
}
