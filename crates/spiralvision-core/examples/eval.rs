//! Round-trip quality harness.
//! cargo run --release -p spiralvision-core --example eval -- frames.rgb [wear] [out.ppm]
use spiralvision_core::*;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let data = std::fs::read(&args[1]).expect("read frames");
    let wear: f32 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(0.0);
    let fsz = W * H * 3;
    let frames: Vec<&[u8]> = data.chunks_exact(fsz).collect();
    let mut enc = Encoder::new();
    let mut dec = Decoder::new();
    if let Ok(v) = std::env::var("LS") {
        dec.params.luma_sigma = v.parse().unwrap();
    }
    if let Ok(v) = std::env::var("CS") {
        dec.params.chroma_sigma = v.parse().unwrap();
    }
    let mut tape = TapeSim::new(TapeParams::wear(wear), SAMPLE_RATE as f32, 7);
    let mut outs = Vec::new();
    let t0 = std::time::Instant::now();
    // lead-in silence to shift alignment
    dec.process_samples(&vec![(0.0, 0.0); 1234]);
    for f in &frames {
        let a = enc.encode_frame(f);
        let a = if wear > 0.0 { tape.process(&a) } else { a };
        dec.process_samples(&a);
        while let Some(o) = dec.pop_frame() {
            outs.push(o);
        }
    }
    let el = t0.elapsed();
    // align: find best offset
    let mut best = (f64::MIN, 0i32);
    for off in -3i32..=3 {
        let mut s = 0.0;
        let mut c = 0;
        for (i, o) in outs.iter().enumerate().skip(3) {
            let k = i as i32 + off;
            if k >= 0 && (k as usize) < frames.len() {
                s += image::psnr(frames[k as usize], o);
                c += 1;
            }
        }
        if c > 0 && s / c as f64 > best.0 {
            best = (s / c as f64, off);
        }
    }
    println!(
        "frames in {} out {} locked {} speed {:.4} psnr {:.2} dB (offset {}) time/frame {:.2} ms",
        frames.len(),
        outs.len(),
        dec.frames_locked,
        dec.speed(),
        best.0,
        best.1,
        el.as_secs_f64() * 1000.0 / frames.len() as f64
    );
    if let Some(p) = args.get(3) {
        let i = (outs.len() / 2).max(3).min(outs.len() - 1);
        let k = (i as i32 + best.1).clamp(0, frames.len() as i32 - 1) as usize;
        let mut f = std::fs::File::create(p).unwrap();
        write!(f, "P6\n{} {}\n255\n", W * 2, H).unwrap();
        for y in 0..H {
            f.write_all(&frames[k][y * W * 3..(y + 1) * W * 3]).unwrap();
            f.write_all(&outs[i][y * W * 3..(y + 1) * W * 3]).unwrap();
        }
    }
}
