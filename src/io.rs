use std::process::{Command, Stdio, ChildStdout};
use std::io::Read;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Sender, Receiver};
use crate::protocol::{W, H, AUDIO_RATE};

pub fn start_ffmpeg_reader(input_path: &str, is_device: bool) -> Option<std::process::Child> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-hide_banner").arg("-loglevel").arg("error");
    
    if is_device {
        cmd.arg("-f").arg("v4l2");
    }
    
    cmd.arg("-i").arg(input_path)
       .arg("-f").arg("rawvideo")
       .arg("-pix_fmt").arg("rgb24")
       .arg("-s").arg(format!("{}x{}", W, H))
       .arg("-r").arg("10")
       .arg("-")
       .stdout(Stdio::piped())
       .spawn()
       .ok()
}

pub fn start_ffmpeg_writer(output_path: &str, is_device: bool) -> Option<std::process::Child> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-hide_banner").arg("-loglevel").arg("error")
       .arg("-f").arg("rawvideo")
       .arg("-pix_fmt").arg("rgb24")
       .arg("-s").arg(format!("{}x{}", W, H))
       .arg("-r").arg("10")
       .arg("-i").arg("-");
       
    if is_device {
        cmd.arg("-f").arg("v4l2").arg(output_path);
    } else {
        cmd.arg("-y").arg(output_path);
    }
    
    cmd.stdin(Stdio::piped())
       .spawn()
       .ok()
}

pub fn start_audio_output(rx: Receiver<Vec<(f32, f32)>>) -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_output_device()?;
    
    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(AUDIO_RATE),
        buffer_size: cpal::BufferSize::Default,
    };
    
    let mut buffer = Vec::new();
    
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut i = 0;
            while i < data.len() {
                if buffer.is_empty() {
                    if let Ok(new_data) = rx.try_recv() {
                        buffer.extend(new_data);
                    } else {
                        // Fill with silence if no data
                        for j in i..data.len() {
                            data[j] = 0.0;
                        }
                        break;
                    }
                }
                
                let to_take = std::cmp::min((data.len() - i) / 2, buffer.len());
                for _ in 0..to_take {
                    let (l, r) = buffer.remove(0);
                    data[i] = l;
                    data[i+1] = r;
                    i += 2;
                }
            }
        },
        |err| eprintln!("Audio output error: {}", err),
        None
    ).ok()?;
    
    stream.play().ok()?;
    Some(stream)
}

pub fn start_audio_input(tx: Sender<Vec<(f32, f32)>>) -> Option<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_input_device()?;
    
    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: cpal::SampleRate(AUDIO_RATE),
        buffer_size: cpal::BufferSize::Default,
    };
    
    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut buf = Vec::with_capacity(data.len() / 2);
            for chunk in data.chunks_exact(2) {
                buf.push((chunk[0], chunk[1]));
            }
            let _ = tx.send(buf);
        },
        |err| eprintln!("Audio input error: {}", err),
        None
    ).ok()?;
    
    stream.play().ok()?;
    Some(stream)
}
