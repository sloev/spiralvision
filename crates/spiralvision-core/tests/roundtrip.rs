use spiralvision_core::dsp::{Resampler, Rng};
use spiralvision_core::protocol::rgb_to_yuv;
use spiralvision_core::*;

fn solid(r: u8, g: u8, b: u8) -> Vec<u8> {
    (0..W * H).flat_map(|_| [r, g, b]).collect()
}

/// Encode `frames` copies of `img`, optionally through a tape, decode, return last frame.
fn roundtrip(img: &[u8], frames: usize, tape: Option<TapeParams>) -> (Vec<u8>, Decoder) {
    let mut enc = Encoder::new();
    let mut dec = Decoder::new();
    let mut sim = tape.map(|p| TapeSim::new(p, SAMPLE_RATE as f32, 11));
    let mut last = None;
    for _ in 0..frames {
        let mut a = enc.encode_frame(img);
        if let Some(s) = &mut sim {
            a = s.process(&a);
        }
        dec.process_samples(&a);
        while let Some(f) = dec.pop_frame() {
            last = Some(f);
        }
    }
    (last.expect("no frame decoded"), dec)
}

fn mean_yuv(f: &[u8]) -> (f32, f32, f32) {
    let mut acc = (0.0, 0.0, 0.0);
    // ignore a border where the spiral corners are extrapolated
    let mut n = 0.0;
    for y in 20..H - 20 {
        for x in 20..W - 20 {
            let i = (y * W + x) * 3;
            let (a, b, c) = rgb_to_yuv(f[i], f[i + 1], f[i + 2]);
            acc.0 += a;
            acc.1 += b;
            acc.2 += c;
            n += 1.0;
        }
    }
    (acc.0 / n, acc.1 / n, acc.2 / n)
}

const COLORS: [(u8, u8, u8); 6] = [
    (0, 0, 0),
    (255, 255, 255),
    (128, 128, 128),
    (255, 0, 0),
    (0, 160, 0),
    (40, 40, 220),
];

#[test]
fn clean_channel_reproduces_solid_colors() {
    for &(r, g, b) in &COLORS {
        let (out, dec) = roundtrip(&solid(r, g, b), 5, None);
        assert!(dec.is_locked());
        let want = rgb_to_yuv(r, g, b);
        let got = mean_yuv(&out);
        for (w, g) in [(want.0, got.0), (want.1, got.1), (want.2, got.2)] {
            assert!(
                (w - g).abs() < 0.02,
                "({r},{g},{b}): want {want:?} got {got:?}"
            );
        }
    }
}

#[test]
fn worn_cassette_keeps_colors_close() {
    for &(r, g, b) in &COLORS {
        let (out, dec) = roundtrip(&solid(r, g, b), 10, Some(TapeParams::wear(1.0)));
        assert!(dec.is_locked());
        assert!((dec.speed() - 0.988).abs() < 0.005, "speed {}", dec.speed());
        let want = rgb_to_yuv(r, g, b);
        let got = mean_yuv(&out);
        for (w, g) in [(want.0, got.0), (want.1, got.1), (want.2, got.2)] {
            assert!(
                (w - g).abs() < 0.09,
                "({r},{g},{b}): want {want:?} got {got:?}"
            );
        }
    }
}

#[test]
fn picture_survives_tape() {
    // left half white, right half black: check the edge survives
    let img: Vec<u8> = (0..W * H)
        .flat_map(|i| {
            if i % W < W / 2 {
                [255, 255, 255]
            } else {
                [0, 0, 0]
            }
        })
        .collect();
    let (out, _) = roundtrip(&img, 10, Some(TapeParams::wear(1.0)));
    let y = |x: usize| {
        rgb_to_yuv(
            out[(H / 2 * W + x) * 3],
            out[(H / 2 * W + x) * 3 + 1],
            out[(H / 2 * W + x) * 3 + 2],
        )
        .0
    };
    assert!(y(60) > 0.8, "left {}", y(60));
    assert!(y(260) < 0.2, "right {}", y(260));
}

#[test]
fn constant_frame_rate_for_any_input() {
    let mut rng = Rng::new(5);
    let secs = 5usize;
    let inputs: Vec<Vec<(f32, f32)>> = vec![
        (0..48000 * secs)
            .map(|_| (rng.normal() * 0.3, rng.normal() * 0.3))
            .collect(),
        vec![(0.0, 0.0); 48000 * secs],
        {
            // coherent signal with a 0.37 s gap of silence (tape splice) in the middle
            let mut enc = Encoder::new();
            let img = solid(200, 50, 50);
            let mut v = Vec::new();
            for i in 0..secs * 10 {
                if i == 20 {
                    v.extend(std::iter::repeat((0.0, 0.0)).take(17_760));
                }
                v.extend(enc.encode_frame(&img));
            }
            v
        },
    ];
    for input in inputs {
        let mut dec = Decoder::new();
        for chunk in input.chunks(517) {
            dec.process_samples(chunk);
        }
        let expect = input.len() as f64 / N as f64;
        let got = dec.frames_out as f64;
        assert!((got - expect).abs() <= 2.0, "frames {got} vs {expect}");
    }
}

#[test]
fn relocks_after_splice() {
    let mut enc = Encoder::new();
    let mut dec = Decoder::new();
    let img = solid(255, 255, 255);
    for i in 0..30 {
        if i == 10 {
            dec.process_samples(&vec![(0.0, 0.0); 1777]);
        }
        dec.process_samples(&enc.encode_frame(&img));
    }
    assert!(dec.is_locked());
    assert!(
        dec.frames_locked >= 24,
        "locked frames {}",
        dec.frames_locked
    );
}

#[test]
fn decodes_44k1_and_slow_tape() {
    // 44.1 kHz capture of a tape running 3% slow
    let mut enc = Encoder::new();
    let img = solid(255, 0, 0);
    let mut sig = Vec::new();
    for _ in 0..12 {
        sig.extend(enc.encode_frame(&img));
    }
    let mut rs = Resampler::new(48000.0 * 0.97, 44100.0);
    let mut cap = Vec::new();
    for &s in &sig {
        rs.push(s, &mut cap);
    }
    let mut dec = Decoder::with_rate(44100.0);
    dec.process_samples(&cap);
    assert!(dec.is_locked());
    assert!((dec.speed() - 0.97).abs() < 0.004, "speed {}", dec.speed());
    let mut last = None;
    while let Some(f) = dec.pop_frame() {
        last = Some(f);
    }
    let got = mean_yuv(&last.unwrap());
    let want = rgb_to_yuv(255, 0, 0);
    assert!(
        (got.0 - want.0).abs() < 0.04 && (got.2 - want.2).abs() < 0.06,
        "{got:?} vs {want:?}"
    );
}

#[test]
fn noise_makes_varied_frames() {
    let mut rng = Rng::new(9);
    let mut dec = Decoder::new();
    let noise: Vec<(f32, f32)> = (0..48000)
        .map(|_| (rng.normal() * 0.2, rng.normal() * 0.2))
        .collect();
    dec.process_samples(&noise);
    assert!(!dec.is_locked());
    let f = dec.pop_frame().unwrap();
    let mean = f.iter().map(|&x| x as f64).sum::<f64>() / f.len() as f64;
    let var = f.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / f.len() as f64;
    assert!(
        var.sqrt() > 30.0,
        "noise frame too flat: std {}",
        var.sqrt()
    );
}
