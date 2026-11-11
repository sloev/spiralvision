//! SPIRALVISION-10 desktop deck (egui). All DSP lives in `spiralvision-core`.

use crate::io::{start_audio_input, start_audio_output};
use crossbeam_channel::{bounded, Receiver, Sender};
use eframe::egui::{self, Color32, RichText};
use parking_lot::Mutex;
use spiralvision_core::io::{camera_format, list_cameras, VideoReader, VideoWriter, WavOut};
use spiralvision_core::{Decoder, Encoder, TapeParams, TapeSim, H, SAMPLE_RATE, W};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const PINK: Color32 = Color32::from_rgb(255, 43, 214);
const CYAN: Color32 = Color32::from_rgb(0, 240, 255);
const GREEN: Color32 = Color32::from_rgb(57, 255, 20);
const YELLOW: Color32 = Color32::from_rgb(255, 230, 0);
const CHROME: Color32 = Color32::from_rgb(200, 205, 215);

#[derive(PartialEq, Clone, Copy)]
pub enum Mode {
    Encode,
    Decode,
    Loopback,
}

#[derive(PartialEq, Clone, Copy)]
enum VideoSrc {
    Camera,
    File,
}

#[derive(PartialEq, Clone, Copy)]
enum AudioPort {
    Device,
    File,
}

/// Knobs the worker reads live.
#[derive(Clone)]
struct Live {
    persistence: f32,
    wear: f32,
    saturation: f32,
}

#[derive(Default, Clone)]
struct Status {
    locked: bool,
    speed: f32,
    frames: u64,
    fps: f32,
    msg: String,
}

pub struct SpiralVisionApp {
    mode: Mode,
    vsrc: VideoSrc,
    camera: String,
    video_in: String,
    enc_out: AudioPort,
    enc_wav: String,
    dec_in: AudioPort,
    dec_wav: String,
    dec_save: bool,
    dec_out: String,
    dec_out_v4l2: bool,
    cameras: Vec<String>,
    live: Arc<Mutex<Live>>,
    status: Arc<Mutex<Status>>,
    stop: Option<Arc<AtomicBool>>,
    src_rx: Option<Receiver<Vec<u8>>>,
    out_rx: Option<Receiver<Vec<u8>>>,
    src_tex: Option<egui::TextureHandle>,
    out_tex: Option<egui::TextureHandle>,
    t0: Instant,
}

impl Default for SpiralVisionApp {
    fn default() -> Self {
        let cameras = list_cameras();
        Self {
            mode: Mode::Loopback,
            vsrc: VideoSrc::Camera,
            camera: cameras.first().cloned().unwrap_or_default(),
            video_in: String::new(),
            enc_out: AudioPort::Device,
            enc_wav: "spiralvision.wav".into(),
            dec_in: AudioPort::Device,
            dec_wav: "spiralvision.wav".into(),
            dec_save: false,
            dec_out: "decoded.mp4".into(),
            dec_out_v4l2: false,
            cameras,
            live: Arc::new(Mutex::new(Live {
                persistence: 0.0,
                wear: 0.0,
                saturation: 1.0,
            })),
            status: Arc::new(Mutex::new(Status::default())),
            stop: None,
            src_rx: None,
            out_rx: None,
            src_tex: None,
            out_tex: None,
            t0: Instant::now(),
        }
    }
}

pub fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(14, 6, 32);
    v.window_fill = Color32::from_rgb(20, 8, 44);
    v.extreme_bg_color = Color32::from_rgb(6, 2, 16);
    v.selection.bg_fill = PINK;
    v.selection.stroke.color = Color32::BLACK;
    v.hyperlink_color = CYAN;
    v.widgets.inactive.bg_fill = Color32::from_rgb(48, 20, 90);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(48, 20, 90);
    v.widgets.hovered.bg_fill = Color32::from_rgb(120, 20, 140);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(120, 20, 140);
    v.widgets.active.bg_fill = PINK;
    v.widgets.noninteractive.fg_stroke.color = CHROME;
    v.widgets.inactive.fg_stroke.color = Color32::WHITE;
    ctx.set_visuals(v);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    ctx.set_style(style);
}

