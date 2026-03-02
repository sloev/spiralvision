use eframe::egui;
use std::thread;
use std::io::{Read, Write};
use crossbeam_channel::{unbounded, Sender, Receiver};
use crate::protocol::{W, H};
use crate::encoder::Encoder;
use crate::decoder::Decoder;
use crate::io::{start_ffmpeg_reader, start_ffmpeg_writer, start_audio_output, start_audio_input};


#[derive(PartialEq)]
pub enum Mode {
    Encoder,
    Decoder,
}

pub struct SpiralVisionApp {
    mode: Mode,
    
    // Encoder settings
    enc_input_path: String,
    enc_is_device: bool,
    enc_output_path: String,
    enc_audio_file: bool,
    
    // Decoder settings
    dec_input_path: String,
    dec_audio_file: bool,
    dec_output_path: String,
    dec_is_device: bool,
    
    is_running: bool,
    
    // Device lists
    video_devices: Vec<String>,
    
    // Preview image
    preview_tex: Option<egui::TextureHandle>,
    preview_rx: Option<Receiver<Vec<u8>>>,
    
    // Control channel
    stop_tx: Option<Sender<()>>,
}

impl Default for SpiralVisionApp {
    fn default() -> Self {
        let video_devices = Self::list_video_devices();
        let default_video = video_devices.first().cloned().unwrap_or("/dev/video0".to_string());
        
        Self {
            mode: Mode::Encoder,
            enc_input_path: default_video.clone(),
            enc_is_device: true,
            enc_output_path: "output.wav".to_string(),
            enc_audio_file: true,
            
            dec_input_path: "output.wav".to_string(),
            dec_audio_file: true,
            dec_output_path: "/dev/video10".to_string(),
            dec_is_device: true,
            
            is_running: false,
            video_devices,
            preview_tex: None,
            preview_rx: None,
            stop_tx: None,
        }
    }
}

