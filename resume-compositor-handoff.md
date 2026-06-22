# Resume Handoff — Compositor Blur Refactor (z=0)

> **Purpose:** restart this workstream with full context, no re-discovery.
> Last updated: 2026-06-22. Branch: `feature/compositor-blur-refactor`. PR: **#165** (open, not merged).
> Plan: [`compositor-blur-refactor-plan.md`](compositor-blur-refactor-plan.md) (single source of truth — read it first).

---

## 1. What this is

Implementing the **z=0 compositor blur refactor**: heca renders its own blurred
gradient background at z=0, and panes composite translucently over it, so the
frosted-glass frost is **heca-owned and cross-platform** (no OS-vibrancy dependency).
Replaces the tiled-tint (`terminal_frost_color`/`terminal_blur`) approach.

The user's architectural model (locked, governs everything):
1. **Terminal** owns its translucency/blur via config knobs (`terminal_transparency`, `terminal_floating_blur`).
2. **Pane** = border + padding only, **no background fill** (verified: `paint_terminal_pane_shell` sets only `.border()`+`.radius()`; the `Pane` widget paints a transparent fill when no bg set).
3. **App/window** owns transparency + blur, and the bg is a **gradient** (not solid) so blur has content to blur. macOS has native vibrancy (OS blurs desktop behind a translucent window); Linux/Windows have no reliable cross-platform OS-vibrancy, so a translucent window there shows a **sharp** desktop — hence z=0 must own the frost and default opaque.
4. **macOS vibrancy is deferred** — try z=0 WITHOUT vibrancy first; revisit only if z=0 frost is insufficient on macOS. Do **not** wire vibrancy in Phases 1–5; it stays `Vibrancy::None`.

---

## 2. Status — what's done

| Phase | Status | Commit |
|---|---|---|
| 0 — Setup & baseline | ✅ done | (no commit; baseline recorded in plan) |
| 1 — GPU plumbing (gradient + BackgroundLayer) | ✅ done, user-reviewed | `dc3d0d7` |
| 2 — Config knobs (z=0 fields + resolvers + TOML-sourced grid_tron) | ✅ done, user-reviewed | `f32bdff` |
| 3 — Pipeline integration + remove tint | ⬜ **NEXT** | — |
| 4 — Documentation | ⬜ | — |
| 5 — Visual tuning (with user) | ⬜ | — |
| 6 — Review + cleanup + ship | ⬜ | — |

**Baseline (recorded):** clippy = 0 warnings. `cargo test --workspace` green **except**
the pre-existing env-dependent `terminal_backend_bash_integration_reports_success_error_and_cwd`
(`heca-core/src/backend/terminal.rs:801`) — verified failing on plain `origin/main`,
**excluded** baseline failure (do not block on it; do not "fix" it as part of this work).

---

## 3. Locked decisions (grill-me 2026-06-21 — see plan §0.1)

| # | Decision |
|---|---|
| Q1 translucency channel | **Option A** — theme `terminal_background` is **opaque**; `terminal_transparency`→`surface_alpha` is the **only** translucency channel. **ACTION (Phase 3):** fix `heca-theme/src/themes/latte.toml` `terminal_background = "#e6e9ef00"` → `"#e6e9ef"` (alpha-0 is the prime suspect for the deferred "no blur/transparency" regression). |
| Q2 gradient source | **Option C** — explicit `background_gradient_top/bottom` theme fields + derived fallback (top=background, bottom=darker(background)). |
| Q2-extra scanlines | **z=0.5** — scanlines render over the blurred gradient, under panes (frost with the background). Add a z=0.5 scanline overlay pass in the pipeline order (Phase 3). |
| Q3 tiled frost | **Option A** — drop `terminal_blur` + `terminal_frost_color` for tiled; tiled frost = z=0 (`background_blur`). Keep `terminal_transparency` + `terminal_floating_blur`. **ACTION (Phase 4):** update `example.config.toml` + docs removing `terminal_blur` references. |
| Q4 z=0 default | **Option A** — opaque by default (`background_transparency = 0`). |
| Q5 execution | #163 and #164 were already merged; branch off `origin/main` (`351d857`). |
| Q6 vibrancy | **DEFERRED** — try z=0 without vibrancy first; parked work (light/dark detection on `Theme` + `setAppearance` + live-reload via retained effect-view handle) only if z=0 is insufficient. Not in Phases 1–5. |

