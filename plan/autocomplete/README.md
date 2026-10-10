# Fast autocomplete

[chosen] Reuse the existing Nucleo search engine and editor menu. Retain the full snippet library, search indexes and static previews in memory; refresh all tabs when snippets change. Return local matches immediately and merge asynchronous language-server results without changing the selected item. Expand only the accepted snippet with current document context.

[chosen] Warm the preferred language server at normal startup on the background executor, retain started servers across language switches, and reuse them for documents. Keep isolated fixtures free of real server startup. Retry failed running servers in the background; preserve explicit restart and tool-install reset behavior.

Verify indexed ranking with focused core regressions and autocomplete with the snippet input fixture, including a deliberately delayed server. Deploy once after the milestone.