impl eframe::App for SpiralVisionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let running = self.stop.is_some();
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                // spinning spiral logo
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
                let t = self.t0.elapsed().as_secs_f32();
                let p = ui.painter();
                let c = rect.center();
                let mut prev = c;
                for i in 0..120 {
                    let q = i as f32 / 120.0;
                    let a = q * 18.0 - t * 4.0;
                    let pt = c + egui::vec2(a.cos(), a.sin()) * q * 16.0;
                    let col = if i % 2 == 0 { PINK } else { CYAN };
                    p.line_segment([prev, pt], egui::Stroke::new(2.0_f32, col));
                    prev = pt;
                }
                ui.label(
                    RichText::new("SPIRALVISION-10")
                        .size(26.0)
                        .strong()
                        .color(PINK),
                );
                ui.label(
                    RichText::new("video on cassette, totally")
                        .italics()
                        .color(YELLOW),
                );
            });
            ui.horizontal(|ui| {
                for (m, label) in [
                    (Mode::Loopback, "⟲ LOOPBACK"),
                    (Mode::Encode, "⏺ ENCODE  (video > tape)"),
                    (Mode::Decode, "▶ DECODE  (tape > video)"),
                ] {
                    if ui
                        .selectable_label(self.mode == m, RichText::new(label).strong())
                        .clicked()
                        && self.mode != m
                    {
                        self.stop_worker();
                        self.mode = m;
                    }
                }
            });
            ui.add_space(4.0);
        });

        egui::SidePanel::left("controls")
            .min_width(300.0)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!running, |ui| self.io_controls(ui));
                ui.separator();
                ui.label(RichText::new("LIVE KNOBS").color(CYAN).strong());
                {
                    let mut l = self.live.lock();
                    if self.mode == Mode::Loopback {
                        ui.add(egui::Slider::new(&mut l.wear, 0.0..=2.0).text("tape wear"));
                    }
                    if self.mode != Mode::Encode {
                        ui.add(egui::Slider::new(&mut l.persistence, 0.0..=0.95).text("phosphor"));
                        ui.add(egui::Slider::new(&mut l.saturation, 0.0..=2.5).text("color"));
                    }
                }
                ui.separator();
                let big = |t: &str| RichText::new(t).size(20.0).strong();
                if !running {
                    if ui
                        .add(egui::Button::new(big("▶ PLAY").color(Color32::BLACK)).fill(GREEN))
                        .clicked()
                    {
                        self.start_worker();
                    }
                } else if ui
                    .add(
                        egui::Button::new(big("■ STOP").color(Color32::WHITE))
                            .fill(Color32::from_rgb(200, 0, 60)),
                    )
                    .clicked()
                {
                    self.stop_worker();
                }
                let st = self.status.lock().clone();
                ui.add_space(8.0);
                if running || st.frames > 0 {
                    let (txt, col) = if st.locked {
                        ("SYNC LOCKED", GREEN)
                    } else {
                        ("FREE-RUN / NOISE MODE", YELLOW)
                    };
                    if self.mode != Mode::Encode {
                        ui.label(RichText::new(txt).color(col).strong());
                        ui.label(format!("tape speed {:.1}%", st.speed * 100.0));
                    }
                    ui.label(format!("frames {}  •  {:.1} fps", st.frames, st.fps));
                }
                if !st.msg.is_empty() {
                    ui.label(RichText::new(&st.msg).color(PINK));
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(rx) = &self.src_rx {
                while let Ok(f) = rx.try_recv() {
                    self.src_tex = Some(ctx.load_texture(
                        "src",
                        egui::ColorImage::from_rgb([W, H], &f),
                        Default::default(),
                    ));
                }
            }
            if let Some(rx) = &self.out_rx {
                while let Ok(f) = rx.try_recv() {
                    self.out_tex = Some(ctx.load_texture(
                        "out",
                        egui::ColorImage::from_rgb([W, H], &f),
                        Default::default(),
                    ));
                }
            }
            let avail = ui.available_size();
            let both = self.mode != Mode::Decode;
            let n = if both { 2.0 } else { 1.0 };
            let w = ((avail.x - 20.0) / n)
                .min((avail.y - 40.0) * 4.0 / 3.0)
                .max(160.0);
            let size = egui::vec2(w, w * 0.75);
            ui.horizontal(|ui| {
                if both {
                    screen(ui, "SIDE A: SOURCE", &self.src_tex, size, CYAN);
                }
                screen(ui, "SIDE B: SPIRALVISION", &self.out_tex, size, PINK);
            });
        });

        if running || self.mode == Mode::Loopback {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
}

