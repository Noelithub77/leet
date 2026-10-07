# Language setup

- **[CHOSEN]** Java uses the existing language paths, LeetCode judge, local `javac`/`java` stdin runner, and Eclipse JDT LS when installed.
- **[CHOSEN]** Enable GPUI Kit's maintained Java grammar feature, approved by the user.
- Check only the selected language during onboarding, off the UI thread. Show missing requirements with official setup links and a Recheck action; do not install system tools automatically.
- Keep a single active language server. Clear inactive editor adapters, invalidate pending attachments, and explicitly stop replaced servers, including servers retained by pending requests.
- Preserve separate saved solutions, personal configuration, and accounts. Remove the AI/language popup shortcut footers; retain keyboard behavior.
- Verify Java compilation, failures and timeouts; selected-language requirements; inactive server disposal; workspace checks; and the installed native build.
