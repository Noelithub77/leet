<details open>
<summary>Feat</summary>

- Vesper-themed README with an animated logo, real demos, and quick installation.
- Contribution and security guidance; current source uses GNU AGPL v3.
- New landing page with real app footage, an interactive debugger replay, and keyboard copy with feedback.
- Synchronize the hero logo and title, add breathing room, and refine installation keycaps and Windows download.
- Versioned release downloads with readable platform and architecture names.

- Keep Description, Examples, and Constraints visible as pastel-bordered cards with one section open at a time.

- Replay Python and C++ Codeforces and CodeChef programs in Debug, including CPH samples.

- Native LaTeX math in statements, chat replies, and structured AI answers.
- Click NeetCode to return to the saved list; hover to choose another list.

- Reopen closed problem tabs with Ctrl+Shift+T, preserving their editor state.
- Close all problem tabs with Ctrl+Shift+W and toggle tags with Alt+T.

- Open global search with Ctrl+T as an alternative shortcut.
- Label primary and alternative shortcuts; Home uses Ctrl+H with Ctrl+. as an alternative.

- CodeChef provider with its logo and 100 offline beginner/intermediate problems.
- Import CodeChef samples through CPH and run solutions locally in every supported language.
- Background CPH listener opens Leet when closed and reuses tabs in its existing window.

- Multiple chat threads per problem with saved drafts, message forks, branch edits, deletion, and undo.
- Search the open problem’s chat threads with automatic search focus.
- Send chats with Enter, add lines with Shift+Enter, and keep send/stop inside the prompt box.
- Use every prompt shortcut in ordinary chats, with readable prompts on hover and shared native agent capabilities.
- Let agents write and revise native visualization artifacts directly in private files.
- Native resizable Explorer on Alt+S, Description on Alt+A, and AI on Alt+D.
- General, Analysis, and Chats AI tabs with a combined solution review and visual dry run.
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
- A centered, animated guided tour appears once for every user, with step icons, clickable actions, and native shortcut keycaps.
- The tour moves aside when you try a step with its action or keyboard shortcut.
- Learn the Explorer, problem description, and AI sidebar toggles in the guided tour.
- Start with Explorer open and NeetCode 150 selected on new installs.
- Collapse Assist result cards while keeping run status and recovery controls visible.
- LeetCode weekly and biweekly contests alongside Codeforces, with native problem explorers.
- Continue stopped solves or retry interrupted AI actions from their result cards.
- Assist panel with hints, I'm stuck, find bugs, edge cases, complexity, optimize, visualize, dry run, pattern, explain, and solve.
- Run Codex, Claude Code, OpenCode, Antigravity, Gemini CLI, or Cursor Agent locally with live models, reasoning levels, and Fast tier.
- Install OpenCode or Antigravity from the AI menu to use free models.
- Quick Solve writes first and asks to submit; checks and fixes follow a rejected attempt.
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

- Keep tour shortcuts visible during practice; use Prev/Next with arrow keys and arrow keycaps.
- Ctrl+P opens full search, including settings, just like Ctrl+Shift+P.

- Keep tour navigation working after changing keyboard shortcuts, and restore the workspace layout when the tour ends.

- Preserve provider formulas through statement conversion and keep math out of literal code.
- Retry blocked Codeforces statements with installed curl before the snapshot fallback.
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
