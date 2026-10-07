# Public releases

Publish this repository as **leet**. **[CHOSEN]** Releases start from version tags or a manual workflow.

- Package native Linux, macOS, and Windows builds through GitHub Actions.
- Provide a checksum-verified Unix installer that installs and opens the release without touching user data.
- Build a minimal Vite page with the detected platform's install command or Windows executable download.
- Verify repository checks, installer behavior, site rendering, installed commands, and remote jobs separately.

**[CHOSEN]** Vite as the sole frontend dependency, GitHub Pages hosting, native credential stores through a maintained library, and unsigned first releases.

Implementation is complete; local checks and remote publication are in progress. Native build/version checks do not establish rendered UI or account-store behavior on macOS/Windows.
