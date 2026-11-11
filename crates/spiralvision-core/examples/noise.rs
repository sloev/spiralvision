use spiralvision_core::{dsp::Rng, *};
use std::io::Write;
fn main() {
    let sp = std::env::args().nth(1).unwrap();
    let mut rng = Rng::new(1);
    let secs = 3;
    let n = 48000 * secs;
    let kinds: Vec<(&str, Box<dyn Fn(usize, &mut Rng) -> (f32, f32)>)> = vec![
        (
            "white",
            Box::new(|_, r: &mut Rng| (r.normal() * 0.3, r.normal() * 0.3)),
        ),
        (
            "music",
            Box::new(|i, r: &mut Rng| {
                let t = i as f32 / 48000.0;
                let env = (0.5 + 0.5 * (6.28 * 2.0 * t).sin()).powi(2);
                let x = 0.4 * (6.28 * 220.0 * t).sin()
                    + 0.2 * (6.28 * 3300.0 * t + 3.0 * (6.28 * 5.0 * t).sin()).sin() * env
                    + 0.05 * r.normal();
                (
                    x,
                    0.4 * (6.28 * 330.0 * t).sin() + 0.25 * (6.28 * 7000.0 * t).sin() * (1.0 - env),
                )
            }),
        ),
        ("silence", Box::new(|_, _| (0.0, 0.0))),
    ];
    for (name, f) in kinds {
        let mut dec = Decoder::new();
        let mut count = 0;
        let mut f_out = std::fs::File::create(format!("{sp}/noise-{name}.ppm")).unwrap();
        // feed in odd-sized chunks like a sound card would
        let mut i = 0;
        while i < n {
            let len = 333.min(n - i);
            let chunk: Vec<(f32, f32)> = (i..i + len).map(|k| f(k, &mut rng)).collect();
            dec.process_samples(&chunk);
            i += len;
            while let Some(fr) = dec.pop_frame() {
                count += 1;
                if count == 20 {
                    write!(f_out, "P6\n{} {}\n255\n", W, H).unwrap();
                    f_out.write_all(&fr).unwrap();
                }
            }
        }
        println!(
            "{name}: {count} frames from {secs}s (expected {})",
            secs * 10
        );
    }
}
