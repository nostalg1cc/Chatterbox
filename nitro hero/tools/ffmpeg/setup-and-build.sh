#!/bin/bash
# Run from Git Bash. Sets up a temporary MSYS2 (nothing is installed on the system) from its
# official release, then builds the small ffmpeg with build-ffmpeg.sh. It ends up in
# tools/ffmpeg/out/ffmpeg.exe. The temporary folder, %TEMP%\nh-ffmpeg, can be deleted afterwards.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="$TEMP/nh-ffmpeg"
mkdir -p "$WORK"
cd "$WORK"
if [ ! -d msys64 ]; then
  echo "Downloading MSYS2..."
  curl -fSL -o msys2.sfx.exe https://github.com/msys2/msys2-installer/releases/latest/download/msys2-base-x86_64-latest.sfx.exe
  ./msys2.sfx.exe -y -o"$(cygpath -w "$WORK")" > /dev/null
fi
SHELL_EXE="$WORK/msys64/usr/bin/bash.exe"
export MSYSTEM=MINGW64 CHERE_INVOKING=1
echo "Installing the compiler (first run sets up MSYS2, which takes a minute)..."
"$SHELL_EXE" -lc 'pacman --noconfirm -Sy --needed mingw-w64-x86_64-gcc mingw-w64-x86_64-x264 mingw-w64-x86_64-dav1d mingw-w64-x86_64-pkgconf make nasm tar xz diffutils'
WORK_MSYS="$(cygpath -m "$WORK")"
HERE_MSYS="$(cygpath -m "$HERE")"
echo "Building ffmpeg..."
"$SHELL_EXE" -lc "cd '$WORK_MSYS' && bash '$HERE_MSYS/build-ffmpeg.sh' '$HERE_MSYS/out' '$WORK_MSYS/build'"
echo
echo "Done: $HERE/out/ffmpeg.exe"
