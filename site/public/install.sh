#!/bin/sh
# Download, verify, install, and open the latest public leet release.
set -eu

fail() { printf 'leet: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "Required command not found: $1"; }
for tool in curl tar uname mktemp; do need "$tool"; done
[ -n "${HOME:-}" ] || fail 'Home directory unavailable.'

case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) fail 'Use the portable Windows executable from https://github.com/Noelithub77/leet/releases/latest' ;;
esac
case "$(uname -m)" in
  x86_64|amd64) arch=x86_64 ;;
  arm64|aarch64) arch=aarch64 ;;
  *) fail "Unsupported architecture: $(uname -m)" ;;
esac
if [ "$os" = macos ]; then
  # A terminal running under Rosetta should still install the Apple Silicon build.
  if [ "$(sysctl -in sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then arch=aarch64; fi
  major=$(sw_vers -productVersion | cut -d. -f1)
  [ "$major" -ge 13 ] || fail 'macOS 13 or later is required.'
  if [ "$arch" = aarch64 ]; then platform=mac-arm; else platform=mac-intel; fi
elif [ -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
  fail 'Run this command inside a Linux graphical desktop to install and open leet.'
elif [ "$arch" = aarch64 ]; then
  platform=linux-arm
else
  platform=linux
fi


tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
repo=https://github.com/Noelithub77/leet
latest=$(curl -fsSL --retry 3 -o /dev/null -w '%{url_effective}' "$repo/releases/latest")
tag=${latest##*/}
printf '%s\n' "$tag" | LC_ALL=C grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || fail 'No stable release is available yet.'
asset=leet-$platform-$tag.tar.gz
base=$repo/releases/download/$tag
printf 'Getting leet %s for %s %s…\n' "$tag" "$os" "$arch"
curl -fSL --retry 3 "$base/$asset" -o "$tmp/$asset"
curl -fsSL --retry 3 "$base/SHA256SUMS" -o "$tmp/SHA256SUMS"
awk -v asset="$asset" '$2 == asset { print; found=1 } END { if (!found) exit 1 }' "$tmp/SHA256SUMS" > "$tmp/checksum" || fail 'Release checksum missing.'
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$tmp" && sha256sum -c checksum) || fail 'Checksum verification failed.'
else
  need shasum
  (cd "$tmp" && shasum -a 256 -c checksum) || fail 'Checksum verification failed.'
fi
mkdir "$tmp/unpack"
tar -xzf "$tmp/$asset" -C "$tmp/unpack"

if [ "$os" = linux ]; then
  need ldd
  libs=$(ldd "$tmp/unpack/leet" 2>&1) || fail "This build cannot run on this system: $libs"
  if printf '%s\n' "$libs" | grep -q 'GLIBC_.*not found'; then
    fail 'This build needs a newer glibc: x86_64 requires 2.35+, ARM64 requires 2.39+.'
  fi
  vulkan=false
  for path in /usr/lib/libvulkan.so.1 /usr/lib64/libvulkan.so.1 /usr/lib/*-linux-gnu/libvulkan.so.1; do
    if [ -f "$path" ]; then vulkan=true; fi
  done
  if printf '%s\n' "$libs" | grep -q 'not found' || [ "$vulkan" = false ]; then
    printf 'Installing desktop runtime libraries (sudo may ask for your password)…\n'
    need sudo
    sudo -v </dev/tty || fail 'Runtime libraries need administrator approval.'
    if command -v apt-get >/dev/null 2>&1; then
      sudo apt-get update
      sudo apt-get install -y libxcb1 libxkbcommon0 libxkbcommon-x11-0 libvulkan1 fonts-liberation ca-certificates
    elif command -v dnf >/dev/null 2>&1; then
      sudo dnf install -y libxcb libxkbcommon libxkbcommon-x11 vulkan-loader liberation-sans-fonts ca-certificates
    elif command -v pacman >/dev/null 2>&1; then
      sudo pacman -S --needed --noconfirm libxcb libxkbcommon libxkbcommon-x11 vulkan-icd-loader ttf-liberation ca-certificates
    elif command -v zypper >/dev/null 2>&1; then
      sudo zypper --non-interactive install libxcb1 libxkbcommon0 libxkbcommon-x11-0 libvulkan1 liberation-fonts ca-certificates
    else
      fail "Install libxcb, libxkbcommon, libxkbcommon-x11 and Vulkan for your distribution, then retry. Missing libraries: $libs"
    fi
  fi
  "$tmp/unpack/leet" --version || fail 'The downloaded app could not start; check runtime libraries and glibc.'
fi

bin=$HOME/.local/bin
builds=$HOME/.local/share/leet/releases
mkdir -p "$bin" "$builds"
for link in "$bin/leet" "$bin/1337"; do
  [ ! -e "$link" ] || [ -L "$link" ] || fail "Refusing to replace an existing file: $link"
done
if [ "$os" = macos ]; then
  mkdir -p "$HOME/Applications"
  app=$HOME/Applications/leet.app
  [ ! -e "$app" ] || [ -L "$app" ] || fail "Refusing to replace an existing application: $app"
fi
installed=$(mktemp -d "$builds/$tag-$os-$arch.XXXXXX")
cp -R "$tmp/unpack/." "$installed/"
if [ "$os" = macos ]; then
  executable=$installed/leet.app/Contents/MacOS/leet
  ln -sfn "$installed/leet.app" "$app"
else
  executable=$installed/leet
fi
ln -sfn "$executable" "$bin/leet"
ln -sfn "$executable" "$bin/1337"
if [ "$os" = linux ]; then
  desktop=$HOME/.local/share/applications
  icons=$HOME/.local/share/icons/hicolor/scalable/apps
  mkdir -p "$desktop" "$icons"
  cp "$installed/leet.svg" "$icons/leet.svg"
  escaped=$(printf '%s' "$bin/leet" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g')
  cat > "$desktop/leet.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=leet
GenericName=Coding practice IDE
Exec="$escaped"
Icon=leet
Terminal=false
StartupWMClass=leet
Categories=Development;
EOF
fi
printf 'Installed %s. Opening leet…\n' "$tag"
case ":$PATH:" in *":$bin:"*) ;; *) printf 'For later terminal launches, add %s to PATH.\n' "$bin" ;; esac
if [ "$os" = macos ]; then
  open "$app" || fail 'Installed successfully, but macOS blocked opening it. See System Settings → Privacy & Security.'
else
  mkdir -p "$HOME/.local/state/leet"
  nohup "$bin/leet" > "$HOME/.local/state/leet/launch.log" 2>&1 < /dev/null &
  app_pid=$!
  sleep 2
  kill -0 "$app_pid" 2>/dev/null || fail "Installed successfully, but launch failed. See $HOME/.local/state/leet/launch.log (a Vulkan GPU driver is required)."
fi
