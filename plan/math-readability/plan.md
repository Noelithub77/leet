# Readable statement variables and formulas

Status: implemented. Stable description sizing, inline math sizing/style, engine-based baseline alignment, ink-preserving cropping, byte-bounded caching, and native regression fixtures are in place. The user approved direct dependencies on the already-locked `ratex-layout` and `ratex-parser` 0.1.14 crates.

## Goal

Match the reference's legibility: clear italic math variables, readable subscripts and superscripts, natural spacing, and alignment with surrounding prose. Preserve the native statement cards, selected theme, user font preference, selection/copy, and app zoom.

## Diagnosis before implementation

- The supplied screenshot shows small, thin formulas beside larger-looking prose. It is the visual bug report, not a fresh native reproduction.
- `crates/gui/src/statement.rs:299` reduces description font size from 13 to 12 logical px as content grows. At line 185 that size is applied to the description, including its inline formulas.
- `crates/gui/src/rich_text.rs:64` passes the surrounding font size directly to the formula renderer. Matching em sizes does not ensure matching visible letter heights across the user's prose font and the embedded math font.
- `crates/gui/src/rich_text.rs:67` estimates the baseline as cropped image height minus 18% of the prose font size. That cannot distinguish a lone variable from a formula with a deep subscript or fraction.
- Formula padding is cropped using the renderer's `PAD` constant. This is not proof of clipping; compare the raster's ink bounds before changing it.
- The installed `ratex-gpui` 0.4.1 source rasterizes formulas at the requested device pixel ratio, embeds math font outlines, and recolors them to the host text color. The app already passes inherited inline text color. A separate muted-color defect has not been established.
- The renderer returns width and height, but no baseline. Recovering layout metrics may require a narrow integration change; inspect existing dependency APIs before selecting that route. Do not edit Cargo registry sources.
- Current GUI math tests use 18 px and check render success, dimensions, cache reuse, and scaling. The statement fixture uses mostly backtick code and synthetic provider prose, so it does not reproduce this small-formula case.
- Native reproduction in `/tmp/leet-math-before-2` confirmed small math beside prose and an incorrect fraction baseline. Final captures in `/tmp/leet-math-baseline` show stable 14 px prose, 1.15x inline math, and corrected alignment in dark/light themes, compact width, monospace prose, and 140% zoom. These are offscreen 2x captures, not real-time performance measurements.

## Implementation evidence

- Focused `rich_text::tests` passed (3 tests), including small-size 1x/2x rasterization, exact foreground colors, cache reuse, byte-budget eviction, baseline stability when adding subscripts, and containment of every nontransparent raster pixel for variables, indices, roots, and fractions.
- `./ops gui:test --fixture statement --output /tmp/leet-math-baseline --json` passed with native pixels, pointer selection/keyboard copy of `Copy a_{j-1} and 10^9.`, scrolling, and existing provider/card checks. The local HTTP image fixture required sandbox escalation; it used fixture data only.
- `./ops gui:test --fixture chat --output /tmp/leet-math-baseline-chat --json` passed with indexed formulas, inequalities, an exponent, and a fraction in deterministic replies, plus existing composer/navigation/selection checks. Its rendered formula-heavy reply was visually inspected.
- Inline formulas now use `\textstyle`; display formula sizing is unchanged. Cache retention is bounded to 256 entries and 32 MiB of raster bytes. Layout dimensions and estimated raster bytes are checked before rasterization, preserving source fallback for oversized formulas.
- The engine's ascent supplies the image baseline, adjusted for cropping. Cropping retains the nominal TeX box, all raster ink, and one physical pixel of guard space. Extra metrics parsing/layout and ink scanning happen only on cache misses; their latency has not been benchmarked.
- Dependency versions are pinned to 0.1.14 to match the renderer's current engine. Cargo.lock only gains the direct GUI dependency references; no package versions or checksums change.
- No release deployment or active-desktop launch has occurred. Native interaction/pixel evidence is 2x; 1x is covered by renderer tests. Existing atomic inline math wrapping behavior is retained.

## Approved implementation sequence

