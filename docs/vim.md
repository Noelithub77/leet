# Vim keybindings

Enable **Settings > Editor > Vim keybindings**, or choose **Toggle Vim keybindings** in the command palette. The setting defaults to off, applies immediately to code editors and persists across restarts. New editors start in Normal. Name, search, chat and other ordinary inputs keep their usual keys.

A compact indicator shows the current mode, pending count or command, and command messages. **Vim command help** in the palette opens the supported bindings. Vim keys take precedence over conflicting app shortcuts while a code editor has focus.

## Modes and clipboard

| Key | Effect |
| --- | --- |
| `i/I/a/A/o/O` | Enter Insert before/after the cursor, at line boundaries, or on a new line |
| `Esc` or `Ctrl+[` | Dismiss suggestions, finish snippet navigation and return to Normal |
| `v` | Visual character selection |
| `V` | Visual line selection |
| `Ctrl+v` in Normal or Visual character/line | Visual Block selection |
| `Ctrl+v` in Visual Block | Leave the rectangle and return to Normal |
| `Ctrl+v` in Insert | Native clipboard paste, staying in Insert |

**Insert `Ctrl+v` intentionally differs from Vim's literal-character insertion.** It pastes the actual clipboard text using Leet's normal paste behavior, including multiline text and selection replacement. Empty clipboard content leaves the text unchanged. Paste is one undoable edit and its inserted content is included in dot repeat.

Insert mode retains native indentation, paired brackets, completions and snippets. `Ctrl+w` deletes the preceding word and `Ctrl+u` deletes to the start of the line. Independent extra cursors are unavailable while Vim keybindings are enabled.

## Supported commands

| Area | Bindings |
| --- | --- |
| Movement | `h/j/k/l`, arrows, `w/W/b/B/e/E/ge/gE`, `0/^/$`, `gg/G`, `{`/`}`, `%`, `f/F/t/T`, `;`/`,` |
| Counts | Prefix motions and operators, including multiplied counts such as `2d3w` |
| Operators | `d/c/y`, `dd/cc/yy`, `D/C/Y` |
| Editing | `x/X`, `s/S`, `r`, `J`, `>>/<<` |
| Text objects | Inner/around words, WORDs, quotes, parentheses, brackets and braces, such as `iw`, `aW`, `i"`, `a(` |
| Visual | Inclusive selections, motions/counts, text objects, `o/O`, `gv`, yank/delete/change/replace, indentation and paste |
| Visual Block | Rectangular yank/delete/change/replace/put; `I/A` insert across selected rows |
| History | Unnamed register, `p/P`, `u`, `Ctrl+r`, `.` |
| Search | Literal `/` and `?`, `n/N`, whole-word `*`/`#`, wraparound |
| Scrolling | `Ctrl+d/u/f/b`, `zz/zt/zb` |
| Commands | `:w`, `:q`, `:wq`, `:x`, `:<number>`, `:$` |

The unnamed register preserves characterwise, linewise or blockwise content. Use Up/Down in `/` or `?` to recall literal queries. Register contents, last change and search history are shared across workspace code editors; each editor keeps its own mode and selection.

Rectangles use display columns, including tabs and wide characters. Short or empty rows follow Vim's default virtual editing behavior. Block insert/change replicates the first row's insertion to eligible rows when insertion finishes. Each completed block operation is one undo unit.

Read-only code allows navigation, selection, yank and search. Editing commands cannot change it. Leaving an editor clears incomplete commands and finishes an open insertion transaction. Turning Vim off preserves text and history.

## Saving and closing

`:w` uses the existing solution or snippet save flow. `:q` uses the existing tab close flow, which saves changes before closing. `:wq` and `:x` close only after saving succeeds; a failed save leaves the editor open and displays the error. `:x` avoids writing unchanged text. Unsupported commands display a message. `:<number>` and `:$` navigate within the current code editor.

## Deferred commands

Continuous Replace mode, named/numbered registers, macros, marks, jump lists, substitutions, regex search, custom mappings, vimrc, plugins and Vim window management are outside this release. Named registers are the recommended next extension.
