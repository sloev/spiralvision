//! Sound card I/O via cpal. Any device rate/channel count is accepted:
//! output is resampled from 48 kHz, input is handed to the decoder at its
//! native rate (the decoder resamples internally).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, Sender};
use spiralvision_core::dsp::Resampler;
use spiralvision_core::SAMPLE_RATE;
use std::collections::VecDeque;

/// Play 48 kHz stereo blocks from `rx`. Returns the stream (keep it alive).
pub fn start_audio_output(rx: Receiver<Vec<(f32, f32)>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("no audio output device")?;
    let cfg = device.default_output_config().map_err(|e| e.to_string())?;
    let rate = cfg.sample_rate().0;
    let channels = cfg.channels() as usize;
    let config: cpal::StreamConfig = cfg.into();
    let mut rs = if rate != SAMPLE_RATE {
        Some(Resampler::new(SAMPLE_RATE as f32, rate as f32))
    } else {
        None
    };
    let mut buf: VecDeque<(f32, f32)> = VecDeque::new();
    let mut tmp = Vec::new();
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _| {
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
                    f[0] = l;
                    if channels > 1 {
                        f[1] = r;
                    }
                    for x in f.iter_mut().skip(2) {
                        *x = 0.0;
                    }
                }
            },
            |e| eprintln!("audio output error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
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
    let config: cpal::StreamConfig = cfg.into();
    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _| {
                let block = data
                    .chunks(channels)
                    .map(|c| {
                        if channels > 1 {
                            (c[0], c[1])
                        } else {
                            (c[0], c[0])
                        }
                    })
                    .collect();
                let _ = tx.try_send(block);
            },
            |e| eprintln!("audio input error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, rate))
}