---

## 4. What was built (Phases 0–2)

### Phase 1 — `heca-renderer` (headless primitives)
- `heca-renderer/src/gradient.rs` + `gradient.wgsl` — `GradientRenderer`: fills a
  render target with a 2-color vertical gradient (fullscreen-triangle pipeline,
  linear `f32x4` uniforms). `render(&self, queue, encoder, target, top, bottom)` —
  no `device` param (dropped per review).
- `heca-renderer/src/background.rs` + `background.wgsl` — `BackgroundLayer`: owns z=0
  state. `render(&mut self, device, queue, encoder, blur: &Blur) -> &TextureView`:
  if dirty → gradient→`z0` → `blur.process(z0)` → fullscreen **blit** of blurred
  result into `cache_tex`; else returns cached view (zero work). Static + cached
  (recompute only on `resize`/`set_params`). `params_changed`/`params_differ`
  factored as a device-free unit-tested helper.
- Exported in `heca-renderer/src/lib.rs`: `pub mod gradient; pub mod background;`.
- Tests: `gradient_params_layout_matches_wgsl_uniform` (pins `#[repr(C)]` layout:
  size 32, align 4, top@0, bottom@16), `params_differ_*`.

### Phase 2 — config (additive)
- `heca-config/src/appearance.rs`:
  - Fields: `background_gradient_top/bottom: Option<Color>`, `background_blur: u8`
    (default 0), `background_transparency: u8` (default 0 = opaque z=0).
  - Resolvers: `effective_background_gradient_top/bottom(&self, theme)`,
    `background_blur_radius()` (0..100 → 0..`MAX_BLUR_PX`), `background_alpha()`.
  - Tests: `z0_background_knobs_map_to_derived_values`,
    `z0_gradient_default_resolves_to_theme_colors`,
    `z0_gradient_config_override_wins_over_theme`,
    `z0_gradient_unset_theme_fields_fall_back_to_derived`.
- `heca-theme/src/theme.rs`:
  - `Theme` struct: `background_gradient_top/bottom: Option<Color>` (`#[serde(default)]`).
  - Accessors `effective_background_gradient_top/bottom()` (derived fallback via
    `GRADIENT_BOTTOM_DARKEN_FACTOR = 0.08`, with rationale comment).
  - **`Theme::grid_tron()` now parses `include_str!("themes/grid_tron.toml")`** —
    single source of truth, ALL hardcoded `Color::rgb` removed from the Rust
    constructor (user directive). `Theme::default()` returns this.
  - `bundled_themes()` in `loader.rs` made `pub(crate)`; the gradient-contract test
    iterates it (no hardcoded theme-name list).
- Bundled TOMLs (`grid_tron`/`mocha`/`latte`): explicit `background_gradient_top/bottom`
  pairs (tasteful top-lighter/bottom-darker per theme).

---

## 5. Phase 3 — NEXT (pipeline integration + remove tint) — the riskiest phase

**Goal:** wire z=0 into the render pipeline, remove the tiled tint path, apply the
floating backdrop-100% fix. Build incrementally; review the diff carefully. Read
the plan's Phase 3 section in full before starting.

### Tasks (from the plan)
- **3.1** Own `BackgroundLayer` in `AppState` (`heca/src/app_state.rs`): add
  `pub background: heca_renderer::background::BackgroundLayer`; construct it in the
  renderer-init path (where `blur`/`compositor` are created), sized to the physical
  framebuffer.
- **3.2** Resize hook (`heca/src/app/events.rs`): in `WindowEvent::Resized`, alongside
  `state.blur.resize(...)`/`state.compositor.resize(...)`, call
  `state.background.resize(&device, w, h)` (sets dirty).
