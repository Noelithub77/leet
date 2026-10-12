# Provider statement completeness

- Fetch NeetCode's own question HTML using the active provider, with separate statement caching from the LeetCode judge payload.
- Retain complete source statement bodies; preserve samples, notes, nested sections, details, links, math, and media through the renderer.
- Verify provider parsers with complete-content fixtures and bounded live public fetches; keep offline fallbacks explicit.

Verified: the core suite passed 207 tests; live public LeetCode and NeetCode statements, the NeetCode diagram/cache, and CodeChef’s additional sample/explanations passed. The native four-provider statement fixture passed and its captures were inspected. Latest shared parser/media tests passed. Codeforces returned HTTP 403 and its mirror returned a browser challenge; full HTML parsing is fixture-verified and the historical fallback is now explicitly marked as potentially missing images.
