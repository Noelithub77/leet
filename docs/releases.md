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

Leet downloads the tools needed to run local Python, C, C++, Go, and Java solutions in the background. Continue in onboarding stays available during setup. LeetCode's remote judge remains available with an account.

## Private language tools

Onboarding and ordinary startup queue all supported language tools, with the preferred language first. Setup keeps running after onboarding closes. The status bar shows the current stage, transfer progress when available, elapsed time, queued jobs, Ready, or Retry with failure details on hover. Debug mode also offers Set up tools. Copilot downloads and connection stages have a separate indicator in the status bar and Settings; GitHub browser approval remains a user action.

Leet downloads pinned archives and wheels, verifies SHA-256 digests, and keeps them in its user data directory under `leet/tools`. Existing vg data directories remain compatible. Installation preserves system tools, system PATH, and personal solutions. It needs no system package manager or administrator access. The app selects managed executables directly and keeps explicit custom Python paths authoritative.

Python includes portable CPython, basedpyright, private Node, and debugpy. C/C++ include Zig's compiler and standard libraries, clangd, and CodeLLDB with LLDB. Go includes its official toolchain, gopls, and Delve. Java includes Temurin Java 25, Eclipse JDT LS, and the Java debug plugin. Setup prepares compiler caches and private standard-header paths for the editor. See [private toolchains](toolchains.md) for package versions, debugger behavior, and function-harness limits.

The catalogs cover Linux x64 and arm64, macOS Intel and Apple Silicon, and Windows x64. Pinned URLs and hashes live in `crates/practice/setup/tools.json` and `crates/practice/setup/toolchains.json`. Downloads and cache sizes vary by platform and language; setup reports actual transfer bytes. Windows setup uses built-in Windows PowerShell 5.1 and tar, available on Windows 10 version 1803 and newer. macOS setup uses its built-in shell, curl, shasum, and tar. Downloads need internet access. Windows setup, checks, language servers, runners, and debuggers request no console window.

For an isolated install or a retry, use the installed binary or the tracked operator:

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

Prepare and commit the release notes in `docs/changelog.md`, then publish from a clean `main` checkout with Git and an authenticated GitHub CLI:

```sh
./ops release:publish --version 0.4.1 --dry-run --json
./ops release:publish --version 0.4.1 --json
```

The operator updates the workspace and lockfile versions, commits only those two files, creates an annotated tag, atomically pushes the commit and tag, and starts one Release workflow on `main`. Its result reports queued publication and the workflow URL; publication completes only after CI succeeds. It refuses dirty checkouts, reused tags, older versions, and branches other than `main`. A failed push leaves the version commit and tag local; a failed dispatch reports the exact command to retry without creating another version.

The **Publish release** script in `t3.json` asks for a version in its terminal and calls the same operator. Import it once through **Settings → Project → Actions → Import scripts**. T3's project-file schema has no parameter field, so the terminal prompt supplies the input; the operator itself remains non-interactive.

Use the GitHub Actions **Release → Run workflow** button on `main` with an existing `vX.Y.Z` tag to retry/rebuild that release. The selected tag must exist; every build checks out that tag. Publishing waits for every target, uploads archives, the executable, installer, and SHA-256 manifest to a draft, and then publishes it. Failed builds do not publish an incomplete release. Workflow tokens need `contents: write` only in the publishing job. Never put personal credentials or data into build artifacts.

Rust builds use the maintained `Swatinem/rust-cache` action, pinned to the verified v2.9.2 commit, for Cargo downloads and compiled dependencies. Test and release caches are separate; native caches include Cargo/Rust/compiler flags and the macOS deployment target and are isolated by runner OS/version, target architecture, compiler identity, lockfile, manifests, build configuration, Windows CRT flags, and macOS deployment target. The action removes workspace artifacts, incremental directories, and unused packages before saving, keeping caches focused on reusable dependencies. Cargo still builds the current app sources on every run; a cache hit never skips checks or tests. CI invokes `cargo test --locked --workspace` and `python3 tests/install.py` directly, covering the same tests as `./ops check --json` without first compiling the local deployment operator.

Pushes to `main` and version tags do not start native builds automatically. The release operator explicitly starts one workflow, which tests, builds, packages, and publishes without a second validation run. Concurrent requests for the same release are serialized.

For an optional manual validation, select **main** in **Release → Run workflow** and leave **tag** empty. To test release packaging without replacing published assets, enter an existing tag and disable **publish**. Release dispatches run on `main` while checking out the selected tag, so their caches remain available to subsequent releases. Only workflows dispatched on main save Rust caches. New compiler versions or native build changes can require a fresh cache.

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
