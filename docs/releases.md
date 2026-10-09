# Installing leet

Visit [the landing page](https://leet.allpyq.in/). It detects Windows or the combined Linux/macOS option. Ctrl+C (or Cmd+C) copies the Unix install command when no text or form control is selected; ordinary text copying keeps its normal behavior.

On Linux or macOS, paste this single command into your terminal:

```sh
curl -fsSL https://leet.allpyq.in/install.sh | sh
```

The installer selects the native architecture, downloads one stable release, verifies its archive against that release's `SHA256SUMS`, installs a versioned build, creates `leet` and `1337` launch commands, and opens the app. Run the same command again to upgrade. It preserves settings, cache, accounts, and solutions. Checksums detect corrupted downloads; they are distributed by the same GitHub release and are not independent publisher signatures.

Windows users download the versioned portable executable (`leet-windows-vX.Y.Z.exe`) from the [latest release](https://github.com/Noelithub77/leet/releases/latest) and open it. There is no installation wizard or Rust requirement. Settings and solutions live in the normal user folders, rather than beside the executable.

On each launch, leet checks the latest stable GitHub release with one metadata request. Click the bottom-right icon to open the scrollable changelog above the status line. Click the icon again or elsewhere to close it. New releases download, verify, and install automatically in the background while the current app keeps running. Once installed, choose Restart now or Later. Later closes the panel; the next normal launch uses the new version. Failed installs offer Retry update. Feat starts expanded and Fix collapsed; both contain concise change lists. Editor state is saved before restart. Installation replaces only the executable or macOS application bundle. Failed checksum or version checks preserve the installed app. The same Restart now action launches the stable installed command after a local deployment, including when the previous versioned binary has been removed. If the release check is unavailable, click the icon to see the bundled changelog.

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

The app itself needs no compiler. Running local Python solutions requires Python (`python3` on Unix, `python` on Windows); other local languages require their respective compiler. LeetCode's remote judge remains available with an account. Continue in onboarding works while tools are missing or still being checked.

## Private editor tools

Onboarding starts private tool setup automatically for Python, C, and C++, and Continue stays available throughout. Setup keeps running after onboarding closes; the status bar shows progress, Ready, or Retry with the failure details on hover. Debug mode also offers Install tools. Leet downloads pinned archives and wheels directly, verifies SHA-256 digests, and keeps them in its user data directory under `leet/tools`. Existing vg data directories remain compatible. Installation does not change system PATH, replace system tools, install a package manager, or request administrator access. Leet launches its managed language servers and default Python interpreter by their full paths; a custom Python executable remains authoritative.

Pinned versions are portable Python 3.13.16 (Astral build 20261003), basedpyright 1.40.2, Node 24.19.0, and clangd 23.1.0. Only the selected language tools are downloaded; Python is reused when installing another language. Windows setup uses built-in Windows PowerShell 5.1 and tar, available on Windows 10 version 1803 and newer. macOS setup uses the built-in shell, curl, shasum, and tar; Homebrew is not required. Downloads need internet access. Windows setup, tool checks, language servers, runners, and debugger processes request no console window.

Python tools include a portable interpreter and basedpyright with its private Node runtime. C/C++ tools install clangd for completion and diagnostics; they do not include a compiler or GDB. Native C++ debugging requires GDB with Python support and a C++ compiler. clangd alone does not provide debugging. The pinned upstream Linux clangd archive is x64 only; Python setup also supports Linux ARM64. macOS editor setup supports Intel and Apple Silicon; Windows builds target x64.

| Download | Windows x64 | macOS Intel / Apple Silicon | Linux x64 |
| --- | --- | --- | --- |
| Portable Python | 21.0 MiB | 23.8–24.1 MiB | 33.5 MiB |
| basedpyright + Node | 53.6 MiB | 66.6–66.7 MiB | 71.4 MiB |
| clangd | 28.3 MiB | 95.4 MiB | 112.5 MiB |

These are compressed asset sizes from the pinned upstream releases. Actual Linux x64 installation measured 388 MiB for Python tools and an additional 225 MiB for clangd, totaling 613 MiB. Temporary downloads need extra space during setup; installed sizes on Windows/macOS have not been measured. The pinned URLs and hashes live in `crates/practice/setup/tools.json`.

For manual setup, use the installed binary or the tracked operator:

```sh
leet --setup-tools --language python
leet --setup-tools --language cpp
./ops toolchain:setup --language python --dry-run --json
./ops toolchain:setup --language python --directory /absolute/private/tools --json
```

The app uses its normal data directory automatically; a custom operator directory is useful for isolated verification. `--dry-run` reports the manifest without creating files or downloading tools. Native Windows and macOS setup and window behavior still require verification on those platforms; Linux installation and isolated UI checks do not establish that evidence.

Linux retains the existing Secret Service and private-file credential behavior. macOS uses Keychain and Windows uses Credential Manager through the maintained `keyring` crate. Those platforms fail explicitly if saving to the OS store fails, rather than writing a new unprotected credentials file. Existing compatible account files can still be read.

## Building and publishing

The [release workflow](../.github/workflows/release.yml) builds all five native targets on GitHub-hosted runners. A separate Linux x86_64 job runs the full workspace and installer tests alongside the native builds; publishing requires both tests and every platform build to pass. All targets build the executable and packaging tool together, then check the workspace and examples in the same release profile and check the executable's version. Packaging invokes the compiled Rust tool directly, avoiding a separate development build or another Cargo invocation with a different dependency feature selection. Thin LTO and the app's release optimization remain enabled. The Rust packaging operator produces Unix archives or a single Windows executable:

Release downloads include their version in the filename: `leet-linux-vX.Y.Z`, `leet-linux-arm-vX.Y.Z`, `leet-mac-intel-vX.Y.Z`, `leet-mac-arm-vX.Y.Z`, and `leet-windows-vX.Y.Z.exe`. Unix archives use `.tar.gz`; Linux also publishes `.AppImage` files for both Linux architectures. The old `leet-windows-x86_64.exe` download name remains as a landing-page compatibility link.

```sh
cargo build --locked --release -p gui --target x86_64-unknown-linux-gnu
./ops release:package --target x86_64-unknown-linux-gnu --tag v0.2.0 --output dist
```

On Windows, invoke the same Rust operator directly:

```powershell
cargo run --locked --release -p practice --example release --target x86_64-pc-windows-msvc -- --target x86_64-pc-windows-msvc --tag v0.2.0 --output dist
```

Push a version tag to release it:

```sh
git tag -a v0.2.0 -m "leet v0.2.0"
git push origin v0.2.0
```

Use the GitHub Actions **Release → Run workflow** button with an existing `vX.Y.Z` tag to retry/rebuild that release. The selected tag must exist; every build checks out that tag. Publishing waits for every target, uploads archives, the executable, installer, and SHA-256 manifest to a draft, and then publishes it. Failed builds do not publish an incomplete release. Workflow tokens need `contents: write` only in the publishing job. Never put personal credentials or data into build artifacts.

Rust builds use the maintained `Swatinem/rust-cache` action, pinned to the verified v2.9.2 commit, for Cargo downloads and compiled dependencies. Test and release caches are separate; native caches include Cargo/Rust/compiler flags and the macOS deployment target and are isolated by runner OS/version, target architecture, compiler identity, lockfile, manifests, build configuration, Windows CRT flags, and macOS deployment target. The action removes workspace artifacts, incremental directories, and unused packages before saving, keeping caches focused on reusable dependencies. Cargo still builds the current app sources on every run; a cache hit never skips checks or tests. CI invokes `cargo test --locked --workspace` and `python3 tests/install.py` directly, covering the same tests as `./ops check --json` without first compiling the local deployment operator.

Changes to Rust sources, manifests, native configuration, tests, the operator, or the release workflow on **main** run all five native builds plus Linux tests and warm their caches. Superseded main builds are canceled. Documentation-only and landing-page-only changes do not start native builds. Main builds do not package or publish a release.

For a manual cache warm-up, select **main** in **Release → Run workflow** and leave **tag** empty. To test release packaging without replacing published assets, enter an existing tag and disable **publish**. GitHub scopes caches to the workflow's branch: caches saved on main are available to future tags; caches saved on one tag cannot be read by another tag. Only main runs save Rust caches to avoid filling storage with redundant tag caches. The first run is cold. New compiler versions or native build changes can require a fresh cache.

The Vite landing page lives in `site/` and is deployed to Cloudflare Pages at `https://leet.allpyq.in`. Static HTML and inline hero CSS paint the install command first; platform tabs and copying use browser APIs. GSAP, Lenis, and OGL load lazily for motion, with React/Sonner used only for feedback. Titles use Nunito and the page follows Vesper’s peach and mint palette. Videos load near the viewport; reduced motion and Save-Data retain posters. Cloudflare Pages builds the `site/` directory and publishes its `dist/` output from `main`; its build watch paths include the site and its shared logo.

```sh
cd site
pnpm install --frozen-lockfile
pnpm ops --help
pnpm ops check --json
pnpm dev
# http://localhost:1337/
```

Cloudflare Pages installs from the frozen `site/pnpm-lock.yaml` lockfile and runs `pnpm run build`; `pnpm ops check --json` is the local site check. Release uploads skip redundant ZIP compression and expire after three days; the published GitHub Release assets are retained independently.

The landing page keeps requirements in this document and release notes. Its only repository link is the header's GitHub icon, adapted from Primer Octicons with its MIT license in `site/src/icons/LICENSE`.

`./ops check --json` covers Rust and isolated installer regressions; `pnpm ops check --json` in `site/` covers platform detection and the production page build. A successful build/version probe does not establish native window rendering, account-store behavior, or OS download-policy behavior on another computer. Validate those on the corresponding desktop before claiming device-level support.
