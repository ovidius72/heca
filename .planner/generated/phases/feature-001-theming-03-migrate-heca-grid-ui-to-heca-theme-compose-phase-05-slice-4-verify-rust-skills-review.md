# feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-05-slice-4-verify-rust-skills-review — Slice 4 — verify + rust-skills review

**Status:** 📄 `draft`
**Created:** 2026-06-30T16:52:52.003Z
**Updated:** 2026-06-30T16:52:52.003Z

Slice 4 — verify: showcase theme cycling + workspace tests + clippy + rust-skills review

Verify the whole theming-03 migration end-to-end. Run the showcase (cargo run -p heca-renderer --example showcase) and confirm all widgets react to theme cycling: colors, radius, border, glow, fonts; confirm the light theme (latte) renders with intensity = off and glow_size = none. Run cargo test --workspace (note: 2 pre-existing non-green tests are NOT regressions — heca-grid-ui white-press-flash toast test fails on plain main, and terminal_backend_exit_captures_code_via_take_exit is a flaky PTY-timing test under workspace-parallel load; both pass in isolation). Run cargo clippy --workspace --all-targets --all-features — must be 0 warnings (only the pre-existing block v0.1.6 future-incompat warning is acceptable). Load rust-skills and run a formal review against its rules before marking complete (repo mandate).

## Tasks

_No tasks defined._
