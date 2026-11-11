#!/usr/bin/env bash
# Generate the demo tapes for the web page: 5 source clips -> SpiralVision ->
# simulated worn cassette -> decoded, plus a "noise mode" demo.
# Needs ffmpeg and a release build of spiralvision-cli.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p spiralvision-cli
CLI=target/release/spiralvision-cli
OUT=docs/videos
TMP=$(mktemp -d)
WEAR=${WEAR:-1.0}
SECS=${SECS:-8}
mkdir -p "$OUT"
FONT=$(fc-match -f '%{file}' 'DejaVu Sans:bold' 2>/dev/null || echo /usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf)

src() { # name, lavfi graph
  ffmpeg -loglevel error -y -f lavfi -i "$2" -t "$SECS" -r 10 -s 320x240 -pix_fmt yuv420p "$TMP/$1.mp4"
}

src mandelbrot "mandelbrot=s=320x240:r=10:start_scale=1.2:end_scale=0.02:end_pts=80,hue=h=t*30"
src testcard   "testsrc2=s=320x240:r=10"
src plasma     "nullsrc=s=320x240:r=10,geq=r='128+127*sin(X/14+T*2.1)*cos(Y/19-T)':g='128+127*sin((X+Y)/23+T*1.3)':b='128+127*cos(hypot(X-160-60*sin(T),Y-120-40*cos(T*0.7))/11-T*3)'"
src life       "life=s=80x60:r=10:mold=10:life_color=#ff2bd6:death_color=#00f0ff:mold_color=#2a0060:ratio=0.18:seed=42,scale=320:240:flags=neighbor"
src neonsign   "gradients=s=320x240:r=10:c0=#ff2bd6:c1=#00f0ff:c2=#39ff14:c3=#2a0060:nb_colors=4:speed=0.03,drawtext=fontfile=$FONT:text='SPIRAL':fontsize=58:fontcolor=white:borderw=5:bordercolor=black:x=(w-tw)/2+40*sin(t*2):y=60+20*cos(t*3),drawtext=fontfile=$FONT:text='VISION':fontsize=58:fontcolor=yellow:borderw=5:bordercolor=black:x=(w-tw)/2-40*sin(t*2):y=140-20*cos(t*3)"

enc_mp4() { ffmpeg -loglevel error -y -i "$1" -c:v libx264 -pix_fmt yuv420p -crf 23 -preset slow -movflags +faststart "${@:3}" "$2"; }

for n in mandelbrot testcard plasma life neonsign; do
  echo "== $n"
  $CLI roundtrip "$TMP/$n.mp4" "$TMP/$n-out.mp4" --tape "$WEAR" --wav "$TMP/$n.wav"
  enc_mp4 "$TMP/$n.mp4" "$OUT/$n-in.mp4" -an
  # decoded video carries the actual SpiralVision tape audio
  ffmpeg -loglevel error -y -i "$TMP/$n-out.mp4" -i "$TMP/$n.wav" -map 0:v -map 1:a -c:v libx264 -pix_fmt yuv420p -crf 23 -preset slow \
    -c:a aac -b:a 96k -shortest -movflags +faststart "$OUT/$n-out.mp4"
  ffmpeg -loglevel error -y -ss 4 -i "$OUT/$n-out.mp4" -frames:v 1 -q:v 4 "$OUT/$n-poster.jpg"
done

# NOISE MODE: random input in, interesting noise out (same decoder, no sync)
echo "== noise"
ffmpeg -loglevel error -y -f lavfi -i "anoisesrc=d=3:c=pink:r=48000:a=0.3" \
  -f lavfi -i "sine=f=220:d=$((SECS-3)):r=48000,volume=0.5" -f lavfi -i "sine=f=5000:d=$((SECS-3)):r=48000,tremolo=f=3:d=0.9" \
  -filter_complex "[1][2]amerge=inputs=2[m];[0]aformat=channel_layouts=stereo[n];[n][m]concat=n=2:v=0:a=1[a]" -map "[a]" -ac 2 "$TMP/noise.wav"
$CLI decode "$TMP/noise.wav" "$TMP/noise-out.mp4"
ffmpeg -loglevel error -y -i "$TMP/noise.wav" -filter_complex "[0]showspectrum=s=320x240:mode=combined:color=rainbow:slide=scroll:scale=log,fps=10[v]" -map "[v]" -t "$SECS" \
  -c:v libx264 -pix_fmt yuv420p -crf 23 -an -movflags +faststart "$OUT/noise-in.mp4"
ffmpeg -loglevel error -y -i "$TMP/noise-out.mp4" -i "$TMP/noise.wav" -map 0:v -map 1:a -c:v libx264 -pix_fmt yuv420p -crf 23 \
  -c:a aac -b:a 96k -shortest -movflags +faststart "$OUT/noise-out.mp4"
ffmpeg -loglevel error -y -ss 5 -i "$OUT/noise-out.mp4" -frames:v 1 -q:v 4 "$OUT/noise-poster.jpg"

rm -rf "$TMP"
ls -l "$OUT"
