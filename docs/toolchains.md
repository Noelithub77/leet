# Private toolchains

Leet queues tools automatically on startup, with the preferred language first.
Setup runs one language at a time and survives leaving onboarding or switching
views. Continue stays available. The status bar shows the active stage, transfer
bytes/percentage when available, elapsed time, queued jobs, and a Retry action
when setup fails. Copilot has its own connection/download indicator in the same
bar; GitHub sign-in remains optional and browser approval is a user action.

Tools live under the app's private data directory (`~/.local/share/leet/tools`
on a new Linux install). Setup preserves configuration, credentials, solutions,
existing system tools, and system PATH. Existing legacy data paths remain valid.
Downloads use pinned HTTPS URLs and SHA-256 hashes. Archive extraction rejects
paths outside staging, verifies size limits, and preserves the previous tool
until a verified replacement is extracted. Completed assets are reused on retry.

| Language | Runtime/compiler | Editor | Debugger |
| --- | --- | --- | --- |
| Python | Portable CPython | basedpyright and private Node | debugpy for stdin; structured Python tracer for function cases |
| C and C++ | Zig's bundled Clang, C/C++ libraries, and linker | clangd | CodeLLDB and bundled LLDB |
| Go | Official Go distribution | gopls | Delve |
| Java | Eclipse Temurin JDK | Eclipse JDT LS | Microsoft Java debug plugin |

The managed JDK is Java 25, matching the pinned JDT LS runtime requirement.
Java solutions compile and run with that private JDK.

The pinned catalog in `crates/practice/setup/toolchains.json` covers Linux x64
and arm64, macOS Intel and Apple silicon, and Windows x64. These match the native
release targets. There is no native Windows ARM tool catalog. Linux arm64 clangd comes from its
pinned Python wheel. Other clangd builds and the portable Python bootstrap retain
the existing `tools.json` catalog.

The application and operator orchestrate setup in Rust. The portable Python
bootstrap handles verified archive extraction and existing language-tool install
helpers. Go builds gopls at a pinned module version with module checksum
verification and private module/build caches; compiler parallelism is limited to
two. Windows subprocesses run without console windows. PowerShell streams the
Python installer's output to the application.

Managed executables are selected by the app directly. Explicit custom Python
paths remain authoritative. C++ gets a private compatibility `bits/stdc++.h`
header containing standard C++ headers, so ordinary contest templates work with
Zig's libc++ without a separate GCC installation. GNU-specific libraries and
compiler extensions are not bundled. Setup prepares C/C++ compiler caches and
passes the compiler's private header paths to clangd's fallback configuration;
project compile commands retain priority. Compiler caches stay in the tool directory.
Managed child processes receive private Go/JDK paths without changing the user's
system environment or shell configuration.

Debug adapters record real source stops, call stacks, and bounded variables into
the existing seekable trace. Recording has a deadline, step/item/output limits,
and process-tree cleanup. C function harnesses support scalar and one-dimensional
array cases; Go and Java also support typed nested arrays/lists. Unsupported
metadata types and design classes return a specific
error. Python and C++ retain their broader judge harnesses. Adapter values may
remain typed labels when a richer structure cannot be reconstructed. The legacy
system-GDB C++ recorder remains available while managed tools are missing.

Use the tracked operator for an isolated install or a retry:

```sh
./ops toolchain:setup --language python --directory /absolute/private/tools --json
./ops toolchain:setup --language cpp --directory /absolute/private/tools --json
./ops toolchain:setup --language go --directory /absolute/private/tools --json
./ops toolchain:setup --language java --directory /absolute/private/tools --json
./ops toolchain:setup --language c --dry-run --json
```

`--dry-run` only reports the target and catalog. An isolated application test must
also point its data-directory environment at the corresponding parent so runtime
resolution uses that same directory. Linux uses `XDG_DATA_HOME`; normal installs
use platform data-directory conventions.

Offline integrity regressions: `python3 tests/toolchain_install.py`. The ignored
`debugger::adapter_tests::managed_adapters_record_real_stdin_and_function_cases`
Rust test exercises all installed adapters without reading personal solutions.
The ignored `lsp::tests::live_private_all_language_servers` test exercises real
completion, hover, definition, and diagnostics against all five managed servers.
Native macOS/Windows execution requires those systems; catalog coverage and a
Linux run do not establish native behavior on another platform.

Primary references: [Zig's C/C++ toolchain](https://ziglang.org/learn/overview/),
[CodeLLDB configuration](https://github.com/vadimcn/codelldb/blob/master/MANUAL.md),
[Delve's debug-adapter interface](https://github.com/go-delve/delve/blob/master/Documentation/api/dap/README.md),
and [Java debug plugin startup](https://github.com/microsoft/java-debug#usage-with-eclipsejdtls).
