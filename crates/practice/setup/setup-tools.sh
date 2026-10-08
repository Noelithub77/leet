#!/bin/sh
set -eu
[ "$#" -eq 2 ] || { echo "Usage: setup-tools.sh DIRECTORY python|cpp|c" >&2; exit 2; }
tools_dir=$1
language=$2
case "$language" in python|cpp|c) ;; *) echo "Unsupported tool setup language" >&2; exit 2 ;; esac
mkdir -p "$tools_dir"
mkdir "$tools_dir/.install-lock" 2>/dev/null || { echo "Tool installation is already running" >&2; exit 1; }
printf '%s' "${LEET_TOOL_INSTALL_ID:-manual}" > "$tools_dir/.install-lock/owner"
stage=$(mktemp -d "$tools_dir/.download-XXXXXX")
trap 'rm -rf "$stage" "$tools_dir/.install-lock"' EXIT HUP INT TERM
python="$tools_dir/python/bin/python3"
if [ ! -x "$python" ]; then
  echo "Downloading portable Python…"
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64) url="https://github.com/astral-sh/python-build-standalone/releases/download/20261003/cpython-3.13.16%2B20261003-aarch64-apple-darwin-install_only_stripped.tar.gz"; digest="9e01f63bbb08576cd9c8bc2d0564d098cb30c8453a0cd4bcf6aef458f6d2a147" ;;
    Linux:aarch64) url="https://github.com/astral-sh/python-build-standalone/releases/download/20261003/cpython-3.13.16%2B20261003-aarch64-unknown-linux-gnu-install_only_stripped.tar.gz"; digest="6e9641400f8debd9b7924b27b5ff0662c372852382e291a76c450c7b67414cf6" ;;
    Darwin:x86_64) url="https://github.com/astral-sh/python-build-standalone/releases/download/20261003/cpython-3.13.16%2B20261003-x86_64-apple-darwin-install_only_stripped.tar.gz"; digest="b4dad38ba6a344555ccb71a1b08caad0a6c0dda88c5803658bc95bd7f04e9f5c" ;;
    Linux:x86_64) url="https://github.com/astral-sh/python-build-standalone/releases/download/20261003/cpython-3.13.16%2B20261003-x86_64-unknown-linux-gnu-install_only_stripped.tar.gz"; digest="4595c5589fff7bf0cb158d9a88a797e0d791fa33830770fcb7bf3f4b104feeae" ;;
    *) echo "Unsupported platform or architecture" >&2; exit 1 ;;
  esac
  curl --silent --show-error --fail --location --retry 2 --connect-timeout 20 --max-time 600 "$url" -o "$stage/python.tar.gz"
  if command -v sha256sum >/dev/null 2>&1; then actual=$(sha256sum "$stage/python.tar.gz" | cut -d " " -f 1); else actual=$(shasum -a 256 "$stage/python.tar.gz" | cut -d " " -f 1); fi
  [ "$actual" = "$digest" ] || { echo "Python checksum mismatch" >&2; exit 1; }
  tar -xzf "$stage/python.tar.gz" -C "$stage"
  "$stage/python/bin/python3" --version
  mv "$stage/python" "$tools_dir/python"
fi
"$python" "$tools_dir/setup-tools.py" "$tools_dir" "$language"
