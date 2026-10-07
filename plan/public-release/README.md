# Public releases

Publish this repository as **leet**. **[CHOSEN]** Releases start from version tags or a manual workflow.

- Package native Linux, macOS, and Windows builds through GitHub Actions.
- Provide a checksum-verified Unix installer that installs and opens the release without touching user data.
- Build a minimal Vite page with the detected platform's install command or Windows executable download.
- Verify repository checks, installer behavior, site rendering, installed commands, and remote jobs separately.

**[CHOSEN]** Vite with React/Tailwind for the user-requested shadcn keyboard component, GitHub Pages hosting, native credential stores through a maintained library, and unsigned first releases.

The repository is public, GitHub Pages is live, and all five native builds and publishing passed for `v0.1.0`. The public Linux installer passed checksum, version, alias, and native-window checks in an isolated profile. Native build/version checks do not establish rendered UI or account-store behavior on macOS/Windows.

The updated landing page combines Linux/macOS, uses Vesper teal at a 130% desktop type scale, shows the app logo and “leet for the eleet,” and copies via Ctrl+C/Cmd+C. The initial logo splash ends when the page is ready. Local site checks, keyboard behavior, mobile layout, and the standalone splash passed; GitHub Pages publishes the committed update from main.
