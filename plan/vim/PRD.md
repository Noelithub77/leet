# Vim mode requirements

Status: Frozen. Approved by the user on 2026-10-13 through the explicit instruction to implement the supplied plan.

## Context and goals
Provide native modal editing in solution and editable snippet source editors, default off, without changing conventional non-code inputs. Success requires command results and production input routing, not only engine tests.

## Functional requirements
- FR-001: Persist Settings > Editor > Vim keybindings; toggle existing, parked, reopened and new editors immediately. Each newly enabled editor starts Normal.
- FR-002: Normal, Insert, Visual character, Visual line and Visual Block. Normal/Visual Ctrl+v enters Block; Block Ctrl+v returns Normal; Insert Ctrl+v uses native clipboard paste and remains Insert.
- FR-003: All motions, counts, operators, text objects, registers, put, undo/redo, repeat, literal search and scrolling in the user plan.
- FR-004: Display-column rectangles, tabs, Unicode wide characters, short and empty rows; block I/A/c replication, replacement, put and logical undo.
- FR-005: Vim precedence over conflicting shortcuts; IME guard outside Insert; native assistance inside Insert; one Escape ends suggestions/snippets and enters Normal; no independent extra cursors.
- FR-006: Shared unnamed register, repeat and search across workspace editors; per-editor modes/selections; finish transactions and clear pending input on blur.
- FR-007: Read-only views permit navigation, selection, yank and search only. :w/:q/:wq/:x use existing host flows, close after successful save; errors and unsupported commands are visible.
- FR-008: Mode/pending indicator, block cursor, rectangular highlights, palette toggle, supported command help and changelog.

## Scope and decisions
All commands listed in the supplied plan are in scope. Insert Ctrl+v intentionally differs from Vim literal insertion. Search is literal and wraps. Standard Vim block geometry with default virtualedit applies. Retain existing safe Rust and dependencies. No personal data or account access for tests.

## Deferred
Continuous Replace, named/numbered registers, macros, marks, jumps, substitutions, regex, mappings, vimrc, plugins, window commands. Named registers are the recommended next extension.

## Decision log
2026-10-13: User supplied an implementation-ready plan and explicitly authorized implementation and verification. No product decisions remain open.
