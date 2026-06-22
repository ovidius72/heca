# Handoff — `intensity` / `glow_size` `[appearance]` override + full token doc audit

> Self-contained restart doc. Any agent picking this up should have full context
> from this file alone. **Do not commit until the user reviews.** See §7 workflow.

Last updated: 2026-06-22

---

## 1. What this task is

Add a `[appearance]` config override for the two existing **effect** theme tokens,
`intensity` and `glow_size`, and clean up the fact that `intensity` currently
leaks into glow rendering. Then do a **full documentation audit** of every theme
and `[appearance]` token across the user-facing docs.

This is **not** a new theme variable from scratch — both tokens already exist on
`Theme`. The new work is: (a) the `[appearance]` override, (b) cleaning the
glow/scanline separation, (c) documenting everything properly.

### 1.1 The two tokens (corrected semantics — IMPORTANT)

The code currently has **misleading docs** that say `intensity` drives "glow +
scanlines". That is wrong. The intended and correct separation (per
`heca-grid-ui/src/component.rs:861` *"the **sole** owner of glow"*):

| Token | Type | Drives | TOML values |
|-------|------|--------|-------------|
| `glow_size` | `GlowLevel` | **Glow**: presence + halo radius (+ strength, after this task) | `none` / `thin` / `medium` / `large` |
| `intensity` | `Intensity` | **Scanlines / CRT overlay** only (opacity) | `off` / `low` / `medium` / `heavy` |

They are **separate**. `intensity` does **not** give more glow. The user
confirmed this explicitly and wants the separation made real.

---

## 2. Decisions (all confirmed with the user)

1. **Keep the name `intensity`** — do NOT rename. Just document it better
   (scanlines/CRT overlay, not glow).
2. **Clean the drift** — remove `Intensity::glow_scale()`; fix the one widget that
   uses it (`marker_group.rs`) to take glow strength from `glow_size` instead.
   Add `GlowLevel::strength_scale()` so `glow_size` owns glow
   **presence + radius + strength**.
3. **Defaults stay `Medium`** — keep `#[default] Medium` on both `Intensity` and
   `GlowLevel` (the user reversed an earlier "default to off" idea). The 3
   bundled TOMLs already set both explicitly, so the serde default only matters
   for custom/user themes that omit the field.
4. **`[appearance]` override is 4-level string enum** (not numeric; the user's
   original `0|1` was dropped in favor of the existing enum). `Option<…>`:
   unset → fall back to the theme value; set → wins over the theme.
5. **Branch:** fresh off `main` (independent of the not-yet-merged Phase 4 docs
   branch — see §6).
6. **Full token audit in docs** — document every `Theme` field + every
   `[appearance]` field, not just these two.
7. **`GlowLevel::strength_scale()` curve — DECIDED:** `none=0.0, thin=0.5, medium=1.0, large=1.6`. Radius and strength are independent dimensions (radius = halo size, strength = alpha), so `Large` = 2.0× radius but 1.6× strength is intentional and risk-free. The 1.6 curve preserves the old `intensity.glow_scale()` exactly, so MarkerGroup's glow strength is unchanged after the move. `GlowLevel::radius_scale()` stays as-is (`0.0/0.5/1.0/2.0`).

---

## 3. Code changes

### 3.1 `[appearance]` override fields + resolvers
**File:** `heca-config/src/appearance.rs`

- Imports already present: `use crate::theme::Theme;` (line 2).
  `heca-config/src/theme.rs:7` re-exports `pub use heca_theme::{Color, GlowLevel, Intensity, Shadow, Theme};` — so use `crate::theme::{Intensity, GlowLevel}`.
- Add two fields to `AppearanceConfig` (with `#[serde(default)]`, type `Option<…>`):
  - `pub intensity: Option<Intensity>`
  - `pub glow_size: Option<GlowLevel>`
- Add resolvers (mirror the existing `effective_background_gradient_*` pattern in this file):
  - `pub fn effective_intensity(&self, theme: &Theme) -> Intensity` → `self.intensity.unwrap_or(theme.intensity)`
  - `pub fn effective_glow_size(&self, theme: &Theme) -> GlowLevel` → `self.glow_size.unwrap_or(theme.glow_size)`
