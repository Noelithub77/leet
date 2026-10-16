# Copilot status controls

- [chosen] Clicking the status logo toggles completions. A small adjacent arrow opens usage and a Settings action; the disabled logo has a cross.
- [chosen] Settings owns account connection and sign-out, with the reported GitHub username when available. Pausing completions preserves authentication.
- Use the server's `checkStatus` account response and `signOut` request; never read credential files or guess the account name.
- Verify request serialization and sign-out with the existing fake server, then exercise the production toggle, menu, settings navigation, and account controls in the isolated Copilot fixture.
