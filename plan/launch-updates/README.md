# Launch updates and portable Linux releases

- **[CHOSEN]** One background latest-release request per application launch. Cache its metadata in memory; download only after the user activates the existing update UI.
- **[CHOSEN]** Maintained `self_update`, using ureq, checksum verification, safe native executable replacement, and full macOS bundle replacement.
- **[CHOSEN]** Hover or click the bottom-right icon for a popup above the status line. Its single Update action becomes Restart after installation; restart is explicit. Keep the badge for release updates and local deployments.
- Save editor state before restarting; cancel restart if saving fails. Installation touches only application binaries/bundles, never configuration, credentials, progress, caches, or solution repositories.
- **[CHOSEN]** Classified Feat and Fix bullet lists in every release, with Feat expanded and Fix collapsed by default. AI and language controls open compact popups above their status-bar entries.
- **[CHOSEN]** Three panel toggles beside Settings reuse existing actions, live shortcut tooltips, and panel springs, with a 160 ms active-color transition.
- Preserve local development build notifications. AppImage launches update their original AppImage file, not the mounted executable.
- Package both Linux AppImages in the existing native matrix jobs, reusing compiled binaries and bundling eligible shared libraries.
- Verify request count, platform/version selection, invalid checksums, safe installation, workflows, local build and deployment. Distinguish local proof from remote platform results.
