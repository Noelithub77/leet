# Launch updates and portable Linux releases

- **[CHOSEN]** One background latest-release request per application launch. Cache its metadata in memory; download only after the user activates the existing update UI.
- **[CHOSEN]** Maintained `self_update`, using ureq, checksum verification, safe native executable replacement, and full macOS bundle replacement.
- **[CHOSEN]** Hover the bottom-right button for a scrollable changelog; click to install and restart. Keep the badge for release updates and local deployments.
- Save editor state before restarting; cancel restart if saving fails. Installation touches only application binaries/bundles, never configuration, credentials, progress, caches, or solution repositories.
- Preserve local development build notifications. AppImage launches update their original AppImage file, not the mounted executable.
- Package both Linux AppImages in the existing native matrix jobs, reusing compiled binaries and bundling eligible shared libraries.
- Verify request count, platform/version selection, invalid checksums, safe installation, workflows, local build and deployment. Distinguish local proof from remote platform results.
