<details open>
<summary>Feat</summary>

- Sign in to LeetCode, NeetCode, and Take U Forward in a private window on Windows and macOS, with manual import available.
- Browse Striver A2Z and SDE sheets by section; open linked LeetCode problems in the editor and other entries on TUF.
- Connect a TUF account and sync sheet completion marks.
- Edit selected code with Ctrl+I in a floating card at the selection, with a distinct highlight, a reviewable diff, and an optional agent note.
- Reopen recent Ctrl+I instructions with Ctrl+Shift+I or Up and Down.
- Send the whole file, problem statement, examples, and tests with Ctrl+I, using a fast completion mode.
- Use a compact icon toolbar beside selected code, including Edit with Ctrl+I.
- Preview Copilot next-edit suggestions as a diff, using context from every open solution tab.
- Redesign AI chat with right-aligned requests, flat replies, and smaller text.
- Give chat more room with a one-row header, a history dropdown, and a one-line prompt that expands on focus.
- Jump through chat prompts from a compact side rail or with Shift+Up and Shift+Down.
- Turn quick actions into removable cards in the chat prompt, so you can add details before sending.
- Toggle Copilot completions from its status icon, with a crossed logo when disabled and a quick menu for usage and Settings.
- Show the connected Copilot account in Settings and sign out separately from disabling completions.
- Reset the current solution to its template from the editor toolbar, with Undo support.
- Navigate parameter overloads with Alt+Up and Alt+Down.
- Archive chats from history; restore or permanently delete them in General settings.
- Use Frizbee for typo-tolerant autocomplete while keeping local symbols first.
- Show parameter hints automatically inside calls and alongside Ctrl+Space completions.
- Show subtle cyan feedback when a guided-tour shortcut is used.
- Choose Rich, Raw, or Off from a compact annotation dropdown.
- Enable Vim keybindings for solutions and snippets, with visual blocks, search, repeat, and command help.

</details>

<details>
<summary>Fix</summary>

- Give parameter hints more height, bottom padding, and a draggable scrollbar.
- Show Quote, Explain, and Fix shortcuts directly in the selection toolbar.

- Show an agent's plain-text reply as chat instead of an error when it skips the structured format.
- Keep the selection when right-clicking so context-menu actions apply to it.
- Hide code hover cards over selected text.
- Simplify the status bar by removing the LeetCode account label and showing only the search shortcut.
- Keep the Copilot preview cue inside parameter hints so it cannot cover the signature.
- Give parameter hints a full-width strip above completions, with overload shortcuts on the arrows.
- Align snippet placeholders and the caret with selected text while scrolling and zooming.
- Resolve C++ judge-provided standard-library names without changing saved solutions.
- Restore C++ highlighting for shared keywords, types, numbers, strings, and comments.
- Keep completions close to the caret and soften suggestion text changes with a fast transition.
- Toggle the AI panel without a status toast.
- Make code text and completion panels smaller, with suggestions positioned farther right.
- Avoid repeated document copies during Vim navigation and typing.
- Always wrap long code and text in editors and the debugger.
- Keep debugger highlights and scrolling aligned with wrapped source lines.

</details>
