//! File and ffmpeg helpers shared by the CLI and the desktop app
//! (enabled with the `io` feature; not used by the WASM build).

use crate::protocol::{FPS, H, SAMPLE_RATE, W};
use std::io::{Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// ffmpeg input format for the local camera API.
pub fn camera_format() -> &'static str {
    if cfg!(target_os = "macos") {
        "avfoundation"
    } else if cfg!(target_os = "windows") {
        "dshow"
    } else {
        "v4l2"
    }
}

/// Default camera name for the local camera API.
pub fn default_camera() -> String {
    if cfg!(target_os = "macos") {
        "0".into()
    } else if cfg!(target_os = "windows") {
        "video=Integrated Camera".into()
    } else {
        "/dev/video0".into()
    }
}

/// List camera candidates (Linux: /dev/video*).
pub fn list_cameras() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("video") {
                v.push(format!("/dev/{n}"));
            }
        }
    }
    v.sort();
    if v.is_empty() {
        v.push(default_camera());
    }
    v
}

pub fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Streams W x H RGB24 frames at 10 fps from any ffmpeg input.
pub struct VideoReader {
    child: Child,
    out: ChildStdout,
}

impl VideoReader {
    /// `format`: optional ffmpeg input format (e.g. "v4l2", "lavfi").
    pub fn open(input: &str, format: Option<&str>, seconds: Option<f32>) -> std::io::Result<Self> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error"]);
        if let Some(f) = format {
            cmd.args(["-f", f]);
        }
        cmd.args(["-i", input]);
        if let Some(s) = seconds {
            cmd.args(["-t", &s.to_string()]);
        }
        let vf =
            format!("fps={FPS},scale={W}:{H}:force_original_aspect_ratio=increase,crop={W}:{H}");
        cmd.args(["-vf", &vf, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]);
        let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).spawn()?;
        let out = child.stdout.take().unwrap();
        Ok(Self { child, out })
    }

    /// Next frame, or None at end of stream.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; W * H * 3];
        self.out.read_exact(&mut buf).ok()?;
        Some(buf)
    }
}

impl Drop for VideoReader {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Writes RGB24 frames at 10 fps to any ffmpeg output.
pub struct VideoWriter {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl VideoWriter {
    /// `format`: optional ffmpeg output format (e.g. "v4l2", "sdl").
    /// `audio`: optional audio file to mux into the output.
    pub fn open(output: &str, format: Option<&str>, audio: Option<&str>) -> std::io::Result<Self> {
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-hide_banner", "-loglevel", "error", "-y"]);
        cmd.args([
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            &format!("{W}x{H}"),
            "-r",
            &FPS.to_string(),
            "-i",
            "-",
        ]);
        if let Some(a) = audio {
            cmd.args([
                "-i",
                a,
                "-map",
                "0:v",
                "-map",
                "1:a",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-shortest",
            ]);
        }
        match format {
            Some(f) => {
                if f == "v4l2" {
                    cmd.args(["-pix_fmt", "yuv420p"]);
                }
                cmd.args(["-f", f, output]);
            }
            None => {
                let lower = output.to_lowercase();
                if lower.ends_with(".mp4") || lower.ends_with(".mkv") || lower.ends_with(".mov") {
                    cmd.args([
                        "-c:v",
                        "libx264",
                        "-pix_fmt",
                        "yuv420p",
                        "-crf",
                        "20",
                        "-movflags",
                        "+faststart",
                    ]);
                }
                cmd.arg(output);
            }
        }
        let mut child = cmd.stdin(Stdio::piped()).spawn()?;
        let stdin = child.stdin.take();
        Ok(Self { child, stdin })
    }

    pub fn write(&mut self, rgb: &[u8]) -> std::io::Result<()> {
        self.stdin.as_mut().unwrap().write_all(rgb)
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        drop(self.stdin.take());
        self.child.wait()?;
        Ok(())
    }
}

impl Drop for VideoWriter {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

/// Read any audio file as stereo f32 at its native rate (WAV natively,
/// anything else through ffmpeg at 48 kHz).
pub fn read_audio(path: &str) -> Result<(Vec<(f32, f32)>, u32), String> {
    if path.to_lowercase().ends_with(".wav") {
        if let Ok(r) = read_wav(path) {
            return Ok(r);
        }
    }
    let out = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path,
            "-ac",
            "2",
            "-ar",
            &SAMPLE_RATE.to_string(),
            "-f",
            "f32le",
            "-",
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).to_string());
    }
    let s: Vec<f32> = out
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    Ok((
        s.chunks_exact(2).map(|c| (c[0], c[1])).collect(),
        SAMPLE_RATE,
    ))
}

/// Read a WAV file (8/16/24/32-bit int or float, mono or stereo).
pub fn read_wav(path: &str) -> Result<(Vec<(f32, f32)>, u32), String> {
    let mut r = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = r.spec();
    let ch = spec.channels as usize;
    let s: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|x| x.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|x| x.unwrap_or(0) as f32 * scale)
                .collect()
        }
    };
    let pairs = s
        .chunks_exact(ch.max(1))
        .map(|c| if ch >= 2 { (c[0], c[1]) } else { (c[0], c[0]) })
        .collect();
    Ok((pairs, spec.sample_rate))
}

/// Streaming WAV writer (32-bit float stereo at 48 kHz).
pub struct WavOut(hound::WavWriter<std::io::BufWriter<std::fs::File>>);

impl WavOut {
    pub fn create(path: &str) -> Result<Self, String> {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        hound::WavWriter::create(path, spec)
            .map(Self)
            .map_err(|e| e.to_string())
    }
    pub fn write(&mut self, s: &[(f32, f32)]) -> Result<(), String> {
        for &(l, r) in s {
            self.0
                .write_sample(l.clamp(-1.0, 1.0))
                .map_err(|e| e.to_string())?;
            self.0
                .write_sample(r.clamp(-1.0, 1.0))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn finish(self) -> Result<(), String> {
        self.0.finalize().map_err(|e| e.to_string())
    }
}
