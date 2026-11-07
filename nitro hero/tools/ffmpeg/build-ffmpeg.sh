#!/bin/bash
# Builds the small ffmpeg.exe that ships with Nitro: only what compressing and reading chat
# videos needs. A full build is over 200 MB, this should be a fraction of that. It's one
# static file, so nothing else ships with it. Run inside an MSYS2 MINGW64 shell:
#   pacman -S --needed mingw-w64-x86_64-gcc mingw-w64-x86_64-x264 mingw-w64-x86_64-dav1d \
#     mingw-w64-x86_64-pkgconf make nasm tar xz diffutils
# Takes the output folder as its argument. libx264 makes this build GPL: see FFMPEG-NOTICE.txt.
set -euo pipefail
VERSION=7.1.1
OUT="${1:?output folder}"
WORK="${2:-$PWD/ffmpeg-build}"
mkdir -p "$WORK" "$OUT"
cd "$WORK"
[ -f "ffmpeg-$VERSION.tar.xz" ] || curl -fsSL -o "ffmpeg-$VERSION.tar.xz" "https://ffmpeg.org/releases/ffmpeg-$VERSION.tar.xz"
[ -d "ffmpeg-$VERSION" ] || tar xf "ffmpeg-$VERSION.tar.xz"
cd "ffmpeg-$VERSION"
./configure \
  --prefix="$WORK/prefix" --arch=x86_64 --target-os=mingw32 \
  --pkg-config-flags=--static --extra-ldflags=-static \
  --enable-gpl --enable-libx264 --enable-libdav1d --enable-static --disable-shared \
  --disable-everything --disable-autodetect --disable-doc --disable-debug --disable-network \
  --disable-ffplay --disable-ffprobe --disable-avdevice --disable-postproc --disable-hwaccels \
  --enable-protocol=file,pipe \
  --enable-demuxer=mov,matroska,avi,asf,mpegts,mpegps,flv,ogg,wav,mp3,aac,h264,hevc,m4v,mpegvideo,gif \
  --enable-decoder=h264,hevc,vp8,vp9,libdav1d,mpeg4,mpeg2video,mpeg1video,msmpeg4v3,wmv1,wmv2,wmv3,vc1,mjpeg,prores,gif,theora \
  --enable-decoder=aac,aac_latm,mp3,mp2,ac3,eac3,opus,vorbis,flac,alac,wmav1,wmav2,wmapro,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s32le,pcm_f32le,pcm_u8,pcm_alaw,pcm_mulaw \
  --enable-parser=h264,hevc,vp8,vp9,av1,aac,aac_latm,mpeg4video,mpegvideo,mpegaudio,ac3,opus,vorbis,flac,vc1,mjpeg \
  --enable-bsf=aac_adtstoasc,h264_mp4toannexb,hevc_mp4toannexb,vp9_superframe,vp9_superframe_split,extract_extradata \
  --enable-encoder=libx264,aac,mjpeg \
  --enable-muxer=mp4,image2 \
  --enable-filter=scale,format,aformat,aresample,fps,null,anull,transpose,hflip,vflip,rotate,setsar,setpts,asetpts
make -j"$(nproc)"
strip ffmpeg.exe
cp ffmpeg.exe "$OUT/ffmpeg.exe"
ls -la "$OUT/ffmpeg.exe"
"$OUT/ffmpeg.exe" -hide_banner -version | head -2