- **3.3** z=0 base blit (`heca/src/app/render.rs`): after scene clear, **before** pane
  stencil/content passes:
  1. `state.background.set_params(top, bottom, blur_radius)` from
     `appearance.effective_background_gradient_*(theme)` + `background_blur_radius() * scale_factor`.
  2. `let bg_view = state.background.render(&device, &state.queue, &mut encoder, &state.blur);`
  3. Blit `bg_view` into `scene_view` as a fullscreen textured quad at
     alpha = `appearance.background_alpha()`. Reuse `Backdrop::draw` (full-screen dst)
     or add an alpha-blit pipeline — pick the least new pipeline.
- **3.4** Remove tiled tint — Pass A (`heca/src/app/render.rs`): delete the
  `needs_frosted_backdrop` gate + frosted-tint `draw_rect` block + "Frosted terminal
  backdrop" comment. Tiled panes now render translucent (Pass 2, `surface_alpha`)
  directly over z=0. Keep the stencil content-clip (Pass 2) intact.
- **3.4b** Remove the `content_canvas_fill()` stopgap (`heca/src/app/render.rs`): delete
  `content_canvas_fill`, `FillRect`, `subtract_fill_rect`, `uncovered_fill_rects` +
  their tests (`content_canvas_fill_*`, `uncovered_fill_rects_exclude_pane_rectangles`).
  `rg "content_canvas_fill|FillRect|uncovered_fill_rects|subtract_fill_rect" --glob '!*.md'`
  must return no code refs.
- **3.5** Remove old tint config (`heca-config/src/appearance.rs`): delete
  `terminal_frost_color`, `terminal_frost_opacity()`, `effective_terminal_frost_color()`.
  `rg "terminal_frost" --glob '!*.md'` → also catches `heca-theme/src/themes/latte.toml`
  (`terminal_frost_color = "#e6e9ef"`) and `example.config.toml` — strip those keys.
- **3.6** Floating backdrop-100% fix (`heca/src/app/render.rs`): in the floating-pane
  loop, change the `backdrop.draw` opacity arg from `floating_frost_opacity` → `1.0`.
  Drop `floating_frost_opacity`/`terminal_floating_frost_opacity()`. Floating frost
  visibility now driven solely by `terminal_floating_transparency`.
- **3.7** Showcase + examples (`heca-renderer/examples/showcase.rs`): update any
  signature changes; verify the theme switcher still cycles `grid_tron`/`mocha`/`latte`.

### Phase 3 critical review (pipeline order)
Walk the full `render.rs` order: clear → z=0 blit → tiled stencil → tiled content
(translucent over z=0) → chrome borders → floating blur capture → floating stencil →
floating backdrop(100%)+content → grid-ui chrome → sidebar. Confirm no stencil state
leaks between tiled/floating passes and the z=0 blit is **not** clipped by the tiled
stencil (z=0 is pre-stencil).

### Critical translucency fix (Q1, fold into 3.4b or 3.5)
`heca-theme/src/themes/latte.toml` `terminal_background = "#e6e9ef00"` (alpha 0) →
`"#e6e9ef"` (opaque). This is the prime suspect for the deferred "no blur/transparency"
regression — the alpha-0 bypasses `terminal_transparency`→`surface_alpha`. Phase 5.1
pre-check verifies it.

---

## 6. Build / test / lint state

```bash
cargo build --workspace --all-targets        # green
cargo clippy --workspace --all-targets --all-features   # 0 warnings (baseline)
cargo test -p heca-renderer                   # 15 unit + 3 integration
cargo test -p heca-theme                       # 20
cargo test -p heca-config                      # 50
cargo test --workspace                        # green EXCEPT the env bash test (excluded)
```
**Do not** run `cargo fmt` (repo has no pinned rustfmt.toml; hand-format to match).

---

## 7. Workflow rules (from the user — follow strictly)

1. **After each phase is complete → the USER reviews** (not me). I must NOT run a
   rust-skill self-review step — the user does the review. I just complete the phase
   and hand it over.
2. **If the review is OK → commit + push + PR → `main`.** (PR #165 is the vehicle;
   pushing to `feature/compositor-blur-refactor` updates it.)
3. **If the review is not OK → rework** (fix findings, re-verify, re-hand to user).
4. **Do NOT merge any PR without explicit user approval.** (User was upset when PR #160
   was merged without permission.) "PR -> main" = create/push the PR and stop.
5. **No "for now" fixes.** No lint suppression as a fix. No hardcoded color/style in
   active render/widget paths — colors live in the `Theme`/TOML.
