<details open>
<summary>Feat</summary>

- Revert accepted Copilot suggestions with Alt+K.
- Distinguish Copilot suggestions with faint syntax colors.
- Save AI findings as `#!` comments; Rich hides them and highlights the code, with details on hover.
- Show note hovers as cards with a kind chip, step bar, labelled detail, and shortcut keys.
- Resolve notes with Alt+Shift+R.
- Ask about a line with Alt+Q as a follow-up in the chat that wrote its note.
- Tag quick-action chats in history with colored chips, like Stuck and Analyse.
- Ask the AI in place with `#?` questions; answers appear below them.
- Switch notes between Rich, Raw, and Off; hidden notes stay out of copies.
- Toggle notes Off and back with Ctrl+H.
- Glance at Home with Alt+H and press it again to return.
- Step through I'm stuck reasoning trails with Alt+] and Alt+[.
- Show complexity costs and last debugger values in line hovers.
- Preview and flash cited lines from chat line links, even after edits.
- Leave AI notes and questions out of judge submissions.

</details>

<details>
<summary>Fix</summary>

- Keep tooltips compact, with wrapped descriptions and balanced padding.
- Show NeetCode’s own descriptions, examples, diagrams, and complexity guidance.
- Keep CodeChef’s additional samples and explanations in the question view.
- Preserve nested statement sections and sample notes across providers.
- Load and cache question diagrams when opening statements across practice platforms.
- Replace the running development app when rerunning Dev.
- Fix a crash when starting an AI run from the workspace.
- Merge General into Chats while keeping Analysis separate.
- Group chat history and search in one container with separated rows.
- Keep chat rows neutral until hover or keyboard navigation.
- Give the assistant header balanced spacing, with New chat beside Close.
- Prioritize your own declarations and keep CP-relevant Python auto-imports last.

</details>
