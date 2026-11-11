//! spiralvision-cli — headless SpiralVision-10 deck.
//!
//! Every input/output goes through ffmpeg, so files, cameras (v4l2,
//! avfoundation, dshow), sound cards (alsa, pulse) and v4l2loopback all work.

use spiralvision_core::io::{read_audio, VideoReader, VideoWriter, WavOut};
use spiralvision_core::{image::psnr, Decoder, Encoder, TapeParams, TapeSim, SAMPLE_RATE};
use std::process::{Command, Stdio};

const USAGE: &str = "\
SPIRALVISION-10 // video on cassette, totally.

USAGE:
  spiralvision-cli encode    <video-in> <audio-out.wav>  [opts]
  spiralvision-cli decode    <audio-in> <video-out>      [opts]
  spiralvision-cli roundtrip <video-in> <video-out.mp4>  [opts]
  spiralvision-cli bench     <video-in>                  [opts]

OPTIONS:
  -f <fmt>        ffmpeg input format  (e.g. v4l2, avfoundation, alsa, pulse, lavfi)
  -F <fmt>        ffmpeg output format (e.g. v4l2, sdl, alsa, pulse)
  -t <seconds>    limit duration
  --tape <wear>   run through the cassette simulator (0 = clean, 1 = worn, 2 = trashed)
  --persistence <0..0.95>   phosphor trails on decode
  --wav <path>    roundtrip: also save the (taped) audio
  --mux           decode/roundtrip: put the SpiralVision audio track into the video

EXAMPLES:
  spiralvision-cli encode clip.mp4 tape-side-a.wav
  spiralvision-cli decode recording.wav decoded.mp4 --mux
  spiralvision-cli encode -f v4l2 /dev/video0 default -F alsa        # camera -> line out
  spiralvision-cli decode -f alsa default /dev/video10 -F v4l2       # line in -> virtual cam
";

struct Opts {
    pos: Vec<String>,
    fin: Option<String>,
    fout: Option<String>,
    secs: Option<f32>,
    tape: Option<f32>,
    persistence: f32,
    wav: Option<String>,
    mux: bool,
}

fn parse() -> Opts {
    let mut o = Opts {
        pos: vec![],
        fin: None,
        fout: None,
        secs: None,
        tape: None,
        persistence: 0.0,
        wav: None,
        mux: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = || {
            it.next()
                .unwrap_or_else(|| die(&format!("{a} needs a value")))
        };
        match a.as_str() {
            "-f" => o.fin = Some(val()),
            "-F" => o.fout = Some(val()),
            "-t" => o.secs = Some(val().parse().unwrap_or_else(|_| die("bad -t"))),
            "--tape" => o.tape = Some(val().parse().unwrap_or_else(|_| die("bad --tape"))),
            "--persistence" => {
                o.persistence = val().parse().unwrap_or_else(|_| die("bad --persistence"))
            }
            "--wav" => o.wav = Some(val()),
            "--mux" => o.mux = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0)
            }
            _ => o.pos.push(a),
        }
    }
    o
}

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}\n\n{USAGE}");
    std::process::exit(2)
}

fn main() {
    let o = parse();
    let cmd = o.pos.first().map(String::as_str).unwrap_or("");
    let arg = |i: usize| {
        o.pos
            .get(i)
            .cloned()
            .unwrap_or_else(|| die("missing argument"))
    };
    match cmd {
        "encode" => encode(&arg(1), &arg(2), &o),
        "decode" => decode(&arg(1), &arg(2), &o),
        "roundtrip" => roundtrip(&arg(1), &arg(2), &o),
        "bench" => bench(&arg(1), &o),
        _ => die("unknown command"),
    }
}

fn tape(o: &Opts, seed: u64) -> Option<TapeSim> {
    o.tape
        .map(|w| TapeSim::new(TapeParams::wear(w), SAMPLE_RATE as f32, seed))
}

/// Audio sink: a WAV file, or a raw ffmpeg pipe for sound cards.
enum Sink {
    Wav(WavOut),
    Pipe(std::process::Child),
}

impl Sink {
    fn open(path: &str, fmt: Option<&str>) -> Sink {
        match fmt {
            None => Sink::Wav(WavOut::create(path).unwrap_or_else(|e| die(&e))),
            Some(f) => Sink::Pipe(
                Command::new("ffmpeg")
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        "f32le",
                        "-ar",
                        &SAMPLE_RATE.to_string(),
                        "-ac",
                        "2",
                        "-i",
                        "-",
                        "-f",
                        f,
                        path,
                    ])
                    .stdin(Stdio::piped())
                    .spawn()
                    .unwrap_or_else(|e| die(&format!("ffmpeg: {e}"))),
            ),
        }
    }
    fn write(&mut self, s: &[(f32, f32)]) -> bool {
        match self {
            Sink::Wav(w) => w.write(s).is_ok(),
            Sink::Pipe(c) => {
                use std::io::Write;
                let bytes: Vec<u8> = s
                    .iter()
                    .flat_map(|&(l, r)| [l.to_le_bytes(), r.to_le_bytes()])
                    .flatten()
                    .collect();
                c.stdin.as_mut().unwrap().write_all(&bytes).is_ok()
            }
        }
    }
    fn finish(self) {
        match self {
            Sink::Wav(w) => w.finish().unwrap_or_else(|e| die(&e)),
            Sink::Pipe(mut c) => {
                drop(c.stdin.take());
                let _ = c.wait();
            }
        }
    }
}

