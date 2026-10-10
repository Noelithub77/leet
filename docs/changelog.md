<details open>
<summary>Feat</summary>

- Choose Monokai Pro Spectrum for a complete dark theme with distinct syntax colours.
- Start a chat in the assistant pane with Ctrl+T, Ctrl+Shift+O, or Ctrl+N.
- Optional GitHub Copilot suggestions: hold Alt to preview and accept with Tab or Alt+L.
- Hover the Copilot logo in the status bar to see connection status and reported usage.
- See a subtle Alt cue when Copilot has a suggestion ready.
- Preview the selected snippet's expanded code in the completion menu.
- 205 competitive-programming snippets and file templates for Python, C++, Go, C, and Java.
- Expand prefixes with Tab and browse snippets with Ctrl+J.
- Linked placeholders, choices, transforms, and a cursor that glides between stops.
- Create snippets with draggable stops, a final cursor, and a live preview in Ctrl+Shift+S.
- Import VS Code, Neovim, and Sublime snippets in a skippable onboarding step or the editor.
- Ask the snippet assistant or chat agents to configure snippets, with preference questions and undo.
- Review and edit snippet-assistant proposals before applying them to your library.
- Download and install updates in the background; restart now or on the next launch.
- Set up private Python and clangd tools automatically during onboarding, with background progress and retry in the status bar.
- Open statement cards independently and remember expanded cards across restarts.
- Show pastel scrollbars on overflowing statement cards and scale long descriptions for readability.

</details>

<details>
<summary>Fix</summary>

- Select the first autocomplete result unless you navigate to another suggestion.
- Give Vesper clearer pastel syntax colours and brighter comments.
- Show local autocomplete immediately while language-server results load.
- Prefer keywords, variables, and functions ahead of snippets in autocomplete.
- Fuzzy-search snippet names, descriptions, prefixes, and aliases from an in-memory index.
- Hot-reload snippet changes across editor tabs without restarting.
- Warm IntelliSense in the background at startup and retain servers across language switches.

- Fix Copilot sign-in rejecting the request format.
- Hide empty chat placeholders from history.
- Show the selected conversation title once.
- Keep empty history free of placeholder controls.
- Align the chat toolbar with the history list.
- Give the chat picker, thread list, and selected conversation clear borders.

- Show each command once in search, with its current keybindings beside it.
- Merge snippets into the compact IntelliSense menu below the cursor, with highlighted previews.
- Identify completion kinds with small icons.
- Give completion previews more room and fade their clipped edge smoothly.
- Create snippets for the selected language without an all-languages toggle.
- Edit snippets in a workspace tab with Explorer access and Ctrl+W closing.
- Label snippet fields and explain placeholders in titled help tooltips.
- Show bundled snippet inputs, outputs, and short examples in the preview.
- Explain snippet expansion and placeholders in the skippable onboarding step.
- Suppress Windows console popups from background tools, updates, Companion, and local execution.
- Resolve file-template variables from the new solution's filename and workspace.
- Keep tour shortcuts ahead of editor input and retain keyboard focus when advancing.
- Continue onboarding while language tools are missing or still being checked.
- Start with Description and Examples expanded before any card preferences are saved.
- Keep Hint compact in the problem toolbar, with its shortcut shown on hover.

</details>