impl eframe::App for SpiralVisionApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_value(&mut self.mode, Mode::Encoder, "Encoder Mode").changed() {
                    self.stop_process();
                }
                if ui.selectable_value(&mut self.mode, Mode::Decoder, "Decoder Mode").changed() {
                    self.stop_process();
                }
                
                if ui.button("Refresh Devices").clicked() {
                    self.video_devices = Self::list_video_devices();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.mode {
                Mode::Encoder => {
                    ui.heading("Encoder (Video to Audio)");
                    ui.horizontal(|ui| {
                        if ui.radio_value(&mut self.enc_is_device, true, "Video Device").changed() {
                            self.stop_process();
                        }
                        if ui.radio_value(&mut self.enc_is_device, false, "Video File").changed() {
                            self.stop_process();
                        }
                    });
                    
                    if self.enc_is_device {
                        let mut changed = false;
                        egui::ComboBox::from_id_salt("enc_vid_src")
                            .selected_text(&self.enc_input_path)
                            .show_ui(ui, |ui| {
                                for dev in &self.video_devices {
                                    if ui.selectable_value(&mut self.enc_input_path, dev.clone(), dev).changed() {
                                        changed = true;
                                    }
                                }
                            });
                        if changed {
                            self.stop_process();
                        }
                    } else {
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(&mut self.enc_input_path).changed() {
                                self.stop_process();
                            }
                            if ui.button("Browse...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("video", &["mp4", "mkv", "avi", "mov", "webm"])
                                    .pick_file() {
                                    self.enc_input_path = path.to_string_lossy().to_string();
                                    self.stop_process();
                                }
                            }
                        });
                    }
                    
                    ui.horizontal(|ui| {
                        ui.label("Output:");
                        if ui.radio_value(&mut self.enc_audio_file, false, "Audio Output Device").changed() {
                            self.stop_process();
                        }
                        if ui.radio_value(&mut self.enc_audio_file, true, "Audio File (.wav)").changed() {
                            self.stop_process();
                        }
                    });
                    
                    if self.enc_audio_file {
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(&mut self.enc_output_path).changed() {
                                self.stop_process();
                            }
                            if ui.button("Save As...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("audio", &["wav"])
                                    .save_file() {
                                    self.enc_output_path = path.to_string_lossy().to_string();
                                    self.stop_process();
                                }
                            }
                        });
                    } else {
                        if ui.text_edit_singleline(&mut self.enc_output_path).changed() {
                            self.stop_process();
                        }
                    }
                }
                Mode::Decoder => {
                    ui.heading("Decoder (Audio to Video)");
                    ui.horizontal(|ui| {
                        ui.label("Input:");
                        if ui.radio_value(&mut self.dec_audio_file, false, "Audio Input Device").changed() {
                            self.stop_process();
                        }
                        if ui.radio_value(&mut self.dec_audio_file, true, "Audio File (.wav)").changed() {
                            self.stop_process();
                        }
                    });
                    
                    if self.dec_audio_file {
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(&mut self.dec_input_path).changed() {
                                self.stop_process();
                            }
                            if ui.button("Browse...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("audio", &["wav"])
                                    .pick_file() {
                                    self.dec_input_path = path.to_string_lossy().to_string();
                                    self.stop_process();
                                }
                            }
                        });
                    } else {
                        if ui.text_edit_singleline(&mut self.dec_input_path).changed() {
                            self.stop_process();
                        }
                    }
                    
                    ui.horizontal(|ui| {
                        ui.label("Output:");
                        if ui.radio_value(&mut self.dec_is_device, true, "Virtual Video Device").changed() {
                            self.stop_process();
                        }
                        if ui.radio_value(&mut self.dec_is_device, false, "Video File").changed() {
                            self.stop_process();
                        }
                    });
                    
                    if self.dec_is_device {
                        let mut changed = false;
                        egui::ComboBox::from_id_salt("dec_vid_out")
                            .selected_text(&self.dec_output_path)
                            .show_ui(ui, |ui| {
                                for dev in &self.video_devices {
                                    if ui.selectable_value(&mut self.dec_output_path, dev.clone(), dev).changed() {
                                        changed = true;
                                    }
                                }
                            });
                        if changed {
                            self.stop_process();
                        }
                    } else {
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(&mut self.dec_output_path).changed() {
                                self.stop_process();
                            }
                            if ui.button("Save As...").clicked() {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("video", &["mp4", "mkv", "avi"])
                                    .save_file() {
                                    self.dec_output_path = path.to_string_lossy().to_string();
                                    self.stop_process();
                                }
                            }
                        });
                    }
                }
            }
            
            ui.add_space(20.0);
            
            if !self.is_running {
                if ui.button("Start").clicked() {
                    self.start_process();
                }
            } else {
                if ui.button("Stop").clicked() {
                    self.stop_process();
                }
            }
            
            ui.add_space(20.0);
            ui.label("Preview:");
            
            // Check for new preview frames
            if let Some(rx) = &self.preview_rx {
                while let Ok(frame) = rx.try_recv() {
                    let image = egui::ColorImage::from_rgb([W, H], &frame);
                    self.preview_tex = Some(ctx.load_texture("preview", image, Default::default()));
                }
            }
            
            if let Some(tex) = &self.preview_tex {
                ui.add(egui::Image::new(tex).shrink_to_fit());
            }
            
            // Request repaint if running to ensure smooth preview updates
            if self.is_running {
                ctx.request_repaint();
            }
        });
    }
}