- Add unit tests: unset → theme value; set → override wins.

### 3.2 Wire the override into the app
**File:** `heca/src/chrome/mod.rs`

- The **single choke point** is `chrome_gui_theme(state)` (line ~1686), which
  calls `app_theme_to_gui_theme(&state.theme)` (line ~1586). That function maps
  `theme.glow_size` → `glow_level_to_gui(theme.glow_size)` (line 1611) and
  `theme.intensity` → `intensity_to_gui(theme.intensity)` (line 1612).
- `terminal_pane_gui_theme` (line 339) inherits via `chrome_gui_theme`, so no
  extra wiring needed there.
- Verified: `theme.intensity` / `theme.glow_size` are read in the **app** only at
  `chrome/mod.rs:1611-1612` (plus tests). The heca-renderer glow path uses a
  per-glow-command `intensity: f32` (`scene.rs:199`), NOT the theme enum — so
  overriding in `chrome_gui_theme` is sufficient.
- Change: in `chrome_gui_theme`, after `app_theme_to_gui_theme`, override
  `theme.glow_size = glow_level_to_gui(state.appearance.effective_glow_size(&state.theme));`
  and `theme.intensity = intensity_to_gui(state.appearance.effective_intensity(&state.theme));`

### 3.3 Clean the glow/scanline separation
**Files:** `heca-theme/src/theme.rs`, `heca-grid-ui/src/theme.rs`, `heca-grid-ui/src/widgets/marker_group.rs`, `heca-grid-ui/src/component.rs`

- Remove `Intensity::glow_scale()` from **both** `heca-theme::Intensity` (line 29)
  and `heca-grid-ui::theme::Intensity` (line 24).
- Remove the `intensity_glow_scales` test in `heca-theme/src/theme.rs` (line ~465)
  and any equivalent in `heca-grid-ui`.
- Add `GlowLevel::strength_scale() -> f32` to **both** crates' `GlowLevel`:
  `None=0.0, Thin=0.5, Medium=1.0, Large=1.6` (pending user confirm — §2.7).
- Fix `heca-grid-ui/src/widgets/marker_group.rs:156`: replace
  `t.intensity.glow_scale()` with `t.glow_size.strength_scale()`. Update the
  comment block there (lines ~152-155) that says "its strength via `intensity`"
  to say strength comes from `glow_size`.
- `heca-grid-ui/src/component.rs:861` `scaled_glow` already uses
  `glow_size.radius_scale()` for radius and is documented as the sole owner —
  leave its logic, but the "strength" side is now also `glow_size` via the new
  method (MarkerGroup is the only place strength was used).

### 3.4 Fix the misleading `Intensity` doc comments
**Files:** `heca-theme/src/theme.rs` (line ~14), `heca-grid-ui/src/theme.rs` (line ~10)

- Current wrong text: *"How strongly Tron effects (glow, scanlines) are applied"*
  / *"Plain — no glow or scanlines"* / *"Full Tron: strong glow + visible scanlines"*.
- New text: `Intensity` controls **scanline/CRT overlay opacity only**. It does
  **not** affect glow — glow is `glow_size` (presence + radius + strength).
  Keep the variant doc comments accurate (`Off` = no scanlines; `Heavy` = strong
  CRT grille).

### 3.5 `example.config.toml`
**File:** `example.config.toml` (the canonical config example — where users see
what `[appearance]` accepts)

- Under `[appearance]`, add commented knobs:
  - `# intensity = "medium"` with the value list `off/low/medium/heavy` + note
    "scanline/CRT overlay opacity; unset → theme; does not affect glow".
  - `# glow_size = "medium"` with `none/thin/medium/large` + note
    "glow presence + radius + strength; unset → theme".
- These are commented (Optional) — default behavior (unset → theme) is unchanged.

### 3.6 Verification gates (code)
- `cargo build --workspace --all-targets` green.
- `cargo clippy --workspace --all-targets --all-features` → 0 warnings.
- `cargo test --workspace` green except the known pre-existing env-dependent
  `heca-core::terminal_backend_bash_integration_reports_success_error_and_cwd`
  (fails on plain `main`; excluded baseline).
