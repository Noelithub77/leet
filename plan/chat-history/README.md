# Conversation threads

## Scope

Add multiple private conversation threads per problem and shared root threads in
the Conversation tab. Support creating/selecting threads, forking at a message,
editing requests, undo, and deleting old threads. Preserve existing action and
solution-review views, local agent protocols, credentials, and solution files.

Every AI action is an ordinary thread turn. Render its request as a compact action
card (icon, label, optional user instructions), never as the expanded generated
prompt. Keep answers and follow-ups in that same thread. Preserve the parent-owned
single compact header: Sparkles, General / Analysis / Chats, model, close.

## Design

- Core thread records and persistence belong to `practice`; UI state and agent
  lifecycle remain in `gui`.
- Reuse the private Diesel/SQLite cache and existing serialization dependencies.
- Migrate `assist:<slug>` conversations without losing legacy answer-only turns.
- Give threads and turns stable IDs. Keep each turn's originating problem.
- Only the selected thread supplies bounded prior context to a new request.
- Keep a compact thread list with problem and root scopes; progressively disclose
  message actions and deletion.
- Preserve pending parent UI changes and concurrent CodeChef work. Re-read shared
  files before edits and stage only this task's hunks.

## Confirmed decisions

- **[CHOSEN]** Edit by branching and regenerating; preserve the original.
- **[CHOSEN]** Root sends include the open problem and identify its origin.
- **[CHOSEN]** Undo reverses the latest branch or deletion, including after restart.
- **[CHOSEN]** Dedicated private SQLite thread/message tables.
- **[CHOSEN]** Reuse provider-native tools and a shared native response contract.
- Explicit follow-up authorizes full file/command access through native agents.
- Native response artifacts are editable JSON files using validated Rust schemas.

## Persistence and lifecycle requirements

- Migrate legacy turns transactionally and exactly once; preserve stopped/failed
  requests and answer-only records. Never silently replace malformed records.
- Persist thread selection and per-thread composer drafts; problem navigation must
  not mix drafts or change a selected shared root thread.
- Fork copies the prefix through the chosen message. Editing regenerates from the
  prefix before that request if branching is chosen; later original messages stay
  available in the original thread.
- A mutation must not copy live cancellation handles, agent sessions, or mutable
  playback state. Forked interrupted turns remain stopped historical records.
- Undo affects conversation records only; it cannot undo solution edits, judge
  submissions, or externally opened web conversations.
- Deleting a conversation cancels its live runs and ignores late events. Preserve
  sufficient records for the chosen undo behavior, then remove expired undo data.
- Keep Solve ownership tied to its originating problem, including when a shared
  thread is selected or the user switches threads during streaming.
- Keep the latest solution review in My solution independently of the selected
  conversation thread.

## Verification

Chat search covers readable titles, requests, and response text within the selected
scope. Opening Chats or expanding its list focuses search; Up/Down, Enter, and
Escape navigate results and return to the composer. SQL JSON property names are
excluded from message search, and LIKE wildcards are escaped.


Regression coverage for migration, context isolation, branching, edit/undo,
deletion/restore, and interrupted requests. Run `./ops check --json`, inspect native
UI and keyboard behavior where available, commit focused changes, then run
`./ops local:deploy --json` and verify installed commands separately.

## Progress

- Inspected persistence, composer drafts, streaming/cancellation, recovery, and
  solution action lifecycle. Current storage is a newest-first flat JSON list per
  problem; prompt history is capped at 12 turns and 32,000 characters.
- Thread persistence, migration, scope selection, draft restoration, branch/edit/undo,
  shared response contracts, full native tool access, and artifact loading are
  implemented. Required checks passed: 34 GUI, 134 practice, and 5 installer
  tests. A live Codex turn wrote a validated visualization artifact outside its
  working directory. Native QA confirmed search autofocus, filtering, Enter
  selection, draft restoration, artifact rendering, and fork/undo behavior.
- Focused commit and local release deployment complete this milestone; other
  providers have adapter coverage but were not exercised with live accounts.