fn screen(
    ui: &mut egui::Ui,
    title: &str,
    tex: &Option<egui::TextureHandle>,
    size: egui::Vec2,
    col: Color32,
) {
    ui.vertical(|ui| {
        ui.label(RichText::new(title).color(col).strong());
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 6.0, Color32::BLACK);
        if let Some(t) = tex {
            p.image(
                t.id(),
                rect.shrink(4.0),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            // scanlines
            let mut y = rect.top();
            while y < rect.bottom() {
                p.line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    egui::Stroke::new(1.0_f32, Color32::from_black_alpha(40)),
                );
                y += 3.0;
            }
        } else {
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "NO SIGNAL",
                egui::FontId::proportional(28.0),
                Color32::from_gray(90),
            );
        }
        p.rect_stroke(rect, 6.0, egui::Stroke::new(3.0_f32, col));
    });
}

impl SpiralVisionApp {
    /// Open a video file in loopback mode and start playing.
    pub fn with_video(path: &str) -> Self {
        let mut a = Self::default();
        a.vsrc = VideoSrc::File;
        a.video_in = path.to_string();
        a.start_worker();
        a
    }

    fn io_controls(&mut self, ui: &mut egui::Ui) {
        if self.mode != Mode::Decode {
            ui.label(RichText::new("VIDEO IN").color(CYAN).strong());
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.vsrc, VideoSrc::Camera, "camera");
                ui.radio_value(&mut self.vsrc, VideoSrc::File, "file");
            });
            if self.vsrc == VideoSrc::Camera {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("cam")
                        .selected_text(&self.camera)
                        .show_ui(ui, |ui| {
                            for c in self.cameras.clone() {
                                ui.selectable_value(&mut self.camera, c.clone(), c);
                            }
                        });
                    if ui.small_button("⟳").clicked() {
                        self.cameras = list_cameras();
                    }
                });
                ui.text_edit_singleline(&mut self.camera);
            } else {
                file_row(
                    ui,
                    &mut self.video_in,
                    &["mp4", "mkv", "avi", "mov", "webm", "gif"],
                    false,
                );
            }
        }
        if self.mode == Mode::Encode {
            ui.label(RichText::new("TAPE OUT").color(CYAN).strong());
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.enc_out, AudioPort::Device, "line out / speakers");
                ui.radio_value(&mut self.enc_out, AudioPort::File, ".wav");
            });
            if self.enc_out == AudioPort::File {
                file_row(ui, &mut self.enc_wav, &["wav"], true);
            }
        }
        if self.mode == Mode::Decode {
            ui.label(RichText::new("TAPE IN").color(CYAN).strong());
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.dec_in, AudioPort::Device, "line in / mic");
                ui.radio_value(&mut self.dec_in, AudioPort::File, "audio file");
            });
            if self.dec_in == AudioPort::File {
                file_row(
                    ui,
                    &mut self.dec_wav,
                    &["wav", "mp3", "flac", "ogg", "m4a"],
                    false,
                );
            }
            ui.checkbox(&mut self.dec_save, "record decoded video");
            if self.dec_save {
                ui.checkbox(&mut self.dec_out_v4l2, "to v4l2 virtual webcam");
                file_row(ui, &mut self.dec_out, &["mp4", "mkv"], true);
            }
        }
        if self.mode == Mode::Loopback {
            ui.label(
                RichText::new("camera > encoder > tape sim > decoder")
                    .color(CHROME)
                    .italics(),
            );
        }
    }

    fn stop_worker(&mut self) {
        if let Some(s) = self.stop.take() {
            s.store(true, Ordering::Relaxed);
        }
    }

    fn start_worker(&mut self) {
        self.stop_worker();
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = Some(stop.clone());
        let (src_tx, src_rx) = bounded(2);
        let (out_tx, out_rx) = bounded(2);
        self.src_rx = Some(src_rx);
        self.out_rx = Some(out_rx);
        *self.status.lock() = Status::default();
        let live = self.live.clone();
        let status = self.status.clone();
        let job = Job {
            mode: self.mode,
            video: if self.vsrc == VideoSrc::Camera {
                (self.camera.clone(), Some(camera_format()))
            } else {
                (self.video_in.clone(), None)
            },
            enc_wav: (self.enc_out == AudioPort::File).then(|| self.enc_wav.clone()),
            dec_wav: (self.dec_in == AudioPort::File).then(|| self.dec_wav.clone()),
            dec_out: self
                .dec_save
                .then(|| (self.dec_out.clone(), self.dec_out_v4l2)),
        };
        thread::spawn(move || {
            let st = status.clone();
            if let Err(e) = job.run(stop, live, status, src_tx, out_tx) {
                st.lock().msg = e;
            }
        });
    }
}

