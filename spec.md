# SpiralVision-10: Protocol Specification

## Overview
SpiralVision-10 is a highly lossy, analog-oriented video protocol designed to encode 10 fps, 4:3 color video over a 2-channel (stereo) audio waveform bounded by a 15 kHz frequency limit. It utilizes a dual-interlaced, dynamically morphing superellipse geometry, multiplexing Luma via Frequency Modulation (FM) and Chroma via Amplitude Modulation (AM).

## 1. System Parameters
* **Target Framerate:** 10 fps (T_frame = 0.1 seconds)
* **Audio Base Sample Rate:** 48,000 Hz standard (4,800 samples per frame)
* **Audio Channels:** 2 (Left and Right)
* **Color Space:** YUV (Luma Y, Chroma U, V), normalized to [0.0, 1.0]
* **Frequency Allocation:**
    * Sync Pulse: 1,000 Hz
    * FM Luma Minimum (Black): f_min = 3,000 Hz
    * FM Luma Maximum (White): f_max = 12,000 Hz
* **Amplitude Allocation:**
    * AM Chroma Minimum (0.0): A_min = 0.1 (To preserve the FM carrier for zero-crossing detection)
    * AM Chroma Maximum (1.0): A_max = 1.0

## 2. Spatial Geometry: The Morphing Superellipse
To map a 2D image matrix into a 1D continuous array of samples, SpiralVision-10 uses a center-outward spiral that begins as a circle (1:1 aspect ratio) and morphs into a rectangle (4:3 aspect ratio) at the outer bounds, perfectly filling the display without wasting coordinate mapping on non-visible regions.

Let W be image width, H be image height, and N be total samples per frame (4800).
For a given sample index i in [0, N-1], define normalized progress p = i / N.

**Shape Variables:**
1. Angle (theta): theta = K * sqrt(p) 
   *(where K determines winding density, typically ~150)*
2. Aspect Ratio (a): a(p) = 1.0 + ((W / H) - 1.0) * p
3. Superellipse Exponent (n): n(p) = 2.0 + (8.0 * p)

**Radius Calculation:**
The base radius equation for the superellipse is defined as:
r_base = ( |cos(theta) / a(p)|^n(p) + |sin(theta)|^n(p) )^(-1 / n(p))

The scaled pixel radius is:
R = p * (H / 2) * r_base

**Dual-Interlaced Coordinates (Left and Right Tracks):**
Because the left and right audio channels are sampled simultaneously, they map to two separate spirals 180 degrees (PI radians) out of phase.

* **Spiral A (Left Track) Pixel [X_A, Y_A]:**
  X_A(p) = (W / 2) + R * a(p) * cos(theta)
  Y_A(p) = (H / 2) + R * sin(theta)

* **Spiral B (Right Track) Pixel [X_B, Y_B]:**
  X_B(p) = (W / 2) + R * a(p) * cos(theta + PI)
  Y_B(p) = (H / 2) + R * sin(theta + PI)

*(Note: Calculated coordinates should be clamped to 0 <= X < W and 0 <= Y < H and rounded to nearest integers).*

## 3. Signal Multiplexing (FM + AM)
SpiralVision-10 encodes detail (Luma) into the phase/frequency to survive tape saturation, and color (Chroma) into the amplitude to allow graceful analog decay.

For each audio sample t, extract the source pixels at [X_A, Y_A] and [X_B, Y_B]. Convert these pixels to YUV_A and YUV_B, scaled to [0.0, 1.0].

**1. FM Luma Calculation:**
Calculate the instantaneous frequency for both spirals:
f_A(t) = f_min + Y_A * (f_max - f_min)
f_B(t) = f_min + Y_B * (f_max - f_min)

Calculate the continuous phase integral (to prevent waveform clicking/discontinuities):
Phi_A(t) = 2 * PI * Integral(0 to t) of f_A(tau) d_tau
Phi_B(t) = 2 * PI * Integral(0 to t) of f_B(tau) d_tau

**2. AM Chroma Calculation:**
Map the U and V color channels to the amplitude envelope. (To prevent the audio wave from flattening to 0V, A must never drop below 0.1).
Env_A(t) = A_min + U_A * (A_max - A_min)
Env_B(t) = A_min + V_B * (A_max - A_min)

**3. Final Waveform Assembly:**
Signal_Left(t) = Env_A(t) * sin(Phi_A(t))
Signal_Right(t) = Env_B(t) * sin(Phi_B(t))

## 4. Synchronization Protocol
To signal the start of a new frame and force the decoder to reset p=0, an absolute sync pulse is prepended to the beginning of every frame's audio data.

* **Duration:** First 5 ms of the frame (240 samples at 48 kHz).
* **Waveform:** Pure 1,000 Hz sine wave at 1.0 amplitude on BOTH channels.
* **Constraint:** During this 5 ms window, no image data is encoded. The coordinate mapping p calculation effectively begins after this pulse.

## 5. Encoder Implementation Logic
**Input:** Sequence of RGB image matrices (W x H) at 10 fps.
**Output:** Continuous Stereo Float32 audio array [-1.0, 1.0].

1. **Initialize:** Set a global phase accumulator for Left and Right channels to 0.0.
2. **For each Image Frame:**
   * Generate a 5 ms sync pulse (1000 Hz) on both channels and append to the output stream.
   * Convert the RGB image to YUV space.
   * For sample i from 240 to 4799:
     * Calculate geometry X_A, Y_A and X_B, Y_B.
     * Sample the YUV values at those coordinates.
     * Calculate instantaneous frequency f_A, f_B. Add to the global phase accumulators.
     * Calculate envelopes Env_A, Env_B.
     * Generate the specific audio sample values using the phase and envelope.
     * Append the stereo sample to the output stream.

## 6. Decoder Implementation Logic
**Input:** Continuous Stereo audio array (stream or file).
**Output:** Sequence of RGB image matrices (W x H) at 10 fps.

1. **Sync Detection:** Monitor the Left channel. Apply a narrow bandpass filter at 1,000 Hz. When a threshold is met for >3 ms, flag "Frame Sync" and initialize an empty black image matrix.
2. **Sample Processing:** For incoming samples following the sync pulse:
   * Calculate X_A, Y_A and X_B, Y_B based on the sample index i since the sync pulse.
   * **Demodulate Luma (FM):** Use a Zero-Crossing Detector or Hilbert Transform to measure the instantaneous frequency of the Left and Right signals. Map frequencies [3000, 12000] to Luma [0.0, 1.0].
   * **Demodulate Chroma (AM):** Run the audio signal through a digital absolute value function, followed by a low-pass filter (Leaky Integrator) to extract the amplitude envelope. Map amplitudes [0.1, 1.0] to U and V [0.0, 1.0].
   * Write the resulting YUV values to the image matrix at the computed coordinates.
3. **Frame Completion:** Upon receiving the next 1,000 Hz sync pulse, optionally apply a rapid morphological dilation to the image matrix to fill the microscopic gaps between the spiral lines, convert YUV back to RGB, and emit the finished frame.