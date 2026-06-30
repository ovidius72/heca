# heca — theming-03 (grid-ui → heca-theme compose) — Project Plan

> *Goal not defined yet.*

**Last updated:** 2026-06-30T16:55:11.592Z
**Version:** 1
**Project ID:** `34823f1f-59ec-4388-9b13-7aa477160a5d`

---

## Workflow Rules

---
## Features

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose — theming-03 — Migrate heca-grid-ui to heca-theme (compose)

Migrate heca-grid-ui to compose heca-theme (NOT re-export). Locked design (grill-me 2026-06-30): grid-ui::Theme = { colors: heca_theme::Theme, font_family, font_size, focus_border_width }. Re-export Color/Intensity/GlowLevel. radius → colors.border_radius; shadow → colors.shadow config-driven token (Shadow.color: String→Color, blur = theme.colors.shadow.blur * SHADOW_BLUR_MULT replacing hardcoded SHADOW_BLUR). App adapter (app_theme_to_gui_theme/chrome_gui_theme) STAYS, simplified to embed theme + GUI extras. Contains/generalizes the compositor-04c adapter, does not remove it. Branch: feat/theming-03-grid-ui-port (rebased on origin/main). Plan source: theming-plan.md 3B + BACKLOG.md theming-03.

Status: 📋 `planned`

**Phases:**
- 📋 **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-01-slice-0-heca-theme-prereq** Slice 0 — heca-theme prereq (0/5 tasks)
- 📋 **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-02-slice-1-grid-ui-primitive-re-export** Slice 1 — grid-ui primitive re-export (0/5 tasks)
- 📋 **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-03-slice-2-grid-ui-theme-compose-callsite-churn** Slice 2 — grid-ui Theme compose + callsite churn (0/7 tasks)
- ❓ **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-04-slice-3-app-adapter-simplification** Slice 3 — app adapter simplification (0/0 tasks)
- ❓ **feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-05-slice-4-verify-rust-skills-review** Slice 4 — verify + rust-skills review (0/0 tasks)

---
## Requirements

_No requirements defined yet._

---
## Phases

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-01-slice-0-heca-theme-prereq — Slice 0 — heca-theme prereq

Slice 0 — heca-theme prereq: Shadow.color: String→Color + Color::with_alpha_f32

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-02-slice-1-grid-ui-primitive-re-export — Slice 1 — grid-ui primitive re-export

Slice 1 — grid-ui primitive re-export: Color/Intensity/GlowLevel from heca-theme

Status: 📋 `planned`

**Tasks:** 0/5

### 📋 feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-03-slice-2-grid-ui-theme-compose-callsite-churn — Slice 2 — grid-ui Theme compose + callsite churn

Slice 2 — grid-ui Theme compose + ~47 callsite churn + modal shadow rework

Status: 📋 `planned`

**Tasks:** 0/7

### ❓ feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-04-slice-3-app-adapter-simplification — Slice 3 — app adapter simplification

Slice 3 — app adapter simplification (chrome_gui_theme embeds colors + GUI extras)

Status: 📄 `draft`

### ❓ feature-001-theming-03-migrate-heca-grid-ui-to-heca-theme-compose-phase-05-slice-4-verify-rust-skills-review — Slice 4 — verify + rust-skills review

Slice 4 — verify: showcase theme cycling + workspace tests + clippy + rust-skills review

Status: 📄 `draft`
