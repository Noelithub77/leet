# Startup and guided tour

- [chosen] Show the tour once for every user, recording Skip or Done in the local database. Start it after setup for new users.
- [chosen] Use a centered walkthrough with concise tips, native keycaps, progress, and quiet secondary navigation.
- Move the guide into a compact corner surface while the user tries a step by mouse or keyboard.
- Enable the explorer by default on Home and in the editor; retain saved editor layout choices. Keep NeetCode 150 as the fresh-install default.
- Verify startup persistence and real tour input in isolated fixtures, inspect screenshots, run operator checks, commit, and deploy locally.

Implementation and verification are complete. The fixture covers persistence
across workspaces, all eight steps, mouse and keyboard actions, Left/Right navigation, visible practice shortcuts,
guide placement,
light and dark themes, minimum window size, and zoom. Operator checks pass;
the tour recording contains 240 offscreen frames encoded at 60 fps. Native
macOS rendering and physical compositor performance remain unverified.