6. Read `AGENTS.md` + the plan before each phase. The plan is the source of truth.

---

## 8. Key files / symbols

**Phase 1 (done):**
- `heca-renderer/src/gradient.rs` — `GradientRenderer::new`, `render(queue, encoder, target, top, bottom)`
- `heca-renderer/src/background.rs` — `BackgroundLayer::{new, resize, set_params, render, params_changed, is_dirty, size}`, free `params_differ`
- `heca-renderer/src/blur.rs` — existing `Blur::process(device, queue, encoder, src_view, radius) -> &TextureView` (reused; returned view valid until next `process`/`resize`)
- `heca-renderer/src/backdrop.rs` — existing `Backdrop::draw(...)` (alpha-blit candidate for 3.3)

**Phase 2 (done):**
- `heca-config/src/appearance.rs` — `AppearanceConfig`: `background_gradient_top/bottom`, `background_blur`, `background_transparency`; resolvers `effective_background_gradient_top/bottom`, `background_blur_radius`, `background_alpha`
- `heca-theme/src/theme.rs` — `Theme`: `background_gradient_top/bottom`, `effective_background_gradient_top/bottom()`, `Theme::grid_tron()` (TOML-sourced), `Theme::default()`
- `heca-theme/src/loader.rs` — `bundled_themes()` (pub(crate)), `load_theme`, `available_themes`
- `heca-theme/src/themes/{grid_tron,mocha,latte}.toml` — explicit gradient colors

**Phase 3 targets:**
- `heca/src/app_state.rs` — add `background: BackgroundLayer` field
- `heca/src/app/events.rs` — `WindowEvent::Resized` hook
- `heca/src/app/render.rs` — z=0 blit (3.3), remove tiled tint/Pass A (3.4), remove `content_canvas_fill` stopgap (3.4b), floating backdrop-100% (3.6); symbols: `needs_frosted_backdrop`, `content_canvas_fill`, `FillRect`, `subtract_fill_rect`, `uncovered_fill_rects`, `floating_frost_opacity`
- `heca/src/app/terminal_render.rs` — `paint_terminal_pane_shell` (already border-only, no change)
- `heca-renderer/src/terminal.rs` — `surface_alpha` path (`surface_bg[3] = default_bg[3] * surface_alpha`)
- `heca-config/src/appearance.rs` — remove `terminal_frost_color`/`terminal_frost_opacity()`/`effective_terminal_frost_color()` (3.5)
- `heca-theme/src/themes/latte.toml` — fix `terminal_background` to opaque `#e6e9ef`; strip `terminal_frost_color`
- `example.config.toml` — strip `terminal_frost_color`/`terminal_blur` (Phase 4)
- `heca-renderer/examples/showcase.rs` — verify theme switcher (3.7)

---

## 9. How to resume

```bash
cd /Users/antonio/projects/heca-theme-analyze
git checkout feature/compositor-blur-refactor
git pull --ff-only origin feature/compositor-blur-refactor   # or fetch + merge origin/main if main moved
git log --oneline -3        # expect: f32bdff (Phase 2), dc3d0d7 (Phase 1), d40ec0f (plan lock)
# Read the plan Phase 3 section, then implement 3.1 → 3.7 in order.
# After Phase 3 builds+tests+clippy green, hand to the user for review (DO NOT self-review).
```

**First action on resume:** re-read `compositor-blur-refactor-plan.md` Phase 3 + §0.1
decisions, then start Task 3.1 (own `BackgroundLayer` in `AppState`). Keep each task
building (`cargo check -p heca`) before moving to the next. Phase 3 exit = full
workspace build + test + the critical pipeline-order review.

---

## 10. Open PRs / branches

- **PR #165** — `feature/compositor-blur-refactor` → `main`. **Open, NOT merged.**
  Contains: plan decision lock (d40ec0f) + Phase 1 (dc3d0d7) + Phase 2 (f32bdff).
  Awaits user merge approval (likely at the end, or per-phase per user's call).
- Merged already: #163 (compositor-blur plan reconcile), #164 (terminal plan reconcile).
- `main` HEAD at branch creation: `351d857`.