fn encode(input: &str, output: &str, o: &Opts) {
    let mut rd = VideoReader::open(input, o.fin.as_deref(), o.secs)
        .unwrap_or_else(|e| die(&format!("ffmpeg: {e}")));
    let mut enc = Encoder::new();
    let mut sim = tape(o, 1);
    let mut sink = Sink::open(output, o.fout.as_deref());
    let mut n = 0;
    while let Some(f) = rd.next_frame() {
        let mut a = enc.encode_frame(&f);
        if let Some(s) = &mut sim {
            a = s.process(&a);
        }
        if !sink.write(&a) {
            break;
        }
        n += 1;
    }
    sink.finish();
    eprintln!(
        "encoded {n} frames ({:.1} s of tape) -> {output}",
        n as f32 / 10.0
    );
}

fn decode(input: &str, output: &str, o: &Opts) {
    let mux = if o.mux && o.fout.is_none() {
        Some(input)
    } else {
        None
    };
    let mut wr = VideoWriter::open(output, o.fout.as_deref(), mux)
        .unwrap_or_else(|e| die(&format!("ffmpeg: {e}")));
    let mut dec = Decoder::new();
    dec.params.persistence = o.persistence;
    let mut n = 0;
    let mut emit = |dec: &mut Decoder, wr: &mut VideoWriter| -> bool {
        while let Some(f) = dec.pop_frame() {
            if wr.write(&f).is_err() {
                return false;
            }
            n += 1;
        }
        true
    };
    if let Some(fmt) = o.fin.as_deref() {
        // live capture (e.g. -f alsa default): stream through ffmpeg
        let mut c = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                fmt,
                "-i",
                input,
                "-ac",
                "2",
                "-ar",
                &SAMPLE_RATE.to_string(),
                "-f",
                "f32le",
                "-",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| die(&format!("ffmpeg: {e}")));
        let mut out = c.stdout.take().unwrap();
        let mut buf = vec![0u8; 8 * 960];
        use std::io::Read;
        while let Ok(k) = out.read(&mut buf) {
            if k == 0 {
                break;
            }
            let s: Vec<f32> = buf[..k - k % 8]
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            dec.process_interleaved(&s);
            if !emit(&mut dec, &mut wr) {
                break;
            }
        }
        let _ = c.kill();
    } else {
        let (audio, rate) = read_audio(input).unwrap_or_else(|e| die(&e));
        let mut dec2 = Decoder::with_rate(rate as f32);
        dec2.params = dec.params.clone();
        dec = dec2;
        let limit = o
            .secs
            .map(|s| (s * rate as f32) as usize)
            .unwrap_or(audio.len())
            .min(audio.len());
        for chunk in audio[..limit].chunks(4800) {
            dec.process_samples(chunk);
            if !emit(&mut dec, &mut wr) {
                break;
            }
        }
        dec.flush();
        emit(&mut dec, &mut wr);
    }
    let locked = dec.frames_locked;
    let total = dec.frames_out;
    drop(emit);
    wr.finish().ok();
    eprintln!(
        "decoded {n} frames ({locked}/{total} locked, speed {:.3}) -> {output}",
        dec.speed()
    );
}

fn roundtrip(input: &str, output: &str, o: &Opts) {
    let mut rd = VideoReader::open(input, o.fin.as_deref(), o.secs)
        .unwrap_or_else(|e| die(&format!("ffmpeg: {e}")));
    let tmp_wav = std::env::temp_dir().join(format!("spiralvision-{}.wav", std::process::id()));
    let wav_path = o
        .wav
        .clone()
        .unwrap_or_else(|| tmp_wav.to_string_lossy().to_string());
    let mut wav = WavOut::create(&wav_path).unwrap_or_else(|e| die(&e));
    let mut enc = Encoder::new();
    let mut sim = tape(o, 2);
    let mut frames = 0;
    while let Some(f) = rd.next_frame() {
        let mut a = enc.encode_frame(&f);
        if let Some(s) = &mut sim {
            a = s.process(&a);
        }
        wav.write(&a).unwrap_or_else(|e| die(&e));
        frames += 1;
    }
    wav.finish().unwrap_or_else(|e| die(&e));
    eprintln!("encoded {frames} frames");
    let o2 = Opts {
        pos: vec![],
        fin: None,
        fout: o.fout.clone(),
        secs: None,
        tape: None,
        persistence: o.persistence,
        wav: None,
        mux: o.mux,
    };
    decode(&wav_path, output, &o2);
    if o.wav.is_none() {
        let _ = std::fs::remove_file(tmp_wav);
    }
}

fn bench(input: &str, o: &Opts) {
    let mut rd = VideoReader::open(input, o.fin.as_deref(), o.secs)
        .unwrap_or_else(|e| die(&format!("ffmpeg: {e}")));
    let mut enc = Encoder::new();
    let mut dec = Decoder::new();
    let mut sim = tape(o, 3);
    let mut src = Vec::new();
    let mut out = Vec::new();
    let t0 = std::time::Instant::now();
    while let Some(f) = rd.next_frame() {
        let mut a = enc.encode_frame(&f);
        if let Some(s) = &mut sim {
            a = s.process(&a);
        }
        dec.process_samples(&a);
        while let Some(d) = dec.pop_frame() {
            out.push(d);
        }
        src.push(f);
    }
    let dt = t0.elapsed().as_secs_f64();
    // decoder output lags input by one frame
    let pairs: Vec<f64> = out
        .iter()
        .enumerate()
        .skip(2)
        .filter_map(|(i, d)| src.get(i.checked_sub(1)?).map(|s| psnr(s, d)))
        .collect();
    let mean = pairs.iter().sum::<f64>() / pairs.len().max(1) as f64;
    println!(
        "frames {} | locked {}/{} | speed {:.3} | PSNR {:.2} dB | {:.1} ms/frame (encode+decode)",
        src.len(),
        dec.frames_locked,
        dec.frames_out,
        dec.speed(),
        mean,
        dt * 1000.0 / src.len().max(1) as f64
    );
}
