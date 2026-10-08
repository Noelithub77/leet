# Native stdin tracing

[CHOSEN] Extend the existing Python and C++ recorders and Debug player for Codeforces and CodeChef, including CPH samples. Keep LeetCode APIs compatible. No new dependencies.

- Record whole input samples without parsing internal contest test cases.
- Python traces module code and helpers with real text/binary stdin.
- C++ traces the user's main with debug symbols; stdout is the result.
- Preserve trace budgets, partial states, errors, and sample whitespace comparison.
- Verified all eight stdin regressions, existing recorder coverage, full workspace tests (38 GUI, 147 practice passed, 14 ignored), and five installer regressions.
- Existing Python/C++ templates already match the stdin entry points; retained them.
- CLI smoke verified completed Python and C++ traces with the expected multi-test stdout.
- Delivery uses a scoped commit followed by `./ops local:deploy --json`. Native rendered playback remains unverified.
