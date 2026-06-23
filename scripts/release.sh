#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: %s "commit message" <tag-version>\n' "$0" >&2
  printf 'Example: %s "Release v1.2.3" v1.2.3\n' "$0" >&2
}

if [[ $# -ne 2 ]]; then
  usage
  exit 2
fi

commit_msg=$1
tag_version=$2
cargo_version=${tag_version#v}

if [[ -z "$commit_msg" || -z "$tag_version" || -z "$cargo_version" ]]; then
  usage
  exit 2
fi

if ! [[ "$cargo_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$ ]]; then
  printf 'Invalid version: %s\n' "$tag_version" >&2
  printf 'Pass a semver tag such as v1.2.3 or 1.2.3.\n' >&2
  exit 2
fi

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

branch=$(git branch --show-current)
if [[ -z "$branch" ]]; then
  printf 'Not on a branch. Checkout a branch before releasing.\n' >&2
  exit 1
fi

if git rev-parse -q --verify "refs/tags/$tag_version" >/dev/null; then
  printf 'Tag already exists locally: %s\n' "$tag_version" >&2
  exit 1
fi

if git ls-remote --exit-code --tags origin "refs/tags/$tag_version" >/dev/null 2>&1; then
  printf 'Tag already exists on origin: %s\n' "$tag_version" >&2
  exit 1
fi

update_toml_version() {
  local file=$1
  perl -0pi -e 's/(\[package\]\s*\n(?:[^\[]*?\n)*?version\s*=\s*")[^"]+(")/${1}$ENV{CARGO_VERSION}${2}/s' "$file"
}

export CARGO_VERSION=$cargo_version
update_toml_version Cargo.toml
update_toml_version src-tauri/Cargo.toml
update_toml_version src-tauri/nethelper/Cargo.toml

perl -0pi -e 's/("version"\s*:\s*")[^"]+(")/${1}$ENV{CARGO_VERSION}${2}/' src-tauri/tauri.conf.json

cargo metadata --format-version 1 --no-deps >/dev/null

git add \
  Cargo.toml \
  Cargo.lock \
  src-tauri/Cargo.toml \
  src-tauri/nethelper/Cargo.toml \
  src-tauri/tauri.conf.json

if git diff --cached --quiet; then
  printf 'No release version changes to commit.\n' >&2
  exit 1
fi

git commit -m "$commit_msg"
git push origin "$branch"
git tag "$tag_version"
git push origin "$tag_version"

printf 'Released %s on %s.\n' "$tag_version" "$branch"
