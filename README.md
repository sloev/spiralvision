# SpiraVision-10

SpiraVision-10 is a highly lossy, analog-oriented video protocol designed to encode 10 fps, 4:3 color video over a 2-channel (stereo) audio waveform. It is designed to be precise and performant, capable of running on low-power hardware like a Raspberry Pi.

## Prerequisites (Ubuntu/Debian)

To run SpiraVision-10, you need `ffmpeg` for video processing and some system libraries for audio and UI.

```bash
# Update and install system dependencies
sudo apt update
sudo apt install -y ffmpeg libasound2-dev libwayland-dev libxkbcommon-dev libegl1-mesa-dev libfontconfig1-dev
```

### Virtual Webcam (Optional)
If you want to use the Decoder Mode to output to a virtual webcam, install `v4l2loopback`:

```bash
sudo apt install -y v4l2loopback-dkms
sudo modprobe v4l2loopback devices=1 video_nr=10 card_label="SpiraVision" exclusive_caps=1
```
This will create `/dev/video10` which you can use as an output in Decoder Mode.

## Installation

1. **Install Rust:**
   If you don't have Rust installed, get it from [rustup.rs](https://rustup.rs/):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clone and Build:**
   ```bash
   # From the project root
   cargo build --release
   ```

## Running the Application

For best performance, always run in release mode:

```bash
cargo run --release
```

### Usage
- **Encoder Mode:** Convert a video file or live camera feed into SpiraVision audio. You can output to your speakers or save to a `.wav` file.
- **Decoder Mode:** Convert SpiraVision audio (from a file or microphone) back into video. You can output to a video file or a virtual webcam.
- **Preview:** Both modes provide a real-time preview of the signal processing.
