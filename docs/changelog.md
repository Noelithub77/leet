<details open>
<summary>Feat</summary>

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