impl SpiralVisionApp {
    fn list_video_devices() -> Vec<String> {
        let mut devices = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/dev") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("video") {
                    devices.push(format!("/dev/{}", name));
                }
            }
        }
        devices.sort();
        devices
    }

    fn stop_process(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        self.is_running = false;
        self.preview_rx = None;
    }
    
    fn start_process(&mut self) {
        self.stop_process();
        let (stop_tx, stop_rx) = unbounded();
        self.stop_tx = Some(stop_tx);
        let (preview_tx, preview_rx) = unbounded();
        self.preview_rx = Some(preview_rx);
        self.is_running = true;
        
        if self.mode == Mode::Encoder {
            let input_path = self.enc_input_path.clone();
            let is_device = self.enc_is_device;
            let output_path = self.enc_output_path.clone();
            let is_audio_file = self.enc_audio_file;
            
            thread::spawn(move || {
                let mut reader = start_ffmpeg_reader(&input_path, is_device).expect("Failed to start ffmpeg");
                let mut stdout = reader.stdout.take().unwrap();
                
                let (audio_tx, audio_rx) = unbounded();
                
                let _stream = if !is_audio_file {
                    start_audio_output(audio_rx)
                } else {
                    None // Handled differently if file
                };
                
                let mut wav_writer = if is_audio_file {
                    let spec = hound::WavSpec {
                        channels: 2,
                        sample_rate: crate::protocol::AUDIO_RATE,
                        bits_per_sample: 32,
                        sample_format: hound::SampleFormat::Float,
                    };
                    Some(hound::WavWriter::create(&output_path, spec).unwrap())
                } else {
                    None
                };
                
                let mut encoder = Encoder::new();
                let mut decoder = Decoder::new(); // For preview
                
                let mut frame_buf = vec![0u8; W * H * 3];
                loop {
                    if stop_rx.try_recv().is_ok() { break; }
                    
                    if stdout.read_exact(&mut frame_buf).is_err() {
                        break;
                    }
                    
                    let audio_samples = encoder.encode_frame(&frame_buf);
                    
                    if let Some(writer) = &mut wav_writer {
                        for &(l, r) in &audio_samples {
                            writer.write_sample(l).unwrap();
                            writer.write_sample(r).unwrap();
                        }
                    } else {
                        let _ = audio_tx.send(audio_samples.clone());
                    }
                    
                    // Decode for preview
                    decoder.process_samples(&audio_samples);
                    while let Some(decoded) = decoder.frames.pop() {
                        println!("Encoder mode: decoded a preview frame!");
                        let _ = preview_tx.send(decoded);
                    }
                }
                
                let _ = reader.kill();
            });
            
        } else {
            // Decoder
            let input_path = self.dec_input_path.clone();
            let is_audio_file = self.dec_audio_file;
            let output_path = self.dec_output_path.clone();
            let is_device = self.dec_is_device;
            
            thread::spawn(move || {
                let mut writer = start_ffmpeg_writer(&output_path, is_device).expect("Failed to start ffmpeg writer");
                let mut stdin = writer.stdin.take().unwrap();
                
                let (audio_tx, audio_rx) = unbounded();
                
                let _stream = if !is_audio_file {
                    start_audio_input(audio_tx.clone())
                } else {
                    None
                };
                
                let mut wav_reader = if is_audio_file {
                    Some(hound::WavReader::open(&input_path).unwrap())
                } else {
                    None
                };
                
                let mut decoder = Decoder::new();
                
                loop {
                    if stop_rx.try_recv().is_ok() { break; }
                    
                    let mut samples = Vec::new();
                    
                    if let Some(reader) = &mut wav_reader {
                        let spec = reader.spec();
                        if spec.sample_format == hound::SampleFormat::Float {
                            let mut iter = reader.samples::<f32>();
                            for _ in 0..4800 {
                                if let (Some(Ok(l)), Some(Ok(r))) = (iter.next(), iter.next()) {
                                    samples.push((l, r));
                                } else { break; }
                            }
                        } else if spec.bits_per_sample == 24 {
                            let mut iter = reader.samples::<i32>();
                            for _ in 0..4800 {
                                if let (Some(Ok(l)), Some(Ok(r))) = (iter.next(), iter.next()) {
                                    samples.push((l as f32 / 8388607.0, r as f32 / 8388607.0));
                                } else { break; }
                            }
                        } else if spec.bits_per_sample == 16 {
                            let mut iter = reader.samples::<i16>();
                            for _ in 0..4800 {
                                if let (Some(Ok(l)), Some(Ok(r))) = (iter.next(), iter.next()) {
                                    samples.push((l as f32 / 32767.0, r as f32 / 32767.0));
                                } else { break; }
                            }
                        } else {
                            eprintln!("Unsupported bit depth: {}", spec.bits_per_sample);
                            break;
                        }
                        if samples.is_empty() { break; }
                        // Remove sleep to process file as fast as possible for offline decoding
                    } else {
                        if let Ok(incoming) = audio_rx.try_recv() {
                            samples = incoming;
                        } else {
                            // If we are reading from live audio, block until we have data instead of busy waiting
                            if let Ok(incoming) = audio_rx.recv() {
                                samples = incoming;
                            } else {
                                break;
                            }
                        }
                    }
                    
                    decoder.process_samples(&samples);
                    
                    while let Some(decoded) = decoder.frames.pop() {
                        let _ = preview_tx.send(decoded.clone());
                        let _ = stdin.write_all(&decoded);
                    }
                }
                let _ = writer.kill();
            });
        }
    }
}
