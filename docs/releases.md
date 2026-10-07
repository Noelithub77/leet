# Installing leet

Visit [the landing page](https://noelithub77.github.io/leet/). It detects Windows or the combined Linux/macOS option. Ctrl+C (or Cmd+C) copies the Unix install command when no text or form control is selected; ordinary text copying keeps its normal behavior.

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

The [release workflow](../.github/workflows/release.yml) builds all five native targets on GitHub-hosted runners. A separate Linux x86_64 job runs the full workspace and installer tests alongside the native builds; publishing requires both tests and every platform build to pass. All targets build the executable and packaging tool together, then check the workspace and examples in the same release profile and check the executable's version. This avoids rebuilding dependencies in a separate development profile just to package the app. Thin LTO and the app's release optimization remain enabled. The Rust packaging operator produces Unix archives or a single Windows executable:

```sh
cargo build --locked --release -p gui --target x86_64-unknown-linux-gnu
./ops release:package --target x86_64-unknown-linux-gnu --tag v0.1.0 --output dist
```

On Windows, invoke the same Rust operator directly:

```powershell
cargo run --locked --release -p practice --example release --target x86_64-pc-windows-msvc -- --target x86_64-pc-windows-msvc --tag v0.1.0 --output dist
```

Push a version tag to release it:

```sh
git tag -a v0.1.0 -m "leet v0.1.0"
git push origin v0.1.0
```

Use the GitHub Actions **Release → Run workflow** button with an existing `vX.Y.Z` tag to retry/rebuild that release. The selected tag must exist; every build checks out that tag. Publishing waits for every target, uploads archives, the executable, installer, and SHA-256 manifest to a draft, and then publishes it. Failed builds do not publish an incomplete release. Workflow tokens need `contents: write` only in the publishing job. Never put personal credentials or data into build artifacts.

Rust builds use GitHub's first-party cache action for Cargo downloads and compiled dependencies. Test and release caches are separate; native caches are isolated by runner OS/version, target architecture, compiler identity, lockfile, manifests, and build configuration. Incremental directories and finished executables are excluded. Cargo still validates and builds the sources on every run; a cache hit never skips checks or tests.

To warm caches without replacing a published release, select the **main** branch in **Release → Run workflow**, enter an existing version tag, and disable **publish**. The workflow builds and packages the tag but skips the publishing job. GitHub scopes caches to the workflow's branch: caches saved on main are available to future tags; caches saved on one tag cannot be read by another tag. The first run is cold. Repeat the build-only run to measure restoration and build time before expecting faster releases. New compiler versions or native build changes can require a fresh cache. Ordinary pushes do not start Rust builds.

The Vite landing page lives in `site/`, with React and Tailwind for the requested shadcn `Kbd`/`KbdGroup` components. The adapted upstream component and its MIT license live in `site/src/components/ui/`. Platform detection and copying use browser APIs. The page uses Vesper's `#99FFE4` accent and a 130% desktop type scale, with responsive sizing on mobile. Its logo splash is initial HTML and disappears when React is ready, with no timed delay. The GitHub Pages workflow publishes changes pushed to main; Pages must use **GitHub Actions** as its source.

```sh
cd site
pnpm install --frozen-lockfile
pnpm ops --help
pnpm ops check --json
pnpm dev
```

The Pages workflow caches pnpm's package store using `site/pnpm-lock.yaml`; installation still uses the frozen lockfile, and checks/builds run on every deployment. Release uploads skip redundant ZIP compression and expire after three days; the published GitHub Release assets are retained independently.

`./ops check --json` covers Rust and isolated installer regressions; `pnpm ops check --json` in `site/` covers platform detection and the production page build. A successful build/version probe does not establish native window rendering, account-store behavior, or OS download-policy behavior on another computer. Validate those on the corresponding desktop before claiming device-level support.
