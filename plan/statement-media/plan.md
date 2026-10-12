# Complete statement media

- Preserve statement HTML and media through semantic parsing, including examples.
- Resolve platform-relative and lazy image sources; load images asynchronously through GPUI with the existing core HTTP client and a disk cache.
- Upgrade text-only imported statements when opening, keeping cached text on network failure.
- Verify parser regressions, cached downloads, and focused native statement rendering; commit only these changes.

Completed: parser/cache regressions and live Trapping Rain Water CDN test passed; three shared Markdown tests passed. Focused native statement fixture passed production image loading, offline caching, and existing interaction/layout checks. Inspected `statement-media.png` with the visible diagram. No installed release or live desktop restart performed.