- `rg "\.glow_scale\(\)" --glob '*.rs'` → no remaining callers.
- `rg "intensity\.glow_scale|Intensity::.*glow_scale"` → clean.

---

## 4. Documentation changes (full token audit — user point 4/5)

### 4.1 `README.md`
- Create a proper dedicated **"Theming"** section that clearly describes **all
  available tokens**: every `Theme` field and every `[appearance]` field, with
  types, defaults, TOML value formats, and semantics. Include `intensity`
  (scanlines) and `glow_size` (glow) as `[appearance]`-overridable, with the
  corrected semantics. Reference `example.config.toml` and
  `theming-documentation.md`.
- There is currently a `### Theme` section (~line 441) and an
  `### Appearance & Frost (z=0 background layer)` section (~line 473, added by
  the Phase 4 docs branch). The new Theming section should consolidate/coexist
  with these — be careful about overlap with the not-yet-merged Phase 4 branch
  (see §6).

### 4.2 `terminal-implementation.md` — full audit of terminal tokens used
- Audit which terminal theme tokens are actually consumed by the backend +
  renderer. Known consumers to verify during implementation:
  `TerminalSnapshot.default_fg`, `default_bg`, `cursor_color`, palette/ANSI
  fields, `terminal_background`, `terminal_foreground`, `terminal_font_family` /
  `terminal_font_size` / `terminal_italic_font_family`, cursor/selection colors.
- Update `terminal-implementation.md` to list **all** the terminal tokens
  actually used (currently it has stale `terminal_blur` references too — lines
  ~1185, 1782, 1803, 1823, 1880, 1899, 1934, 1984 — which are now resolved by
  the z=0 refactor; mark them resolved while here).
- This is a full audit, not just the two new knobs.

### 4.3 `theming-documentation.md`
- Full token documentation: every `Theme` field + every `[appearance]` field.
- Note `intensity`/`glow_size` are `[appearance]`-overridable.
- (Phase 4 already removed the 4 `terminal_frost_color` entries here and fixed
  the stale `#e6e9ef00` → `#e6e9ef`. Build on that.)

### 4.4 `AGENTS.md`
- Reflect the new `[appearance]` `intensity`/`glow_size` overrides + all
  theme/config variables. The `Config System` section is at ~line 307; the
  z=0 frost subsection is at ~line 540 (added by Phase 4).
- Reinforce: `intensity` = scanlines, `glow_size` = glow (sole owner).

### 4.5 `BACKLOG.md`
- Update the **theming** section: add the missing tasks (this
  intensity/glow_size override + token audit) and mark what's already done.
- Find the theming track in BACKLOG and reconcile.

---

## 5. Key file/line references (so the next agent doesn't re-discover)

- `heca-theme/src/theme.rs`
  - `Intensity` enum: line 17 (serde `snake_case`, `#[default] Medium`)
  - `Intensity::glow_scale()`: line 29 (REMOVE)
  - `Intensity::scanline_opacity()`: line 39 (keep)
  - `GlowLevel` enum: line 67 (`#[default] Medium`) — add `strength_scale()` here
  - `Theme.glow_size` field: line ~219; `Theme.intensity` field: line ~222
  - `intensity_glow_scales` test: line ~465 (REMOVE)
- `heca-grid-ui/src/theme.rs`
  - `Intensity` enum: line 12; `glow_scale()`: line 24 (REMOVE); `scanline_opacity()`: line 35
  - `Theme.intensity` field: line 137; defaults: lines 187, 213
  - `GlowLevel` enum: line 58 — add `strength_scale()` here too
- `heca-grid-ui/src/component.rs:861` — `scaled_glow`, "sole owner of glow" comment (leave logic)
- `heca-grid-ui/src/widgets/marker_group.rs:156` — the ONLY caller of `intensity.glow_scale()` (FIX)
- `heca/src/chrome/mod.rs`
  - `glow_level_to_gui`: line 1571; `app_theme_to_gui_theme`: line 1586 (maps at 1611-1612)
  - `chrome_gui_theme`: line 1686 (single choke point — apply override here)
  - `terminal_pane_gui_theme`: line 339 (inherits via chrome_gui_theme)
