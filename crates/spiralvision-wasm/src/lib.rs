//! Minimal C ABI over `spiralvision-core` for WebAssembly (no wasm-bindgen,
//! so the build needs nothing but `rustup target add wasm32-unknown-unknown`).
//! The JS side lives in `docs/js/spiralvision.js`.
#![allow(clippy::missing_safety_doc)]

use spiralvision_core::protocol::{H, N, W};
use spiralvision_core::{Decoder, Encoder, TapeParams, TapeSim, SAMPLE_RATE};

#[no_mangle]
pub extern "C" fn sv_width() -> u32 {
    W as u32
}
#[no_mangle]
pub extern "C" fn sv_height() -> u32 {
    H as u32
}
#[no_mangle]
pub extern "C" fn sv_frame_samples() -> u32 {
    N as u32
}
#[no_mangle]
pub extern "C" fn sv_sample_rate() -> u32 {
    SAMPLE_RATE
}

// ---- scratch memory -------------------------------------------------------

#[no_mangle]
pub extern "C" fn sv_alloc(bytes: usize) -> *mut u8 {
    let mut v = vec![0u8; bytes];
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

#[no_mangle]
pub unsafe extern "C" fn sv_free(p: *mut u8, bytes: usize) {
    if !p.is_null() {
        drop(Vec::from_raw_parts(p, bytes, bytes));
    }
}

// ---- encoder --------------------------------------------------------------

pub struct EncState {
    enc: Encoder,
    out: Vec<f32>,
    pairs: Vec<(f32, f32)>,
}

#[no_mangle]
pub extern "C" fn enc_new() -> *mut EncState {
    Box::into_raw(Box::new(EncState {
        enc: Encoder::new(),
        out: vec![0.0; N * 2],
        pairs: Vec::with_capacity(N),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn enc_free(e: *mut EncState) {
    if !e.is_null() {
        drop(Box::from_raw(e));
    }
}

/// Encode one RGBA W x H frame. Returns a pointer to N interleaved stereo f32.
#[no_mangle]
pub unsafe extern "C" fn enc_frame(e: *mut EncState, rgba: *const u8) -> *const f32 {
    let e = &mut *e;
    let img = std::slice::from_raw_parts(rgba, W * H * 4);
    e.pairs.clear();
    e.enc.encode_into(img, 4, &mut e.pairs);
    for (i, &(l, r)) in e.pairs.iter().enumerate() {
        e.out[i * 2] = l;
        e.out[i * 2 + 1] = r;
    }
    e.out.as_ptr()
}

// ---- decoder --------------------------------------------------------------

pub struct DecState {
    dec: Decoder,
    rgba: Vec<u8>,
}

#[no_mangle]
pub extern "C" fn dec_new(sample_rate: f32) -> *mut DecState {
    Box::into_raw(Box::new(DecState {
        dec: Decoder::with_rate(sample_rate),
        rgba: vec![255; W * H * 4],
    }))
}

#[no_mangle]
pub unsafe extern "C" fn dec_free(d: *mut DecState) {
    if !d.is_null() {
        drop(Box::from_raw(d));
    }
}

/// Feed `frames` interleaved stereo samples. Returns the number of decoded
/// frames waiting.
#[no_mangle]
pub unsafe extern "C" fn dec_push(d: *mut DecState, data: *const f32, frames: usize) -> u32 {
    let d = &mut *d;
    let s = std::slice::from_raw_parts(data, frames * 2);
    d.dec.process_interleaved(s);
    d.dec.frames.len() as u32
}

/// Feed `frames` planar samples (WebAudio style: left, right buffers).
#[no_mangle]
pub unsafe extern "C" fn dec_push_planar(
    d: *mut DecState,
    l: *const f32,
    r: *const f32,
    frames: usize,
) -> u32 {
    let d = &mut *d;
    let l = std::slice::from_raw_parts(l, frames);
    let r = std::slice::from_raw_parts(r, frames);
    let pairs: Vec<(f32, f32)> = l.iter().copied().zip(r.iter().copied()).collect();
    d.dec.process_samples(&pairs);
    d.dec.frames.len() as u32
}

/// Pop the oldest frame as RGBA (W * H * 4), or null.
#[no_mangle]
pub unsafe extern "C" fn dec_pop(d: *mut DecState) -> *const u8 {
    let d = &mut *d;
    match d.dec.pop_frame() {
        Some(rgb) => {
            for i in 0..W * H {
                d.rgba[i * 4] = rgb[i * 3];
                d.rgba[i * 4 + 1] = rgb[i * 3 + 1];
                d.rgba[i * 4 + 2] = rgb[i * 3 + 2];
            }
            d.rgba.as_ptr()
        }
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn dec_set(d: *mut DecState, persistence: f32, saturation: f32) {
    let d = &mut *d;
    d.dec.params.persistence = persistence;
    d.dec.params.saturation = saturation;
}

#[no_mangle]
pub unsafe extern "C" fn dec_locked(d: *const DecState) -> u32 {
    (*d).dec.is_locked() as u32
}

#[no_mangle]
pub unsafe extern "C" fn dec_speed(d: *const DecState) -> f32 {
    (*d).dec.speed()
}

#[no_mangle]
pub unsafe extern "C" fn dec_frames_out(d: *const DecState) -> u32 {
    (*d).dec.frames_out as u32
}

// ---- tape simulator -------------------------------------------------------

pub struct TapeState {
    sim: TapeSim,
    out: Vec<f32>,
}

#[no_mangle]
pub extern "C" fn tape_new(wear: f32, seed: u32) -> *mut TapeState {
    Box::into_raw(Box::new(TapeState {
        sim: TapeSim::new(TapeParams::wear(wear), SAMPLE_RATE as f32, seed as u64),
        out: Vec::new(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn tape_free(t: *mut TapeState) {
    if !t.is_null() {
        drop(Box::from_raw(t));
    }
}

#[no_mangle]
pub unsafe extern "C" fn tape_set(t: *mut TapeState, wear: f32) {
    (*t).sim.p = TapeParams::wear(wear);
}

/// Run interleaved stereo through the tape. Returns output frame count;
/// fetch the samples with `tape_out`.
#[no_mangle]
pub unsafe extern "C" fn tape_process(t: *mut TapeState, data: *const f32, frames: usize) -> u32 {
    let t = &mut *t;
    let s = std::slice::from_raw_parts(data, frames * 2);
    let pairs: Vec<(f32, f32)> = s.chunks_exact(2).map(|c| (c[0], c[1])).collect();
    let o = t.sim.process(&pairs);
    t.out.clear();
    for (l, r) in o {
        t.out.push(l);
        t.out.push(r);
    }
    (t.out.len() / 2) as u32
}

#[no_mangle]
pub unsafe extern "C" fn tape_out(t: *const TapeState) -> *const f32 {
    (*t).out.as_ptr()
}
