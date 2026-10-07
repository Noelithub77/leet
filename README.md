# leet

A minimal native coding-practice IDE, built with Rust and GPUI Kit. NeetCode roadmaps, universal LeetCode/Codeforces search, a language-server editor, local tests, solution articles, AI prompt shortcuts, and staged Git/submission history share one keyboard-first workspace.

The default is NeetCode 150, Python, Liberation Sans, and Vesper. The embedded public SQLite is about 11.2 MB: full NeetCode 150 content and metadata-only catalogs elsewhere. Statements opened outside that list fetch on demand and remain cached. See [bundle coverage and sources](docs/bundle.md).

Get leet from [the download page](https://noelithub77.github.io/leet/). Linux and macOS install and launch with one command:

```sh
curl -fsSL https://noelithub77.github.io/leet/install.sh | sh
```

Windows: [download the portable x64 executable](https://github.com/Noelithub77/leet/releases/latest/download/leet-windows-x86_64.exe) and open it. The first releases are unsigned and may show OS security prompts. See [platform requirements and releases](docs/releases.md), including separate Python/compiler requirements for running local solutions.

For source development:

```sh
./ops --help
./ops check --json
./ops local:deploy --json
leet
```

Rust and the Linux system libraries required by GPUI are build prerequisites; language servers are discovered from PATH or compatible Zed installations. See [development and shortcuts](docs/development.md) and [changelog](docs/changelog.md).

Existing vg settings, credentials, cached data, and Solutions remain available after the rename. The obsolete `vg` launcher is removed. New installations use leet paths. Every release installation keeps the current executable intact and offers running windows a restart into the next build.

Try `leet --1337`; there is another small Easter egg hidden in Home.

The desktop application and deployment operator are Rust. No Go compiler, verd executable, or verd deployment wrapper is required. Python is needed to run Python solutions; other languages use their installed compilers and language servers.

Launch with `leet` or `1337`; both commands point to the same installed build. The Rust installer refreshes all aliases at each deployment.
