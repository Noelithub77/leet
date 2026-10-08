<details open>
<summary>Feat</summary>

- Reopen closed problem tabs with Ctrl+Shift+T, preserving their editor state.

- Open global search with Ctrl+T as an alternative shortcut.
- Label primary and alternative shortcuts; Home uses Ctrl+H with Ctrl+. as an alternative.

- CodeChef provider with its logo and 100 offline beginner/intermediate problems.
- Import CodeChef samples through CPH and run solutions locally in every supported language.
- Background CPH listener opens Leet when closed and reuses tabs in its existing window.

- Per-problem AI chat with custom action instructions, follow-up questions, and locally saved conversations.
- Native resizable Explorer on Alt+S, Description on Alt+A, and AI on Alt+D.
- General, My solution, and Conversation AI tabs with a combined solution review and visual dry run.
- Compact AI navigation with centered empty views and time and space analysis placeholders.
- Show AI-generated edge cases with a sparkle icon.
- Resize the rounded results panel vertically and remember its height.
- Keep Constraints visible below statement content and close other sections when opening it.
- Generate extra test cases from the case toolbar with custom counts, types, and instructions.
- Explorer and Description toggles beside Home, AI and results toggles at the top right, and Code icons on problem tabs.
- Softly rounded panes with very dim orange outlines in Vesper.
- Collapsible platform settings with provider icons, Log in / Log out buttons, and Codeforces handle.
- Navigate Settings tabs with Tab, Shift+Tab, and arrow keys.
- Reset app settings and reopen onboarding while keeping solutions, accounts, progress, and cache.
- Optional keyboard tour of problems, editor, tests, Assist, and debugger, with Skip available throughout.
- Collapse Assist result cards while keeping run status and recovery controls visible.
- LeetCode weekly and biweekly contests alongside Codeforces, with native problem explorers.
- Continue stopped solves or retry interrupted AI actions from their result cards.
- Assist panel with hints, I'm stuck, find bugs, edge cases, complexity, optimize, visualize, dry run, pattern, explain, and solve.
- Run Codex, Claude Code, OpenCode, Antigravity, Gemini CLI, or Cursor Agent locally with live models, reasoning levels, and Fast tier.
- Install OpenCode or Antigravity from the AI menu to use free models.
- Solve edits, tests, and retries LeetCode submissions, asking before each submit.
- Ctrl+` switches between Code and Debug mode.
- Show cached AI models immediately and refresh once per agent per app session.
- Antigravity defaults to the latest available Flash model with low reasoning.
- Syntax highlighting in Debug mode follows the editor theme.
- Highlight the selected case's original input in a larger, colored card above the live debugger state.
- Larger array cells and pointer labels with longer stems and smoother movement.
- Clear headings for the call stack, variables, data structures, and printed output.
- Debug mode replays every test case with a seek bar, line heat, and live data structures for Python and C++.
- Explain a recorded case to mark the step where it first goes wrong.
- Animated arrays, grids, trees, graphs, linked lists, stacks, queues, heaps, maps, sets, and intervals.
- Settings → AI for agent, model, reasoning, Fast tier, and web chat.
- Edit test inputs and expected values inline with a pencil or double-click.
- Java in onboarding, language switching, highlighting, and local contest runs.
- Onboarding checks the selected language's tools and links to missing setup.
- Organize Settings into General, Editor, Appearance, Provider, AI, Accounts, and Keybindings tabs.
- Show current shortcuts with keycaps.
- Animated panel toggles with current shortcut tooltips.
- Compact status-bar popups for updates, AI preferences, and language switching.
- Collapsible Feat and Fix release notes with one Update/Restart action.
- Native updates and Linux AppImages that preserve settings and solutions.
- Replace the bottom-left leet label with the bundled app logo.
- New app and site logo: a peach dot, two mint arrows, and a peach bracket that read 1337.

</details>

<details>
<summary>Fix</summary>

- Automatically acquire three free Companion ports, retry occupied ports, and avoid VS Code CPH’s default port.
- Remove CPH settings and suppress duplicate browser deliveries.
- Fetch complete statements after CPH imports and select the provider from the problem URL.

- Bring Leet forward after CPH imports, show listener errors, and release the import port when restarting.
- Label browser import settings and notifications as CPH.

- Preserve AI pane identity while toggling other sidebars and soften panel transitions.
- Hide stray divider lines between rounded panes while keeping native resizing.

- Keep each problem’s AI instructions when launching actions with chat hidden.

- Keep global shortcuts working when returning to Debug from Roadmap or Settings.
- Show account names or emails without internal IDs or authentication metadata.
- Show debugger step and case shortcuts in their tooltips.
- Prevent onboarding requirements checks from crashing when opened from Settings.
- Spaced provider icons in the closed dropdown and a hover submenu for the active NeetCode list.
- Open Settings on the first Platform tab with every platform expanded by default.
- Show function icons for calls, returns, and stack frames in Debug mode.
- Show “Just a template” instead of executing untouched templates in every language.
- Keep the source, case tabs, and debugger controls visible before recording steps.
- Keep array range labels fully visible above the cells.
- Debug mode shows named user functions and variables without generated runtime frames.

- Settings values line up on the right edge.
- Keep explicit Web mode selected when local agents are installed.
- Stop solve runs on cancellation or problem changes.

- Recognize the migrated ~/leet folder when an older running instance saves ~/vg.

- Remove old app names from account controls, export instructions, and internal action namespaces.
- New users use ~/leet for their solutions.

- Keep Settings tabs horizontal on one row.
- Show the solutions folder using its home-relative path.

- Keep only the active language server running.
- Remove shortcut hints from AI and language popups.
- Popups stay above the status line.
- Local deployments restart into the newly installed build.
- The update icon stays visible without a network connection.
- Language changes keep each language's saved solution.
- Keep AI options in their popup and remove duplicate Settings controls and basic navigation hints.
- Move Contests toward the right edge with a wider gap from Recent and an accent-colored browser icon on every contest row.
- Remove the upcoming-contest explanatory text.

</details>
