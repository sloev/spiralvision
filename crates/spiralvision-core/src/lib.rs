//! SpiralVision-10 core: video <-> stereo audio over a morphing spiral.
//!
//! Pure Rust with no dependencies so the exact same DSP runs in the desktop
//! app, the CLI, the Raspberry Pi build and (via WASM) the web/PWA/Android app.

pub mod decoder;
pub mod dsp;
pub mod encoder;
pub mod image;
#[cfg(feature = "io")]
pub mod io;
pub mod protocol;
pub mod tape;

pub use decoder::{Decoder, DecoderParams};
pub use encoder::Encoder;
pub use protocol::{H, N, SAMPLE_RATE, W};
pub use tape::{TapeParams, TapeSim};

#[doc(hidden)]
pub fn tune(name: &str, default: f32) -> f32 {
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(v) = std::env::var(name) {
        return v.parse().unwrap_or(default);
    }
    let _ = name;
    default
}
