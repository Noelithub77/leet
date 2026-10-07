# Build caching

Optimize GitHub builds without reducing native coverage or changing release optimization.

Baseline: the first release took about 38 minutes. Linux x64 spent 16m29s on tests, 6m11s checking, 8m49s building, and 3m47s packaging. macOS Intel spent 12m45s checking, 12m47s building, and 6m41s packaging. The repository had no Actions caches.

- Run workspace and installer tests alongside native builds, with publishing gated on both.
- Build the app and Rust packaging example in the release profile together; retain all-target checks in that profile.
- Cache pnpm's store by the landing page lockfile.
- **[CHOSEN]** Use GitHub's first-party cache support within the requested build-caching scope, avoiding a third-party action dependency. Manual build-only runs on main warm caches for future tags; automatic main builds remain an optional pending choice.
- Validate workflows with actionlint and run cold/warm jobs to measure actual reuse.

Local checks passed: 54 Rust tests, 5 installer regressions, 3 site tests and production build, and actionlint. The combined release executable/packaging-tool build passed. Remote cold/warm cache behavior is not yet verified.
