# feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-05-slice-4-verify-rust-skills-review — Slice 4 — verify + rust-skills review

**Status:** 📋 `planned`
**Created:** 2026-06-30T16:52:52.003Z
**Updated:** 2026-06-30T16:58:03.742Z

Slice 4 — verify: showcase theme cycling + workspace tests + clippy + rust-skills review

Verify the whole theming-03 migration end-to-end. Run the showcase (cargo run -p heca-renderer --example showcase) and confirm all widgets react to theme cycling: colors, radius, border, glow, fonts; confirm the light theme (latte) renders with intensity = off and glow_size = none. Run cargo test --workspace (note: 2 pre-existing non-green tests are NOT regressions — heca-grid-ui white-press-flash toast test fails on plain main, and terminal_backend_exit_captures_code_via_take_exit is a flaky PTY-timing test under workspace-parallel load; both pass in isolation). Run cargo clippy --workspace --all-targets --all-features — must be 0 warnings (only the pre-existing block v0.1.6 future-incompat warning is acceptable). Load rust-skills and run a formal review against its rules before marking complete (repo mandate).

## Goals
- showcase theme cycling verified (colors/radius/border/glow/fonts + latte with intensity off / glow_size none)
- workspace tests green except 2 pre-existing non-regressions
- workspace clippy 0 warnings
- rust-skills formal review done, findings fixed

## Dependencies
- Slice 0
- Slice 1
- Slice 2
- Slice 3

## Risks
- showcase needs manual visual confirmation (cannot be automated headless) - flag for the user to eyeball
- the 2 pre-existing test failures could be mistaken for regressions - document clearly + run in isolation to prove
- a new clippy warning from theming-03 could block the gate - fix all

## Completion Criteria
- cargo run -p heca-renderer --example showcase - all widgets react to theme cycling; latte ok
- cargo test --workspace --all-targets - only the 2 known pre-existing failures (white-press-flash toast; flaky PTY exit-captures)
- cargo clippy --workspace --all-targets --all-features 0 warnings (block v0.1.6 future-incompat acceptable)
- rust-skills review checklist complete; clippy --fix applied; all findings fixed

## Tasks

### 📋 slice-4-verify-rust-skills-review-task-001-verify-showcase-theme-cycling — Run showcase, verify theme cycling (colors/radius/border/glow/fonts) + latte

Status: 📋 `planned`

Run the showcase end-to-end: `cargo run -p heca-renderer --example showcase`. Verify all widgets react to theme cycling (the showcase cycles grid_tron/mocha/latte): colors, radius, border, glow, fonts all update live. Confirm the light theme (latte) renders correctly with intensity = "off" and glow_size = "none" (no visual regressions, no white-press-flash off-theme, modal drop-shadow reads the token). This is the visual verification gate (theming-task-01/02 from BACKLOG).

### 📋 slice-4-verify-rust-skills-review-task-002-verify-workspace-tests — cargo test --workspace (account for 2 pre-existing non-regression failures)

Status: 📋 `planned`

Run `cargo test --workspace --all-targets`. Expected green EXCEPT 2 pre-existing non-regression failures (do NOT treat as regressions, do NOT fix in this slice): (1) heca-grid-ui white-press-flash toast test fails on plain main (pre-existing), (2) terminal_backend_exit_captures_code_via_take_exit is a flaky PTY-timing test under workspace-parallel load (passes in isolation). Both pass in isolation (`cargo test -p <crate>`). Document the result in the task notes. If any OTHER test fails → that's a regression from theming-03 → fix before complete.

### 📋 slice-4-verify-rust-skills-review-task-003-verify-workspace-clippy — cargo clippy --workspace --all-targets --all-features (0 warnings; block v0.1.6 ok)

Status: 📋 `planned`

Run `cargo clippy --workspace --all-targets --all-features` — must be 0 warnings (only the pre-existing `block v0.1.6` future-incompat warning is acceptable; document it). This is the repo-wide clippy gate (repo rule: the codebase must stay clippy-clean). Fix ALL new warnings introduced by theming-03.

### 📋 slice-4-verify-rust-skills-review-task-004-rust-skills-formal-review — Load rust-skills, formal review against its rules, fix findings (FINAL gate)

Status: 📋 `planned`

Repo mandate: load /Users/antonio/.agents/skills/rust/SKILL.md and run a formal review against its rules before marking theming-03 complete. Check: ownership/borrowing (the compose embed uses heca_theme::Theme by value — clone in the adapter is fine; widgets borrow `&self.colors`), no unwrap()/expect in widget paths, error handling, no `#[allow(dead_code)]` without a commented reason (the dropped conversion helpers must be fully removed), idiomatic patterns. Run `cargo clippy --fix --workspace --all-targets --all-features` for auto-fixable, then re-run clippy. Fix all findings before complete. This is the FINAL review gate for the whole theming-03 feature.
