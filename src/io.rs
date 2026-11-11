//! Sound card I/O via cpal. Any device rate/channel count is accepted:
//! output is resampled from 48 kHz, input is handed to the decoder at its
//! native rate (the decoder resamples internally).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use crossbeam_channel::{Receiver, Sender};
use spiralvision_core::dsp::Resampler;
use spiralvision_core::SAMPLE_RATE;
use std::collections::VecDeque;

/// Play 48 kHz stereo blocks from `rx`. Returns the stream (keep it alive).
fn build_out<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    rx: Receiver<Vec<(f32, f32)>>,
    mut rs: Option<Resampler>,
) -> Result<cpal::Stream, String> {
    let mut buf: VecDeque<(f32, f32)> = VecDeque::new();
    let mut tmp = Vec::new();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let frames = data.len() / channels;
                while buf.len() < frames {
                    match rx.try_recv() {
                        Ok(block) => {
                            if let Some(r) = &mut rs {
                                tmp.clear();
                                for s in block {
                                    r.push(s, &mut tmp);
                                }
                                buf.extend(tmp.iter().copied());
                            } else {
                                buf.extend(block);
                            }
                        }
                        Err(_) => break,
                    }
                }
                for f in data.chunks_mut(channels) {
                    let (l, r) = buf.pop_front().unwrap_or((0.0, 0.0));
                    for (i, x) in f.iter_mut().enumerate() {
                        let v = match i {
                            0 => l,
                            1 => r,
                            _ => 0.0,
                        };
                        *x = T::from_sample(v);
                    }
                }
            },
            |e| eprintln!("audio output error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

fn build_in<T: SizedSample>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    tx: Sender<Vec<(f32, f32)>>,
) -> Result<cpal::Stream, String>
where
    f32: FromSample<T>,
{
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let block = data
                    .chunks(channels)
                    .map(|c| {
                        let l: f32 = c[0].to_sample();
                        let r: f32 = if channels > 1 { c[1].to_sample() } else { l };
                        (l, r)
                    })
                    .collect();
                let _ = tx.try_send(block);
            },
            |e| eprintln!("audio input error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

pub fn start_audio_output(rx: Receiver<Vec<(f32, f32)>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no audio output device")?;
    let cfg = device.default_output_config().map_err(|e| e.to_string())?;
    let rate = cfg.sample_rate().0;
    let channels = cfg.channels() as usize;
    let fmt = cfg.sample_format();
    let config: cpal::StreamConfig = cfg.into();
    let rs = if rate != SAMPLE_RATE {
        Some(Resampler::new(SAMPLE_RATE as f32, rate as f32))
    } else {
        None
    };
    let stream = match fmt {
        cpal::SampleFormat::F32 => build_out::<f32>(&device, &config, channels, rx, rs),
        cpal::SampleFormat::I16 => build_out::<i16>(&device, &config, channels, rx, rs),
        cpal::SampleFormat::U16 => build_out::<u16>(&device, &config, channels, rx, rs),
        cpal::SampleFormat::I32 => build_out::<i32>(&device, &config, channels, rx, rs),
        f => return Err(format!("unsupported output format {f:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

/// Capture stereo blocks into `tx`. Returns (stream, sample rate).
pub fn start_audio_input(tx: Sender<Vec<(f32, f32)>>) -> Result<(cpal::Stream, u32), String> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("no audio input device")?;
    let cfg = device.default_input_config().map_err(|e| e.to_string())?;
    let rate = cfg.sample_rate().0;
    let channels = cfg.channels() as usize;
    let fmt = cfg.sample_format();
    let config: cpal::StreamConfig = cfg.into();
    let stream = match fmt {
        cpal::SampleFormat::F32 => build_in::<f32>(&device, &config, channels, tx),
        cpal::SampleFormat::I16 => build_in::<i16>(&device, &config, channels, tx),
        cpal::SampleFormat::U16 => build_in::<u16>(&device, &config, channels, tx),
        cpal::SampleFormat::I32 => build_in::<i32>(&device, &config, channels, tx),
        f => return Err(format!("unsupported input format {f:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, rate))
}
