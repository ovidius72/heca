# feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-02-slice-1-grid-ui-primitive-re-export — Slice 1 — grid-ui primitive re-export

**Status:** 📋 `planned`
**Created:** 2026-06-30T16:52:29.394Z
**Updated:** 2026-06-30T16:57:36.266Z

Slice 1 — grid-ui primitive re-export: Color/Intensity/GlowLevel from heca-theme

Add heca-theme dep to heca-grid-ui/Cargo.toml. Make heca-grid-ui/src/color.rs a re-export `pub use heca_theme::Color;` (heca_theme::Color is a superset — adds serde, to_linear_f32x4, Display, From/TryFrom<String>; with_alpha_f32 added in Slice 0). Verify Intensity and GlowLevel are identical (variants + methods scanline_opacity/next/radius_scale/strength_scale/parse/label) between grid-ui and heca-theme before re-exporting; if identical, re-export from heca_theme. Update heca-grid-ui/src/lib.rs + prelude to re-export Color/Intensity/GlowLevel from heca-theme. grid-ui::Theme still duplicates the color FIELDS at this stage (compose comes in Slice 2) — only the primitive TYPES are deduped here.

## Goals
- heca-grid-ui has heca-theme dep
- grid-ui::Color is re-exported from heca_theme (heca-grid-ui/src/color.rs = pub use heca_theme::Color)
- Intensity/GlowLevel re-exported from heca_theme (after verifying identical variants+methods)
- heca-grid-ui/src/lib.rs + prelude re-export Color/Intensity/GlowLevel; Theme stays local
- grid-ui compiles with the primitive types coming from heca_theme

## Dependencies
- Slice 0 (needs with_alpha_f32 on heca_theme::Color)

## Risks
- Intensity/GlowLevel not byte-identical between grid-ui and heca-theme - must reconcile first (port grid-ui-only behavior into heca-theme)
- a grid-ui::Color method missing on heca_theme::Color besides with_alpha_f32 - block (with_alpha_f32 added in Slice 0 covers the known gap; audit said heca_theme::Color is otherwise a superset)

## Completion Criteria
- cargo check -p heca-grid-ui green
- cargo test -p heca-grid-ui --all-targets green
- cargo clippy -p heca-grid-ui --all-targets --all-features 0 warnings
- all 30+ widgets compile (Theme still old duplicate struct; only primitive types swapped)
- Intensity/GlowLevel diff documented (identical) or reconciled
- rust-skills review done

## Tasks

### 📋 slice-1-grid-ui-primitive-re-export-task-001-add-heca-theme-dep-to-heca-gri — Add heca-theme dep to heca-grid-ui/Cargo.toml

Status: 📋 `planned`

Add `heca-theme = { path = "../heca-theme" }` to heca-grid-ui/Cargo.toml under dependencies (next to heca-core). grid-ui currently has NO heca-theme dep (it owns its own Color/Theme). This is task theming-task-11.

### 📋 slice-1-grid-ui-primitive-re-export-task-002-heca-grid-ui-src-color-rs-re-e — heca-grid-ui/src/color.rs → re-export heca_theme::Color

Status: 📋 `planned`

Replace the contents of heca-grid-ui/src/color.rs with `pub use heca_theme::Color;` (remove the local `pub struct Color` + its impl/FromStr/tests). heca_theme::Color is a superset: it adds serde (try_from=String/into=String), to_linear_f32x4, Display, From<Color> for String, TryFrom<String>; and with_alpha_f32 (added in Slice 0). All grid-ui code that imports `crate::color::Color` keeps working via the re-export. This is task theming-task-12.

### 📋 slice-1-grid-ui-primitive-re-export-task-003-verify-re-export-intensity-glo — Verify + re-export Intensity/GlowLevel from heca_theme (diff first)

Status: 📋 `planned`

Before re-exporting Intensity/GlowLevel from heca_theme, verify the grid-ui and heca-theme enums are identical: same variants (Intensity: off/low/medium/heavy; GlowLevel: none/thin/medium/large) AND same methods (scanline_opacity, next, radius_scale, strength_scale, parse, label) with identical bodies. Diff heca-grid-ui/src/theme.rs vs heca-theme/src/theme.rs for these enums. If identical → re-export `pub use heca_theme::{Intensity, GlowLevel};` in grid-ui/src/theme.rs (remove the local enum definitions). If NOT identical → reconcile first (the heca-theme version is canonical; port any grid-ui-only behavior into heca-theme). Document the diff result in the task notes.

### 📋 slice-1-grid-ui-primitive-re-export-task-004-grid-ui-lib-prelude-reexports — Update grid-ui lib.rs + prelude re-exports

Status: 📋 `planned`

Update heca-grid-ui/src/lib.rs + prelude to re-export Color/Intensity/GlowLevel from heca-theme instead of the local color/theme modules. Today lib.rs:60 `pub use theme::{GlowLevel, Intensity, Theme};` and prelude `pub use crate::theme::{GlowLevel, Intensity, Theme};`. After Slice 1, Color/Intensity/GlowLevel come from heca_theme (via the re-exports in color.rs/theme.rs); Theme STAYS local (grid-ui::Theme, the composed struct — but compose is Slice 2; at Slice 1 Theme is still the old duplicate struct). So: re-export Color/Intensity/GlowLevel from heca_theme; keep `pub use theme::Theme;` (local). Verify `cargo check -p heca-grid-ui`.

### 📋 slice-1-grid-ui-primitive-re-export-task-005-slice1-gate-check-test-clippy — Gate: cargo check/test/clippy -p heca-grid-ui + rust-skills review

Status: 📋 `planned`

Gate for Slice 1: `cargo check -p heca-grid-ui` green + `cargo test -p heca-grid-ui --all-targets` green + `cargo clippy -p heca-grid-ui --all-targets --all-features` 0 warnings. All 30+ widgets must still compile (they call cx.theme().<field> — Theme is still the old duplicate struct here, so callsites are unchanged; only the Color/Intensity/GlowLevel TYPES are now from heca_theme). rust-skills review before complete.
