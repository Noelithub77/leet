# Installing leet

Visit [the landing page](https://noelithub77.github.io/leet/). It detects Windows or the combined Linux/macOS option. Ctrl+C (or Cmd+C) copies the Unix install command when no text or form control is selected; ordinary text copying keeps its normal behavior.

On Linux or macOS, paste this single command into your terminal:

```sh
curl -fsSL https://noelithub77.github.io/leet/install.sh | sh
```

The installer selects the native architecture, downloads one stable release, verifies its archive against that release's `SHA256SUMS`, installs a versioned build, creates `leet` and `1337` launch commands, and opens the app. Run the same command again to upgrade. It preserves settings, cache, accounts, and solutions. Checksums detect corrupted downloads; they are distributed by the same GitHub release and are not independent publisher signatures.

Windows users download [the portable x64 executable](https://github.com/Noelithub77/leet/releases/latest/download/leet-windows-x86_64.exe) and open it. There is no installation wizard or Rust requirement. Settings and solutions live in the normal user folders, rather than beside the executable. On each launch, leet checks the latest stable GitHub release with one metadata request. Hover or click the bottom-right icon to open the changelog above the status line. Its single Update button downloads, verifies, and installs the release, then becomes Restart. Click Restart when ready. Feat starts expanded and Fix collapsed; both contain concise change lists. Editor state is saved before restart. Installation replaces only the executable or macOS application bundle. Failed checksum or version checks preserve the installed app. The same Restart action launches the stable installed command after a local deployment, including when the previous versioned binary has been removed. If the release check is unavailable, the hover view shows the bundled changelog.

## Requirements

| Platform | Supported build | Requirements |
| --- | --- | --- |
| Linux | x86_64 | glibc 2.35+ (Ubuntu 22.04 or newer), graphical X11/Wayland desktop, Vulkan GPU driver |
| Linux | ARM64 | glibc 2.39+ (Ubuntu 24.04 or newer), graphical X11/Wayland desktop, Vulkan GPU driver |
| macOS | Intel / Apple Silicon | macOS 13+ |
| Windows | x64 | Windows 10+ |

Linux archive installs require libxcb, libxkbcommon, libxkbcommon-x11, and the Vulkan loader. Missing libraries can be installed automatically on Debian/Ubuntu, Fedora, Arch, and openSUSE; sudo may request your password. A GPU's Vulkan driver is specific to your hardware and must already be configured. Headless servers and musl distributions such as Alpine are unsupported by these desktop builds. A Linux launch failure is recorded in `~/.local/state/leet/launch.log`.

Linux releases also include portable AppImages for both architectures. They bundle eligible shared libraries and their distribution copyright notices, and update the original AppImage file when launched as an AppImage. The graphical desktop, glibc, and GPU drivers remain system requirements. CI produces the archive and AppImage from the same compiled binary; pinned packaging tools are cached by architecture and verified against SHA-256 values on every use.

Unix launch commands are installed in `~/.local/bin`; if that directory is absent from PATH, the installer explains how to find them. Linux gets a desktop entry; macOS gets `~/Applications/leet.app`. Previous versioned release directories are retained so a running app's executable is never overwritten. Remove old directories only after their app processes exit.

These releases have no publisher signing certificates. macOS bundles have the ad-hoc signature required for Apple Silicon, but are not notarized; Gatekeeper may require approval in System Settings → Privacy & Security. Windows may show SmartScreen warnings or organizational policy may block an unsigned executable. The installer does not disable OS security controls. Signing alone cannot guarantee a Windows reputation warning disappears.

The app itself needs no compiler. Running local Python solutions requires Python (`python3` on Unix, `python` on Windows); other local languages require their respective compiler. Language servers are discovered separately. LeetCode's remote judge remains available with an account. The installer does not provision full development toolchains.

Linux retains the existing Secret Service and private-file credential behavior. macOS uses Keychain and Windows uses Credential Manager through the maintained `keyring` crate. Those platforms fail explicitly if saving to the OS store fails, rather than writing a new unprotected credentials file. Existing compatible account files can still be read.

## Building and publishing

The [release workflow](../.github/workflows/release.yml) builds all five native targets on GitHub-hosted runners. A separate Linux x86_64 job runs the full workspace and installer tests alongside the native builds; publishing requires both tests and every platform build to pass. All targets build the executable and packaging tool together, then check the workspace and examples in the same release profile and check the executable's version. Packaging invokes the compiled Rust tool directly, avoiding a separate development build or another Cargo invocation with a different dependency feature selection. Thin LTO and the app's release optimization remain enabled. The Rust packaging operator produces Unix archives or a single Windows executable:

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

Rust builds use the maintained `Swatinem/rust-cache` action, pinned to the verified v2.9.2 commit, for Cargo downloads and compiled dependencies. Test and release caches are separate; native caches include Cargo/Rust/compiler flags and the macOS deployment target and are isolated by runner OS/version, target architecture, compiler identity, lockfile, manifests, build configuration, Windows CRT flags, and macOS deployment target. The action removes workspace artifacts, incremental directories, and unused packages before saving, keeping caches focused on reusable dependencies. Cargo still builds the current app sources on every run; a cache hit never skips checks or tests. CI invokes `cargo test --locked --workspace` and `python3 tests/install.py` directly, covering the same tests as `./ops check --json` without first compiling the local deployment operator.

Changes to Rust sources, manifests, native configuration, tests, the operator, or the release workflow on **main** run all five native builds plus Linux tests and warm their caches. Superseded main builds are canceled. Documentation-only and landing-page-only changes do not start native builds. Main builds do not package or publish a release.

For a manual cache warm-up, select **main** in **Release → Run workflow** and leave **tag** empty. To test release packaging without replacing published assets, enter an existing tag and disable **publish**. GitHub scopes caches to the workflow's branch: caches saved on main are available to future tags; caches saved on one tag cannot be read by another tag. Only main runs save Rust caches to avoid filling storage with redundant tag caches. The first run is cold. New compiler versions or native build changes can require a fresh cache.

The Vite landing page lives in `site/`. Static HTML and inline hero CSS paint the install command first; platform tabs and copying use browser APIs. GSAP, Lenis, and OGL load lazily for motion, with React/Sonner used only for feedback. Titles use Nunito and the page follows Vesper’s peach and mint palette. Videos load near the viewport; reduced motion and Save-Data retain posters. The GitHub Pages workflow publishes changes pushed to main; Pages must use **GitHub Actions** as its source.

```sh
cd site
pnpm install --frozen-lockfile
pnpm ops --help
pnpm ops check --json
pnpm dev
# http://localhost:1337/leet/
```

The Pages workflow caches pnpm's package store using `site/pnpm-lock.yaml`; installation still uses the frozen lockfile, and checks/builds run on every deployment. Release uploads skip redundant ZIP compression and expire after three days; the published GitHub Release assets are retained independently.

The landing page keeps requirements in this document and release notes. Its only repository link is the header's GitHub icon, adapted from Primer Octicons with its MIT license in `site/src/icons/LICENSE`.

`./ops check --json` covers Rust and isolated installer regressions; `pnpm ops check --json` in `site/` covers platform detection and the production page build. A successful build/version probe does not establish native window rendering, account-store behavior, or OS download-policy behavior on another computer. Validate those on the corresponding desktop before claiming device-level support.