- `heca-config/src/appearance.rs` — `AppearanceConfig` struct; imports lines 1-3
- `heca-config/src/theme.rs:7` — `pub use heca_theme::{Color, GlowLevel, Intensity, Shadow, Theme};`
- Bundled TOMLs (`heca-theme/src/themes/*.toml`) lines 22-23:
  - `grid_tron.toml`: `glow_size = "medium"`, `intensity = "medium"`
  - `mocha.toml`: `glow_size = "none"`, `intensity = "off"`
  - `latte.toml`: `glow_size = "none"`, `intensity = "off"`
- `heca-renderer/examples/showcase.rs` — INTENSITY_OPTS line 94; glow/intensity
  selects (~490-511); maps heca-theme → grid-ui (~143-153). The showcase already
  cycles these; no required change, but verify it still builds after
  `glow_scale()` removal (it uses `scanline_opacity`, not `glow_scale`).
- `heca-config/src/theme.rs:61-62` — test asserting `theme.glow_size == None`,
  `theme.intensity == Off` for a test theme (leave as-is unless the resolver
  tests need a fixture).

---

## 6. Repo state & branch strategy

- `origin/main` HEAD: `66adb41` (local main synced).
- **Phase 4 docs branch** `docs/compositor-phase-4` is **pushed** (commit
  `75c1857`) but **NOT PR'd and NOT merged** — it was committed without the
  user's review by mistake. It touches `README.md`, `theming-documentation.md`,
  `AGENTS.md`, `example.config.toml`, `BACKLOG.md`, `compositor-blur-refactor-plan.md`,
  `.planning/research/ARCHITECTURE.md`, `theming-plan.md`. **Do not touch that
  branch.** It is awaiting the user's review.
- **This task:** fresh branch off `main` (user-confirmed). Because Phase 4 edits
  the same doc files (README, theming-documentation, AGENTS, example.config,
  BACKLOG), expect a small rebase on those files **after Phase 4 merges**.
- No PRs are currently open.

---

## 7. Workflow rules (NON-NEGOTIABLE — the user is strict about these)

1. **Discuss/confirm before coding.** The user wanted to discuss this spec
   before any code. Be patient; don't fire off `ask_user` option dialogs
   prematurely or jump to implementation while they're still refining.
2. **Do NOT commit until the user reviews.** The user reviews every phase; only
   after they say it's OK do you commit + push + open a PR. (I violated this on
   Phase 4 — committed `75c1857` without review — and the user was upset. Don't
   repeat.)
3. **Do NOT merge PRs without explicit user approval.** "PR → main" means create
   the PR and stop.
4. **The USER does the review, not the assistant.** Do NOT run the rust-skill
   self-review step as a substitute for the user's review.
5. **Per-phase PR/review cycle** — each phase gets its own PR.
6. **No hardcoded color/style values** in Rust constructors — read from
  `heca-theme::Theme` / config. (Not directly relevant to this task, but a
  standing project rule.)
7. **Run `cargo clippy --workspace --all-targets --all-features`** and fix all
   warnings before any commit; `cargo test --workspace` green (minus the known
   env bash test). Don't rely on lint suppression as a "fix".
8. **Don't run `cargo fmt`** in this repo (no pinned rustfmt.toml; hand-format
   to match).

---

## 8. Open questions

- None. All decisions resolved (§2.7 `strength_scale` curve confirmed:
  `0.0/0.5/1.0/1.6`). Ready to implement on user's "go".

---

## 9. Suggested implementation order

1. Resolve §2.7 with the user.
2. Code 3.1 (appearance fields + resolvers + tests) → `cargo test -p heca-config`.
3. Code 3.3 (remove `glow_scale`, add `GlowLevel::strength_scale`, fix marker_group)
   → `cargo build --workspace --all-targets`; grep gates.
4. Code 3.2 (wire override into `chrome_gui_theme`).
5. Code 3.4 (fix `Intensity` doc comments) + 3.5 (`example.config.toml`).
6. Full verification (§3.6).
7. Docs 4.1–4.5 (README, terminal-implementation, theming-documentation, AGENTS,
   BACKLOG) — full token audit.
8. Show the user the full diff for review. **Do not commit.**