use spiralvision_core::protocol::rgb_to_yuv;
use spiralvision_core::*;
fn main() {
    let wear: f32 = std::env::args()
        .nth(1)
        .map(|s| s.parse().unwrap())
        .unwrap_or(0.0);
    for &(r, g, b) in &[
        (0u8, 0u8, 0u8),
        (255, 255, 255),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (0, 0, 255),
        (255, 255, 0),
    ] {
        let img: Vec<u8> = (0..W * H).flat_map(|_| [r, g, b]).collect();
        let mut enc = Encoder::new();
        let mut dec = Decoder::new();
        let mut tp = TapeParams::wear(wear);
        if let Ok(o) = std::env::var("ONLY") {
            let base = TapeParams::wear(0.0);
            let w = tp.clone();
            tp = base;
            tp.gain = 1.0;
            match o.as_str() {
                "speed" => tp.speed = w.speed,
                "wow" => {
                    tp.wow = w.wow;
                    tp.flutter = w.flutter
                }
                "hf" => tp.hf_cutoff = w.hf_cutoff,
                "az" => tp.azimuth = w.azimuth,
                "drive" => tp.drive = w.drive,
                "hiss" => tp.hiss = w.hiss,
                "hum" => tp.hum = w.hum,
                "drop" => tp.dropouts = w.dropouts,
                _ => {}
            }
        }
        let mut tape = TapeSim::new(tp, 48000.0, 3);
        let mut last = None;
        for _ in 0..6 {
            let a = enc.encode_frame(&img);
            let a = if wear > 0.0 { tape.process(&a) } else { a };
            dec.process_samples(&a);
            while let Some(f) = dec.pop_frame() {
                last = Some(f);
            }
        }
        let f = last.unwrap();
        let c = (H / 2 * W + W / 2) * 3;
        let e = (20 * W + 20) * 3;
        let yi = rgb_to_yuv(r, g, b);
        let yc = rgb_to_yuv(f[c], f[c + 1], f[c + 2]);
        let ye = rgb_to_yuv(f[e], f[e + 1], f[e + 2]);
        println!(
            "in ({r},{g},{b}) yuv {:.3?} -> center {:.3?} edge {:.3?} {}",
            yi,
            yc,
            ye,
            dec.debug()
        );
    }
}
