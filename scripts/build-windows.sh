#!/usr/bin/env bash
# Builds the Windows release from WSL: copies the sources to a Windows folder,
# runs scripts/build-windows.cmd there, then copies the deliverables to artifacts/.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -n "${RUNTERM_WIN_BUILD_DIR:-}" ]]; then
  build_dir="$RUNTERM_WIN_BUILD_DIR"
else
  win_profile="$(cmd.exe /c 'echo %USERPROFILE%' 2>/dev/null | tr -d '\r')"
  build_dir="$(wslpath -u "$win_profile")/runterm-build"
fi

case "$build_dir" in
  /mnt/*) ;;
  *)
    echo "The build directory must be on a Windows drive (/mnt/...): $build_dir" >&2
    exit 1
    ;;
esac

echo "Syncing sources to $build_dir"
mkdir -p "$build_dir"
rsync -a --delete \
  --exclude /.git \
  --exclude /node_modules \
  --exclude /target \
  --exclude /dist \
  --exclude /artifacts \
  --exclude /node \
  --exclude /toolchain \
  --exclude /.cargo \
  --exclude /.npm-cache \
  --exclude '*.tsbuildinfo' \
  "$repo/" "$build_dir/"

echo "Building on Windows"
(cd "$build_dir" && cmd.exe /c 'scripts\build-windows.cmd')

exe="$build_dir/target/release/runterm.exe"
setup="$(ls -t "$build_dir"/target/release/bundle/nsis/RunTerm_*_x64-setup.exe | head -n 1)"

mkdir -p "$repo/artifacts"
rm -f "$repo"/artifacts/RunTerm_*_x64-setup.exe
cp "$exe" "$repo/artifacts/RunTerm.exe"
cp "$setup" "$repo/artifacts/"
(cd "$repo" && sha256sum artifacts/RunTerm.exe "artifacts/$(basename "$setup")" > artifacts/SHA256SUMS.txt)

echo "Deliverables:"
cat "$repo/artifacts/SHA256SUMS.txt"