fn file_row(ui: &mut egui::Ui, path: &mut String, exts: &[&str], save: bool) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(path).desired_width(190.0));
        if ui.button("…").clicked() {
            let d = rfd::FileDialog::new().add_filter("media", exts);
            let r = if save { d.save_file() } else { d.pick_file() };
            if let Some(p) = r {
                *path = p.to_string_lossy().to_string();
            }
        }
    });
}

struct Job {
    mode: Mode,
    video: (String, Option<&'static str>),
    enc_wav: Option<String>,
    dec_wav: Option<String>,
    dec_out: Option<(String, bool)>,
}

struct Pace {
    t0: Instant,
    n: u64,
    last: Instant,
    fps: f32,
}

impl Pace {
    fn new() -> Self {
        Self {
            t0: Instant::now(),
            n: 0,
            last: Instant::now(),
            fps: 0.0,
        }
    }
    /// Sleep so frames leave at exactly 10 fps.
    fn wait(&mut self) {
        self.n += 1;
        let due = self.t0 + Duration::from_millis(100 * self.n);
        let now = Instant::now();
        if due > now {
            thread::sleep(due - now);
        } else if now - due > Duration::from_millis(500) {
            self.t0 = now - Duration::from_millis(100 * self.n);
        }
        self.tick();
    }
    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().max(1e-3);
        self.last = now;
        self.fps = if self.fps == 0.0 {
            1.0 / dt
        } else {
            self.fps * 0.9 + 0.1 / dt
        };
    }
}

