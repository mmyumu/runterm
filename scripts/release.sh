#!/usr/bin/env bash
# Bumps the app version everywhere, commits and creates the vX.Y.Z tag.
# Pushing the tag triggers .github/workflows/release.yml, which publishes the GitHub release.
set -euo pipefail

if [[ $# -ne 1 || ! "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]; then
  echo "Usage: scripts/release.sh X.Y.Z[-prerelease]" >&2
  exit 1
fi
version="$1"
tag="v$version"

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ -n "$(git status --porcelain)" ]]; then
  echo "The working tree is not clean; commit or stash your changes first." >&2
  exit 1
fi
if [[ "$(git branch --show-current)" != main ]]; then
  echo "Releases are made from main." >&2
  exit 1
fi
if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
  echo "Tag $tag already exists." >&2
  exit 1
fi

npm version "$version" --no-git-tag-version --allow-same-version >/dev/null
sed -i -E "0,/\"version\": \"[^\"]*\"/s//\"version\": \"$version\"/" src-tauri/tauri.conf.json
sed -i -E "0,/^version = \"[^\"]*\"/s//version = \"$version\"/" src-tauri/Cargo.toml crates/core/Cargo.toml
cargo update --workspace --offline --quiet

git add package.json package-lock.json src-tauri/tauri.conf.json src-tauri/Cargo.toml crates/core/Cargo.toml Cargo.lock
git commit --quiet -m "Release $tag"
git tag -a "$tag" -m "RunTerm $tag"

echo "Created commit and tag $tag. Publish with:"
echo "  git push origin main $tag"
