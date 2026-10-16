# Copilot integration capabilities

Research checked 2026-10-17. This records protocol evidence and implementation recommendations, not a claim that every listed feature is implemented or verified with a signed-in account.

## Version boundary

Leet pins native Copilot Language Server **1.552.0** in `crates/practice/setup/copilot.json`. Research inspected the same version's official npm JavaScript distribution, including its bundled README and registered method handlers. That establishes package-level API evidence; it does not independently prove every native build's runtime behavior or account entitlement. No credentials, authentication, or live model requests were used.

Sources: [versioned npm metadata](https://registry.npmjs.org/@github/copilot-language-server/1.552.0), [versioned package archive](https://registry.npmjs.org/@github/copilot-language-server/-/copilot-language-server-1.552.0.tgz), [Leet pin](../crates/practice/setup/copilot.json).

The current [official protocol README](https://github.com/github/copilot-language-server-release#communication-protocol) is a moving document. Its unified `textDocument/copilotSuggest` method is absent from the inspected 1.552.0 package's README and handler registry. Do not call it on the pinned version or infer its first supported version from current documentation. The separately documented next-edit method **is** present in 1.552.0.

## Features suitable for this client

| Feature | Evidence and recommendation |
| --- | --- |
| Inline completions | Documented `textDocument/inlineCompletion`; retain current implementation and stale-document checks. Preserve opaque result metadata and execute the returned acceptance command after acceptance. |
| Next edit suggestions | Documented and registered `textDocument/copilotInlineEdit`; supports replacing/deleting text away from the cursor. Add an explicit diff preview, version/range validation, Apply/Discard, and undo. Do not apply it as append-only ghost text. |
| Open-file context | Documented `didOpen`, `didChange`, `didClose`, and custom `textDocument/didFocus`. Keep actual open solution buffers synchronized, including unsaved changes, and send focus changes. Register actual workspace roots. A folder registration does not prove exhaustive repository indexing. |
| Completion alternatives | Documented `textDocument/copilotPanelCompletion`, with optional `partialResultToken` streaming. Useful later for an explicit alternatives panel, not required for Ctrl+I. |
| Partial acceptance | Documented `textDocument/didPartiallyAcceptCompletion` reports the full item and cumulative accepted UTF-16 length. Add only alongside a real word/line acceptance interaction. Panel completions do not support shown/partial-accept events. |
| Cancellation | Documented `$/cancelRequest`; cancel obsolete work when the buffer, selection, focus, or enabled state changes. |
| Account/status | Documented `signIn`, `signOut`, `didChangeStatus`; the pinned package additionally contains `checkStatus`. Keep disabling predictions separate from signing out. Show account identity only when returned by the server. |
| Model selection | Source-derived `copilot/models` is registered in 1.552.0. Official copilot.vim filters returned models by the `completion` scope and sends `selectedCompletionModel` through configuration. Discover account-available models; do not hardcode a catalog or assume chat models work for completions. |

Documented protocol evidence is in the **1.552.0 archive's README**, particularly Text Document Synchronization, Text Document Focusing, Inline Completions, Next Edit Suggestions, Panel Completions, and Cancellation. Current counterparts: [synchronization](https://github.com/github/copilot-language-server-release#text-document-synchronization), [next edits](https://github.com/github/copilot-language-server-release#next-edit-suggestions), [panel completions](https://github.com/github/copilot-language-server-release#panel-completions). Model selection evidence: [official copilot.vim implementation](https://github.com/github/copilot.vim/blob/release/autoload/copilot.vim), `s:commands.model`, and [configuration transport](https://github.com/github/copilot.vim/blob/release/autoload/copilot/client.vim).

## Exact next-edit request and result

The pinned README documents this request, without an instruction prompt:

```json
{
  "textDocument": { "uri": "file:///solutions/example.py", "version": 12 },
  "position": { "line": 4, "character": 8 }
}
```

The response contains `edits`, each with `text`, a versioned `textDocument`, a replacement `range`, and an optional acceptance `command`. Treat edits as alternatives unless a stronger protocol contract establishes otherwise. Send `textDocument/didShowInlineEdit` with the shown item. Execute that item's own returned command through `workspace/executeCommand` after applying it; do not hardcode an acceptance command from another client/version. Check the document version and exact original range again before applying. Retain the full item for feedback.

**Cache reporting is present in 1.552.0**, not just the current main README. Its bundled README documents optional `cacheTelemetryContext` and `textDocument/reportCachedInlineEdit`; its JavaScript implementation generates the field and registers the notification handler. A newly requested provider suggestion held until its first Alt preview is the original provider opportunity: retaining it does not by itself require inventing a separate cache opportunity. Preserve its original metadata and use normal shown/acceptance feedback. Replaying a suggestion as a new client-cache opportunity is different: create a fresh opportunity ID, retain the opaque context, then report one outcome with `opportunityId`, `context`, `isShown`, and `acceptance` (`accepted`, `rejected`, or `notAccepted`), optionally `disposalReason`. Do not send ordinary provider feedback for synthetic cache IDs. This lifecycle distinction follows the pinned documentation's separation of cached copies from ordinary provider feedback; no signed-in telemetry observation was performed. Avoid client-cache replay until that lifecycle is implemented.

## Instruction-driven editing and Ctrl+I

Next-edit prediction has no documented user-instruction field. `workspace/executeCommand` is a transport for particular server commands, not a generic instruction-edit endpoint. VS Code's inline-chat commands belong to the editor extension and are not portable LSP methods; see its [official extension manifest](https://github.com/microsoft/vscode-copilot-chat/blob/main/package.json).

The pinned JavaScript server **does register** `conversation/create` and `conversation/turn`. These are source-derived APIs omitted from its public protocol README. GitHub's official Xcode client uses them directly over the language-server connection: [request dispatch](https://github.com/github/CopilotForXcode/blob/main/Tool/Sources/GitHubCopilotService/LanguageServer/GitHubCopilotRequest.swift), [conversation types](https://github.com/github/CopilotForXcode/blob/main/Tool/Sources/ConversationServiceProvider/LSPTypes.swift), [progress types](https://github.com/github/CopilotForXcode/blob/main/Tool/Sources/GitHubCopilotService/LanguageServer/GitHubCopilotRequestTypes/Conversation.swift). They are real implementation evidence, not a stable public instruction-to-text-edit contract. Their result identifies the conversation/turn; streamed reply text is not itself a validated LSP edit.

**Chosen product direction:** Ctrl+I uses the currently selected agent in a focused, fast chat interaction, in fast completion mode. It sends the whole file, the target lines, the problem statement, the judge interface, and examples and tests, and a diff is reviewed through Apply/Discard. The agent may use tools in a scratch directory but is told to return code quickly; an optional short note is shown above the diff. It does not depend on Copilot's private conversation endpoints, silently switch providers, or reinterpret next-edit prediction as instruction editing. Keep normal completions and automatic next edits on the dedicated Copilot client. Broader agent exploration and multi-file editing remain separate from this focused interaction.

## Verification limits

The GitHub tree API returned `403: rate limit exceeded`; research used official source archives/raw files and versioned npm artifacts instead. Current Xcode/Vim source may target different server versions; claims about pinned support above additionally rely on the inspected 1.552.0 method registry. Native protocol smoke tests and signed-in model behavior remain separate verification steps. No new dependency or server upgrade is required by these recommendations.