impl Job {
    fn run(
        self,
        stop: Arc<AtomicBool>,
        live: Arc<Mutex<Live>>,
        status: Arc<Mutex<Status>>,
        src_tx: Sender<Vec<u8>>,
        out_tx: Sender<Vec<u8>>,
    ) -> Result<(), String> {
        let apply = |dec: &mut Decoder| {
            let l = live.lock();
            dec.params.persistence = l.persistence;
            dec.params.saturation = l.saturation;
        };
        let report = |dec: &Decoder, fps: f32| {
            let mut s = status.lock();
            s.locked = dec.is_locked();
            s.speed = dec.speed();
            s.frames = dec.frames_out;
            s.fps = fps;
        };
        match self.mode {
            Mode::Encode | Mode::Loopback => {
                let (input, fmt) = &self.video;
                if input.is_empty() {
                    return Err("pick a video source first".into());
                }
                let mut rd = VideoReader::open(input, *fmt, None)
                    .map_err(|e| format!("ffmpeg: {e} (is ffmpeg installed?)"))?;
                let mut enc = Encoder::new();
                let mut dec = Decoder::new();
                let mut wear = -1.0;
                let mut tape = TapeSim::new(TapeParams::wear(0.0), SAMPLE_RATE as f32, 1);
                let mut wav = match &self.enc_wav {
                    Some(p) if self.mode == Mode::Encode => Some(WavOut::create(p)?),
                    _ => None,
                };
                let (atx, arx) = bounded::<Vec<(f32, f32)>>(4);
                let _stream = if self.mode == Mode::Encode && wav.is_none() {
                    Some(start_audio_output(arx)?)
                } else {
                    None
                };
                let live_src = fmt.is_some();
                let mut pace = Pace::new();
                while !stop.load(Ordering::Relaxed) {
                    let Some(frame) = rd.next_frame() else { break };
                    let _ = src_tx.try_send(frame.clone());
                    let mut audio = enc.encode_frame(&frame);
                    if self.mode == Mode::Loopback {
                        let w = live.lock().wear;
                        if (w - wear).abs() > 1e-3 {
                            wear = w;
                            tape.p = TapeParams::wear(w);
                        }
                        if w > 0.0 {
                            audio = tape.process(&audio);
                        }
                    }
                    apply(&mut dec);
                    dec.process_samples(&audio);
                    while let Some(f) = dec.pop_frame() {
                        let _ = out_tx.try_send(f);
                    }
                    if let Some(w) = &mut wav {
                        w.write(&audio)?;
                        pace.tick();
                    } else if _stream.is_some() {
                        // the bounded channel blocks at the sound card's pace
                        if atx.send(audio).is_err() {
                            break;
                        }
                        pace.tick();
                    } else if live_src {
                        pace.tick();
                    } else {
                        pace.wait();
                    }
                    report(&dec, pace.fps);
                }
                if let Some(w) = wav {
                    w.finish()?;
                    status.lock().msg = "tape saved. rewind & play!".into();
                }
            }
            Mode::Decode => {
                let mut writer = match &self.dec_out {
                    Some((p, v4l2)) => Some(
                        VideoWriter::open(p, v4l2.then_some("v4l2"), None)
                            .map_err(|e| format!("ffmpeg: {e}"))?,
                    ),
                    None => None,
                };
                let mut pace = Pace::new();
                let mut emit =
                    |dec: &mut Decoder, pace: &mut Pace, realtime: bool| -> Result<(), String> {
                        while let Some(f) = dec.pop_frame() {
                            if let Some(w) = &mut writer {
                                w.write(&f).map_err(|e| e.to_string())?;
                            }
                            let _ = out_tx.try_send(f);
                            if realtime {
                                pace.wait();
                            } else {
                                pace.tick();
                            }
                            report(dec, pace.fps);
                        }
                        Ok(())
                    };
                if let Some(path) = &self.dec_wav {
                    let (audio, rate) = spiralvision_core::io::read_audio(path)?;
                    let mut dec = Decoder::with_rate(rate as f32);
                    for chunk in audio.chunks(rate as usize / 10) {
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                        apply(&mut dec);
                        dec.process_samples(chunk);
                        emit(&mut dec, &mut pace, true)?;
                    }
                    dec.flush();
                    emit(&mut dec, &mut pace, true)?;
                } else {
                    let (tx, rx) = bounded(64);
                    let (_stream, rate) = start_audio_input(tx)?;
                    let mut dec = Decoder::with_rate(rate as f32);
                    while !stop.load(Ordering::Relaxed) {
                        if let Ok(block) = rx.recv_timeout(Duration::from_millis(200)) {
                            apply(&mut dec);
                            dec.process_samples(&block);
                            emit(&mut dec, &mut pace, false)?;
                        }
                    }
                }
                if let Some(w) = writer {
                    w.finish().map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
}
