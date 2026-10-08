# Contributing to leet

Bug reports, focused fixes, documentation, and improvements to practice workflows are welcome. Follow the [code of conduct](CODE_OF_CONDUCT.md). Report vulnerabilities privately through [the security policy](SECURITY.md).

## Before changing code

Search existing issues and pull requests. For substantial features, new dependencies, architecture changes, or changes to bundled data, open an issue describing the problem and proposed approach before implementing it. Small fixes can go straight to a pull request.

Read [AGENTS.md](AGENTS.md), [development](docs/development.md), and the guidance in the directory you change. Keep unrelated work out of your pull request.

## Development

The app and operator are Rust. `crates/gui` owns the GPUI desktop interface; `crates/practice` owns clients, runners, tracing, storage, and domain behavior. Both crates forbid unsafe code. `site/` is the Vite landing page.

```sh
./ops --help
./ops check --json
```

Use [development setup](docs/development.md) for GPUI system dependencies and [release requirements](docs/releases.md) for supported platforms. Do not include credentials, private account state, personal submissions, solutions, or tests in the public SQLite bundle.

For website changes:

```sh
pnpm --dir site install --frozen-lockfile
pnpm --dir site ops --help
pnpm --dir site ops check --json
pnpm --dir site dev
```

The development URL is `http://localhost:1337/leet/`. Port 1337 is strict: stop the conflicting process or explicitly choose a different port with `--port`.

## Verify the change

Add regression coverage for meaningful behavior. Run checks relevant to the files you changed; native changes use `./ops check --json`, and site changes use `pnpm --dir site ops check --json`.

For GUI changes, follow [isolated GUI testing](docs/gui-testing.md). Use fixtures and temporary settings/data. Report input behavior, rendered pixels, and compositor checks separately. Never capture personal accounts or solution repositories for public screenshots. Check website changes at desktop and phone widths, keyboard navigation, and reduced motion.

`./ops local:deploy --json` installs a development build on your own machine. It preserves existing app data, but is not needed for documentation-only or website-only changes.

## Pull requests

Explain the user-visible problem, what changes, and how you verified it. Attach screenshots or a short recording for visual changes. Note unverified platforms or behavior. Keep code straightforward, preserve existing conventions, and do not add formatting tools or weaken checks.

Update `docs/changelog.md` for user-visible milestones using its existing Feat/Fix format. Keep release and installation details in `docs/releases.md`.

By contributing, you agree that your original contributions are distributed under the repository’s [GNU AGPL v3 license](LICENSE). Preserve the licenses and attribution of third-party material. No contributor license agreement is required.
