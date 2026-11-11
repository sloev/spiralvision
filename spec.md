# SpiralVision-10 Protocol Specification (rev B)

## Overview
SpiralVision-10 carries 10 fps, 4:3 colour video in a 2-channel (stereo) audio signal that fits through a compact cassette (≈ 40 Hz – 14 kHz). Pixels are scanned along two interlaced spirals; luma is frequency-modulated, chroma is amplitude-modulated. Rev B adds a calibration chirp, guard samples, a uniform-density spiral and a flywheel decoder. Reference implementation: `crates/spiralvision-core`.

Rev B is **not** wire-compatible with rev A (the original 2-section frame).

## 1. System parameters
| | |
|---|---|
| Picture | 320 × 240, 10 fps |
| Audio | 48 000 Hz, 2 channels, float [-1, 1] — decoders must accept any rate and resample |
| Samples per frame | N = 4800 (100 ms) |
| Colour space | full-range BT.601 YUV, components in [0, 1] (U, V centred at 0.5) |
| Luma (FM) | f = F_MIN + Y·(F_MAX − F_MIN), F_MIN = 3 kHz, F_MAX = 12 kHz |
| Chroma (AM) | A = A_MIN + C·(A_MAX − A_MIN), A_MIN = 0.1, A_MAX = 1.0; left = U, right = V |

## 2. Frame layout
Phase is continuous per channel across the whole stream (no clicks).

| samples | section | content (both channels) |
|---|---|---|
| 0 – 239 | SYNC | 1 kHz sine, amplitude 1.0 |
| 240 – 335 | CAL-HI | linear chirp 2.5 → 13 kHz, amplitude 1.0 |
| 336 – 431 | CAL-LO | linear chirp 13 → 2.5 kHz, amplitude 0.35 |
| 432 – 455 | GUARD | repeats picture sample 0 |
| 456 – 4775 | PICTURE | 4320 samples, j = 0 … 4319 |
| 4776 – 4799 | GUARD | repeats picture sample 4319 |

Chirp sample c (0 ≤ c < 96) of each half sits at t = (c + 0.5)/96 of the sweep.

## 3. Spiral geometry
For picture sample j, progress p = (j + 0.5)/4320, normalised radius q = √p (Archimedean ⇒ constant line spacing and sample density):

```
θ  = K·q                      K = 80
a  = 1 + (W/H − 1)·q          aspect: circle → 4:3
n  = 2 + (14 − 2)·q           superellipse exponent: circle → rounded rectangle
rb = (|cos θ / a|^n + |sin θ|^n)^(−1/n)
s  = q · (H/2) · 1.03
u  = rb·cos θ·s,  v = rb·sin θ·s
arm A (left):  x = (W−1)/2 + u,  y = (H−1)/2 + v
arm B (right): x = (W−1)/2 − u,  y = (H−1)/2 − v      (arm A rotated by π)
```
Coordinates are clamped to the picture.

## 4. Encoder
1. Convert RGB → YUV; Gaussian pre-filter luma (σ = 1.2 px) and chroma (σ = 3 px) to match what the spiral can carry.
2. Emit SYNC and CAL sections.
3. For each picture sample (with guards): bilinear-sample Y, U at arm A and Y, V at arm B; f_L, f_R from luma; A_L from U, A_R from V; advance the phase accumulators by 2π f / 48000; output A·sin φ.

## 5. Decoder (reference)
1. **Resample** to 48 kHz.
2. **Demodulate** each channel with a 63-tap Hilbert transformer: z = x + j·H{x}; instantaneous frequency = arg(zₙ·z*ₙ₋₁)·fs/2π, amplitude = |z|; 5-tap Hann smoothing; delays compensated.
3. **Sync**: sliding 240-sample complex correlation with 1 kHz on both channels. ratio = 2(|S_L|² + |S_R|²) / (240·energy) > 0.55 for ≥ 48 samples ⇒ pulse. Position = centroid of the correlation peak; frequency offset from the phase advance between window halves gives a first speed estimate.
4. **Flywheel**: frames are rendered on a clock of period P (nominally 4800 samples). A sync within ±6 % (locked) / ±2 % (unlocked) of the predicted start snaps the phase and refines P from sync-to-sync intervals (speed s = 4800/P). Missing syncs decay the lock; an unlocked flywheel re-acquires on the next sync. Exactly one frame is produced per period whatever the input ⇒ constant frame rate.
5. **Calibration** (locked frames): average the demodulated amplitude of CAL-HI and CAL-LO in 8 bins *of measured frequency* (immune to channel group delay) ⇒ per-frequency gain G(f) and compression γ(f) = ln(L/G)/ln 0.35. Chroma is equalised as A = (a/G(f))^(1/γ(f)).
6. **Render**: for picture sample j read the demodulated stream at start + (456 + j)/s (twice per sample, interpolated), Y = (f/s − F_MIN)/(F_MAX − F_MIN), U/V from equalised amplitude. Gaussian-splat luma (σ = 2 px, both arms) and chroma (σ = 4.5 px, half-resolution grid, one arm each), normalise, push-pull fill holes, convert to RGB. Optional phosphor persistence.
7. **Noise mode** (unlocked frames): chroma shows loudness relative to the frame mean, 0.5 + 0.48·tanh(2·ln(a/ā)), so any audio becomes colourful spiral noise at the same 10 fps.
