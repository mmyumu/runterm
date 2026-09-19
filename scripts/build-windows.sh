#!/usr/bin/env bash
# Construit la version Windows depuis WSL : copie les sources dans un dossier
# Windows, y lance scripts/build-windows.cmd, puis copie les livrables dans artifacts/.
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
    echo "Le dossier de build doit être sur un disque Windows (/mnt/...) : $build_dir" >&2
    exit 1
    ;;
esac

echo "Synchronisation des sources vers $build_dir"
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

echo "Compilation Windows"
(cd "$build_dir" && cmd.exe /c 'scripts\build-windows.cmd')

exe="$build_dir/target/release/runterm.exe"
setup="$(ls -t "$build_dir"/target/release/bundle/nsis/RunTerm_*_x64-setup.exe | head -n 1)"

mkdir -p "$repo/artifacts"
rm -f "$repo"/artifacts/RunTerm_*_x64-setup.exe
cp "$exe" "$repo/artifacts/RunTerm.exe"
cp "$setup" "$repo/artifacts/"
(cd "$repo" && sha256sum artifacts/RunTerm.exe "artifacts/$(basename "$setup")" > artifacts/SHA256SUMS.txt)

echo "Livrables :"
cat "$repo/artifacts/SHA256SUMS.txt"
