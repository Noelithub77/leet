# Installing leet

Visit [the download page](https://noelithub77.github.io/leet/). It detects your desktop platform; its selector also lets you get the command for another computer.

On Linux or macOS, paste this single command into your terminal:

```sh
curl -fsSL https://noelithub77.github.io/leet/install.sh | sh
```

The installer selects the native architecture, downloads one stable release, verifies its archive against that release's `SHA256SUMS`, installs a versioned build, creates `leet` and `1337` launch commands, and opens the app. Run the same command again to upgrade. It preserves settings, cache, accounts, and solutions. Checksums detect corrupted downloads; they are distributed by the same GitHub release and are not independent publisher signatures.

Windows users download [the portable x64 executable](https://github.com/Noelithub77/leet/releases/latest/download/leet-windows-x86_64.exe) and open it. There is no installation wizard or Rust requirement. Settings and solutions live in the normal user folders, rather than beside the executable. To upgrade, close leet and replace the executable with the newer download.

## Requirements

| Platform | Supported build | Requirements |
| --- | --- | --- |
| Linux | x86_64 | glibc 2.35+ (Ubuntu 22.04 or newer), graphical X11/Wayland desktop, Vulkan GPU driver |
| Linux | ARM64 | glibc 2.39+ (Ubuntu 24.04 or newer), graphical X11/Wayland desktop, Vulkan GPU driver |
| macOS | Intel / Apple Silicon | macOS 13+ |
| Windows | x64 | Windows 10+ |

Linux requires libxcb, libxkbcommon, libxkbcommon-x11, and the Vulkan loader. Missing libraries can be installed automatically on Debian/Ubuntu, Fedora, Arch, and openSUSE; sudo may request your password. A GPU's Vulkan driver is specific to your hardware and must already be configured. Headless servers and musl distributions such as Alpine are unsupported by these desktop builds. A Linux launch failure is recorded in `~/.local/state/leet/launch.log`.

Unix launch commands are installed in `~/.local/bin`; if that directory is absent from PATH, the installer explains how to find them. Linux gets a desktop entry; macOS gets `~/Applications/leet.app`. Previous versioned release directories are retained so a running app's executable is never overwritten. Remove old directories only after their app processes exit.

These releases have no publisher signing certificates. macOS bundles have the ad-hoc signature required for Apple Silicon, but are not notarized; Gatekeeper may require approval in System Settings → Privacy & Security. Windows may show SmartScreen warnings or organizational policy may block an unsigned executable. The installer does not disable OS security controls. Signing alone cannot guarantee a Windows reputation warning disappears.

The app itself needs no compiler. Running local Python solutions requires Python (`python3` on Unix, `python` on Windows); other local languages require their respective compiler. Language servers are discovered separately. LeetCode's remote judge remains available with an account. The installer does not provision full development toolchains.

Linux retains the existing Secret Service and private-file credential behavior. macOS uses Keychain and Windows uses Credential Manager through the maintained `keyring` crate. Those platforms fail explicitly if saving to the OS store fails, rather than writing a new unprotected credentials file. Existing compatible account files can still be read.

## Building and publishing

The [release workflow](../.github/workflows/release.yml) builds all five native targets on GitHub-hosted runners. Linux x86_64 runs the full workspace and installer tests. All targets check the workspace and examples, build a release executable, and check its version. The Rust packaging operator produces Unix archives or a single Windows executable:

```sh
cargo build --locked --release -p gui --target x86_64-unknown-linux-gnu
./ops release:package --target x86_64-unknown-linux-gnu --tag v0.1.0 --output dist
```

On Windows, invoke the same Rust operator directly:

```powershell
cargo run --locked -p practice --example release --target x86_64-pc-windows-msvc -- --target x86_64-pc-windows-msvc --tag v0.1.0 --output dist
```

Push a version tag to release it:

```sh
git tag -a v0.1.0 -m "leet v0.1.0"
git push origin v0.1.0
```

Use the GitHub Actions **Release → Run workflow** button with an existing `vX.Y.Z` tag to retry/rebuild that release. The selected tag must exist; every build checks out that tag. Publishing waits for every target, uploads archives, the executable, installer, and SHA-256 manifest to a draft, and then publishes it. Failed builds do not publish an incomplete release. Workflow tokens need `contents: write` only in the publishing job. Never put personal credentials or data into build artifacts.

The Vite page lives in `site/`. Its only frontend dependency is Vite; platform detection and the copy button use browser APIs. Its GitHub Pages workflow publishes changes pushed to main. GitHub Pages must use **GitHub Actions** as its source.

```sh
cd site
pnpm install --frozen-lockfile
pnpm ops --help
pnpm ops check --json
pnpm dev
```

`./ops check --json` covers Rust and isolated installer regressions; `pnpm ops check --json` in `site/` covers platform detection and the production page build. A successful build/version probe does not establish native window rendering, account-store behavior, or OS download-policy behavior on another computer. Validate those on the corresponding desktop before claiming device-level support.
