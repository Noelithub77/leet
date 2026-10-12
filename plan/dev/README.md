# Development restart

- Build the same dev profile, gui-test feature, and selected Cargo target directory used for focused GUI checks.
- After a successful build, replace only the current user's matching debug executable running from this checkout. Exclude installed apps, Companion, and fixtures. Serialize restarts and verify process identity before stopping.
- Isolated process tests passed for replacement after a rebuild, leaving other executables and checkouts running, and preserving the dev app after a build failure. No active-desktop input or release installation is needed.
- Update the Dev declaration and operator help. Explain that recompilation plus restart is separate from hot reload.