1. **Reproduce before fixing.** Extend `crates/gui/src/gui_test/statement.rs` with a deterministic Codeforces-shaped HTML statement containing the reported reversing-subarray paragraph. Feed it through provider parsing, description conversion, and the production Statement view. Include `i`, `j`, `n`, `k`, `a_i`, `a_{j-1}`, `j-i+1`, `1 <= i <= j <= n`, and `10^9`. Capture the failing case at the screenshot's approximate pane width, then retain this fixture for regression checks. Keep accounts, personal state, and external requests out of it.
2. **Stabilize prose size.** Replace description-length-dependent shrinking with a consistent rem-based reading size. Start evaluation at 14 logical px at 100% zoom, with about 1.5 line height. Keep the configured UI font; use Liberation Sans in the default-font fixture and include a monospace-user-font case resembling the screenshot. Longer content should scroll, rather than become smaller. Final values come from the native comparison.
3. **Calibrate math to visible prose height.** Retain the existing serif italic math glyphs. Compare inline math at 1.0, 1.1, and 1.15 times the prose em size in the fixture, then choose one restrained default that keeps subscripts legible. Render at the final logical size and actual device scale; do not enlarge an already rasterized bitmap. Avoid synthetic bolding, outlines, or replacing formulas with plain Unicode text.
4. **Align by formula metrics.** Store ascent/descent or baseline alongside each cached image when the existing dependency exposes usable layout metrics. Align the cropped math baseline with the prose alphabetic baseline and allow sufficient line height for tall expressions. If a new direct dependency or package patch is needed, present that concrete choice before adding it. Keep display formulas on a separate sizing/layout path. Test that cropping preserves italic overhangs, roots, indices, and fraction bars.
5. **Verify contrast and sharpness.** Use the same foreground as the surrounding sentence in dark and light themes. Compare native pixels at 1x and 2x device scale before considering supersampling. Add supersampling only if actual captures show undersampling, and evaluate its memory/render cost. Preserve theme/size/device-scale cache invalidation and readable TeX fallback for unsupported syntax.
6. **Protect shared behavior.** The plugin also serves hints, references, and chat. Keep statement prose changes local to Statement; verify shared math changes in a focused chat case. Preserve underlying TeX selection/copy, code literals, table formulas, streamed incomplete math, and existing display-formula scrolling. Do not change provider data or refresh the public bundle.

## Acceptance and verification

- At normal zoom, variables have a comparable visible height and foreground strength to nearby prose. Indices, inequality signs, and exponents are readable without zooming in.
- Single letters, indexed expressions, fractions, and roots sit on the intended baseline without crowding, clipping, or overlapping adjacent lines.
- Check the reported-width pane and a compact 420 px pane, dark/light themes, 100%/140% app zoom, and 1x/2x device scale. Keep representative captures bounded, rather than exhaustively rendering every combination.
- Use GPUI pointer/keyboard dispatch to select and copy a sentence spanning prose and math. Assert the copied source separately from visual inspection. Check narrow-pane wrapping, scroll reachability, and a long atomic inline expression's overflow behavior.
- Add small-size render regressions and metric/crop assertions for material behavior. Replace the test that requires long statements to shrink with one asserting stable, zoom-aware sizing.
- Run the focused math tests and `./ops gui:test --fixture statement --json`; run the focused chat fixture if the shared plugin changes. Broaden checks only for failures or materially wider changes. Run one heavy job at a time, with idle I/O, low CPU priority, two allowed CPUs, and two workers.
- Review cold rendering and cached reuse for a formula-heavy statement. Bound retained raster bytes as well as entry count. Do not claim a speedup or real-time responsiveness from offscreen capture timing.
- Update `docs/changelog.md` with a short Fix bullet after implementation. No release deployment is part of this plan.

## Recommended scope

Start with stable prose size, calibrated inline math, and correct baseline/crop metrics in the existing renderer. Replacing the rendering engine would add dependency and selection/layout risk before proving it necessary. The proposed browser preview is illustrative HTML, not evidence of final GPUI pixels.
