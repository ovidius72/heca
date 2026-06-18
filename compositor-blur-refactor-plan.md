# Compositor Blur Refactor — z-Layer Background Frost

> Plan for replacing the tiled-tint + OS-vibrancy blur approach with a **heca-owned
> z-layer background**: a blurred translucent gradient behind panes, giving a
> real, cross-platform, tunable frosted-glass effect for both tiled and floating
> panes.
>
- **Status:** planning
- **Branch (to create):** `feature/compositor-blur-refactor` (off latest `origin/main`)
- **Depends on:** `feature/terminal-blur` (PR #125) findings — the architectural
  conclusion that heca (an app, not a compositor) cannot blur the real desktop
  cross-platform, and must blur content it renders itself.
- **Supersedes:** the frosted-tint approach in `feature/terminal-blur`
  (`terminal_frost_color` / `terminal_frost_opacity()` / the Pass A tint).

---

## 1. Goal & motivation

Give heca panes a **real frosted-glass blur** that:

1. **Works cross-platform** (Linux / macOS / Windows) — no OS-vibrancy dependency.
2. Is **heca-tunable** (`terminal_blur`-style knobs control the wgpu blur radius,
   not the OS).
3. Is **per-pane** (each pane opaque or translucent independently).
4. **Solves the floating-pane text collision** — a floating terminal's text must
   not collide with the tiled content behind it (no sharp un-blurred leak).

### Why the current approach fails

- **Tiled panes** currently get a frosted *tint* (`terminal_frost_color` rect over
  macOS vibrancy). heca cannot blur the desktop (the OS composites it *behind* the
  window, outside heca's render target), so the tint is indistinguishable from the
  OS vibrancy and is macOS-only.
- **Floating panes** get a real blur of tiled content, but the backdrop stamps it
  at `floating_frost_opacity = √(blur/100) ≈ 0.95`, leaving **5% sharp un-blurred
  content** leaking through → the "text collides / doesn't look good" defect.

### The z-layer model (the fix)

heca renders its **own background layer at z=0** (a 2-color gradient, blurred once
and cached), then composites pane content at z=1 (tiled) / z=2 (floating) over it:

```
z=0   background layer (heca-owned): 2-color vertical gradient
        → blurred by heca (wgpu Kawase), ONCE, cached (static — recompute on resize/param change)
        → composited at alpha = background_alpha() (1 − background_transparency/100)
z=1   tiled panes: translucent terminal surface (terminal_transparency) reveals z=0 frost
z=1.5 blurred-z=1-content (dynamic, per-frame) — only when floating panes exist
z=2   floating panes: translucent surface reveals z=1.5 frost (blurred tiled content)
        → backdrop stamped at 100% (no raw leak → no collision)
```

This is exactly niri/hyprland's model, but heca owns the "wallpaper" (the gradient)
instead of the OS, so it is **cross-platform and tunable**.

---

## 2. Locked decisions (defaults baked into the plan)

| Decision | Choice | Rationale |
|---|---|---|
| Knob naming | **New clean knobs** (`background_blur`, `background_gradient_top/bottom`, `background_transparency`); **drop** `terminal_blur`/`terminal_frost_color` for tiled | Two names for one thing is confusing; manual config migration is acceptable |
| Gradient | **Vertical only** (top→bottom), 2 colors | Simplest shader; sufficient for v1 |
| z=0 default opacity | **Opaque** (`background_transparency = 0`) | Guarantees cross-platform clean frost (no sharp desktop on Linux/Win); user opts into translucency |
| z=0 blur | **Static + cached** (recompute only when dirty: resize / gradient / blur change) | Gradient is static → blur once, cheap |
| Floating backdrop | **100% opacity** when blur active (drop `floating_frost_opacity` coupling) | Kills the 5% sharp-text leak → no collision; frost visibility = `terminal_floating_transparency` |
| Float blur reuse | Reuse the shared `state.blur` for the (rare) dirty-frame z=0 blur; copy `view_b` → cache before the floating blur runs | Minimal new GPU resources (only z0 render target + cache texture) |

---

## 3. Scope & out-of-scope

**In scope:**
- z=0 gradient + static cached blur (heca-renderer).
- Config knobs + resolvers (heca-config).
- Pipeline integration: z=0 base blit, remove tiled tint, floating backdrop-100% fix (heca).
- Docs (keybindings.toml, README, AGENTS, ARCHITECTURE).
- Visual tuning to a user-approved look.

**Out of scope (explicitly deferred):**
- Horizontal/diagonal/radial gradients, multi-stop gradients (v1 = 2-color vertical).
- Background **image** support (wallpaper). The design leaves room for it later
  (z=0 source is content-agnostic), but v1 is gradient-only.
- OS blur (vibrancy/Acrylic/compositor) as a *fallback* — documented as an optional
  macOS bonus, not wired.
- Per-pane *individual* blur radius (one z=0 blur for all tiled panes; floating has
  its own dynamic blur).

---

## 4. Task-relation convention

- **Relations** list task ids a task depends on (must be done first).
- **Check** is the concrete, runnable verification for the task.
- **A task is `checked` only at the end of a review of that task.** The review
  runs the task's Check (command / visual confirmation), inspects the result,
  and only then marks the `[ ]` → `[x]`. A box ticked without a completed review
  is invalid. Reviews are per-task; the **Phase exit** is a separate, combined
  tests + review pass before starting the next phase.
- **Phase exit** = tests + review together before starting the next phase.
- Per AGENTS.md: no commit until the user has tested; rust-skill review at phase end
  (not per task); `cargo clippy --workspace --all-targets --all-features` clean.
- **Review = review the work just done against its Check** (does the build pass /
  does the test pass / does the user confirm the visual), then tick the box.

---

## Phase 0 — Setup & baseline

**Goal:** clean branch off latest `origin/main` with a green build, before any change.

### Tasks

- [ ] **0.1 Sync with main** — `git fetch origin && git checkout -b feature/compositor-blur-refactor origin/main`.
  - Relations: none.
  - Check: `git log -1` shows the latest `origin/main` HEAD; branch is clean.

- [ ] **0.2 Baseline green** — confirm the starting point builds, tests pass, clippy is at the known pre-existing baseline (9 lints — 8 selection `dead_code` + 2 `too_many_arguments`; see PR #125 notes).
  - Relations: 0.1.
  - Check:
    - [ ] `cargo build --workspace --all-targets` → green.
    - [ ] `cargo test --workspace` → green.
    - [ ] `cargo clippy --workspace --all-targets --all-features` → only the 9 known pre-existing warnings (record the exact count).

### Phase 0 exit
- [ ] Tests + clippy baseline recorded. No review needed (no code changed). Proceed to Phase 1.

---

## Phase 1 — GPU plumbing: gradient + background layer (heca-renderer, headless)

**Goal:** two new, isolated, unit-testable GPU primitives in `heca-renderer` — a
gradient fill pipeline and a `BackgroundLayer` (gradient → static cached blur →
blit source). **No app wiring yet.** This phase stays within `heca-renderer` so it
is headless and testable in isolation.

### Tasks

- [ ] **1.1 `heca-renderer/src/gradient.rs` — gradient pipeline** — a `GradientRenderer` that fills a render-target texture view with a 2-color **vertical** gradient (top→bottom) via a fullscreen-triangle pipeline. Inline wgsl (vertex: fullscreen triangle; fragment: `mix(top, bottom, uv.y)`). Colors are linear f32x4 (match `primitive_renderer` convention). No app types.
  - API:
    ```rust,ignore
    impl GradientRenderer {
        pub fn new(device: &Device, format: wgpu::TextureFormat) -> Self;
        pub fn render(&self, device: &Device, queue: &Queue, encoder: &mut CommandEncoder,
                      target: &TextureView, top: [f32;4], bottom: [f32;4]);
    }
    ```
  - Relations: none (leaf primitive).
  - Check:
    - [ ] `cargo build -p heca-renderer` green.
    - [ ] Smoke unit test (existing renderer test harness / headless device if available): construct `GradientRenderer::new` without panic. If no headless device, a compile-only smoke is acceptable and GPU behavior is verified visually in Phase 5.

- [ ] **1.2 `heca-renderer/src/background.rs` — `BackgroundLayer`** — a struct owning the z=0 layer state and a **static cached blur**:
  ```rust,ignore
  pub struct BackgroundLayer {
      z0_tex: Texture, z0_view: TextureView,      // gradient render target
      cache_tex: Texture, cache_view: TextureView, // blurred result (cached)
      dirty: bool,
      // cached params (to detect change → set dirty)
      top: [f32;4], bottom: [f32;4], blur_radius: f32,
      size: (u32, u32), format: wgpu::TextureFormat,
      gradient: GradientRenderer,
  }
  impl BackgroundLayer {
      pub fn new(device, format, w, h) -> Self;
      pub fn resize(&mut self, device, w, h);               // sets dirty
      pub fn set_params(&mut self, top, bottom, blur_radius); // sets dirty if changed
      pub fn render(&mut self, device, queue, encoder, blur: &Blur) -> &TextureView;
      // if dirty: gradient.render → z0; blurred = blur.process(z0); copy blurred → cache; dirty=false
      // if clean: return cache_view (no work)
  }
  ```
  - `cache_tex` needs `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_DST`; the copy is `encoder.copy_texture_to_texture(blurred_view → cache_tex)`. Reuses the shared `Blur` (passed in) so no dedicated ping-pong pair is allocated. **Critical:** the caller must finish using / copy the returned blurred view before the next `blur.process` call in the same frame (documented in the method).
  - Relations: 1.1 (uses `GradientRenderer`); reuses existing `Blur`.
  - Check:
    - [ ] `cargo build -p heca-renderer` green.
    - [ ] Unit test: `render()` twice with no `set_params`/`resize` between → asserts the blur path runs only once (dirty flag). If no headless device, assert via a pure helper `fn params_changed(&self, ...) -> bool` factored out to be unit-testable without a device.

- [ ] **1.3 Export modules in `heca-renderer/src/lib.rs`** — `pub mod gradient; pub mod background;` + re-export `BackgroundLayer` and `GradientRenderer` from the crate root (or a renderer prelude) so the app can import them.
  - Relations: 1.1, 1.2.
  - Check:
    - [ ] `cargo build -p heca-renderer` green.
    - [ ] `cargo test -p heca-renderer` green.

### Phase 1 exit — tests + review
- [ ] `cargo build -p heca-renderer` + `cargo test -p heca-renderer` green.
- [ ] **Review together:** confirm both modules are headless (no `wgpu::Surface`, no app types), reuse `Blur`, and the dirty-cache logic is correct. No app coupling. Proceed to Phase 2.

---

## Phase 2 — Config: new z=0 knobs (heca-config, additive only)

**Goal:** add the new config surface **additively** (no removals yet — keeps the
workspace building). Removal of the old tint path happens in Phase 3 alongside
the render change, so there is no broken intermediate state.

### Tasks

- [ ] **2.1 Add `Appearance` fields (`heca-config/src/appearance.rs`)** — add
  ```rust,ignore
  pub background_gradient_top: Option<Color>,
  pub background_gradient_bottom: Option<Color>,
  pub background_blur: u8,            // 0..=100
  pub background_transparency: u8,    // 0..=100 (0 = opaque z0)
  ```
  with serde defaults (`#[serde(default)]`). Mirror the existing `terminal_blur`/`terminal_transparency` style + clamping.
  - Relations: none.
  - Check: `cargo build -p heca-config` green.

- [ ] **2.2 Resolvers (`heca-config/src/appearance.rs`)** —
  - `effective_background_gradient_top(&self, theme) -> Color`: config → `theme.background_gradient_top` → `theme.background` (fallback chain).
  - `effective_background_gradient_bottom(&self, theme) -> Color`: config → `theme.background_gradient_bottom` → a derived shifted variant → `theme.background`.
  - `background_blur_radius() -> f32`: `terminal_blur_radius()`-style mapping (`background_blur` 0..100 → 0..`MAX_BLUR_PX` logical px).
  - `background_alpha() -> f32`: `(1.0 - background_transparency as f32 / 100.0).clamp(0.0,1.0)`.
  - Relations: 2.1.
  - Check: unit tests in `appearance.rs` — defaults, override, clamping, fallback chain; `cargo test -p heca-config` green.

- [ ] **2.3 Theme defaults (`heca-config/src/theme.rs` + `defaults.rs`)** — add `background_gradient_top` + `background_gradient_bottom` to the `Theme` struct and to the mocha + latte theme definitions (a tasteful top→bottom pair, e.g. mocha: a slightly lighter + the base bg). Defaults in `defaults.rs` for the new `Appearance` fields: `background_blur = 0`, `background_transparency = 0` (opaque by default, per the locked decision).
  - Relations: 2.1.
  - Check: unit test asserting the default theme has gradient colors; `cargo test -p heca-config` green.

- [ ] **2.4 Workspace still green (additive only)** — confirm no removals were made; the old tint fields still exist (removed in Phase 3).
  - Relations: 2.1–2.3.
  - Check:
    - [ ] `cargo build --workspace --all-targets` green.
    - [ ] `cargo test --workspace` green.

### Phase 2 exit — tests + review
- [ ] `cargo test -p heca-config` green; `cargo build --workspace` green.
- [ ] **Review together:** resolver fallback chains, default values (opaque z0), knob names. Proceed to Phase 3.

---

## Phase 3 — Pipeline integration + remove tint (heca app)

**Goal:** wire z=0 into the render pipeline, remove the tiled tint path, and apply
the floating backdrop-100% fix. This is the riskiest phase (core pipeline reorder);
build incrementally and review the diff carefully.

### Tasks

- [ ] **3.1 Own `BackgroundLayer` in `AppState` (`heca/src/app_state.rs`)** — add a `pub background: heca_renderer::background::BackgroundLayer` field; construct it in the renderer-init path (where `blur` / `compositor` are created), sized to the physical framebuffer; add an accessor if needed.
  - Relations: Phase 1 (BackgroundLayer exists).
  - Check: `cargo build -p heca` green.

- [ ] **3.2 Resize hook (`heca/src/app/events.rs`)** — in `WindowEvent::Resized`, alongside `state.blur.resize(...)` and `state.compositor.resize(...)`, call `state.background.resize(&device, w, h)` (sets dirty → z=0 reblurs next frame).
  - Relations: 3.1.
  - Check: `cargo build -p heca` green; behavior verified in Phase 5 (resize test).

- [ ] **3.3 z=0 base blit (`heca/src/app/render.rs`)** — after the scene clear pass and **before** the pane stencil/content passes:
  1. `state.background.set_params(top, bottom, blur_radius)` from `appearance.effective_background_gradient_*` + `background_blur_radius() * scale_factor`.
  2. `let bg_view = state.background.render(&device, &state.queue, &mut encoder, &state.blur);`
  3. Blit `bg_view` into `scene_view` as a fullscreen textured quad at alpha = `appearance.background_alpha()`. Reuse the `Backdrop::draw` (full-screen dst) or the composite blit with an alpha uniform — pick whichever needs the least new pipeline; if a new alpha-blit pipeline is needed, add it to `backdrop.rs` or `composite.rs`.
  - Relations: 3.1, 3.2.
  - Check: `cargo build -p heca` green; **visual** in Phase 5 (gradient frost visible behind translucent panes).

- [ ] **3.4 Remove tiled tint — Pass A (`heca/src/app/render.rs`)** — delete the `needs_frosted_backdrop` gate, the frosted-tint `draw_rect` block, and the "Frosted terminal backdrop" comment block. Tiled panes now render translucent (Pass 2, `surface_alpha`) **directly over z=0** — the frost is z=0 showing through, not a tint. Keep the stencil content-clip (Pass 2) intact.
  - Relations: 3.3 (z=0 must be in place first).
  - Check: `cargo build -p heca` green; **visual** in Phase 5 (tiled frost = z=0).

- [ ] **3.5 Remove old tint config (`heca-config/src/appearance.rs`)** — delete `terminal_frost_color`, `terminal_frost_opacity()`, and `effective_terminal_frost_color()`. Update any remaining references (grep `terminal_frost` across the workspace).
  - Relations: 3.4 (render.rs no longer uses them).
  - Check:
    - [ ] `cargo build --workspace --all-targets` green.
    - [ ] `cargo test -p heca-config` green.
    - [ ] `rg "terminal_frost" --glob '!*.md'` returns no code references (only docs, fixed in Phase 4).

- [ ] **3.6 Floating backdrop-100% fix (`heca/src/app/render.rs`)** — in the floating-pane loop, change the `backdrop.draw` opacity argument from `floating_frost_opacity` → `1.0` (the blurred tiled content fully replaces the raw behind — no sharp leak). Drop `floating_frost_opacity` (and `terminal_floating_frost_opacity()` if it exists); floating frost visibility is now driven solely by `terminal_floating_transparency` (cell surface alpha).
  - Relations: 3.4.
  - Check: `cargo build -p heca` green; **visual** in Phase 5 (no text collision, frost visible at chosen transparency).

- [ ] **3.7 Showcase + examples signature updates (`heca-renderer/examples/showcase.rs`)** — update any `backdrop.draw` / `primitive` / `text.render` calls whose signatures changed (e.g. if a new alpha-blit pipeline altered `Backdrop::draw`).
  - Relations: 3.3, 3.6.
  - Check: `cargo build --workspace --all-targets` green (examples included).

### Phase 3 exit — tests + review
- [ ] `cargo build --workspace --all-targets` green.
- [ ] `cargo test --workspace` green.
- [ ] **Review together (the critical review):** walk the full pipeline order in `render.rs` — clear → z=0 blit → tiled stencil → tiled content (translucent over z=0) → chrome borders → floating blur capture → floating stencil → floating backdrop(100%) + content → grid-ui chrome → sidebar. Confirm no stencil state leaks between the tiled and floating passes, and the z=0 blit does **not** get clipped by the tiled stencil (z=0 is pre-stencil). Proceed to Phase 4.

---

## Phase 4 — Documentation

**Goal:** document the new z-layer model and knobs; remove/replace the tint docs.

### Tasks

- [ ] **4.1 `keybindings.toml`** — replace the tiled-tint appearance block with the z=0 knobs (`background_gradient_top/bottom`, `background_blur`, `background_transparency`) + a concise z-layer model explanation + a **migration note** that `terminal_blur` / `terminal_frost_color` are removed (use `background_blur`).
  - Relations: Phase 3.
  - Check: doc reads consistently; no references to removed knobs as valid.

- [ ] **4.2 `README.md`** — update the appearance / blur section to the z-layer model.
  - Relations: 4.1.
  - Check: review.

- [ ] **4.3 `AGENTS.md`** — add the z-layer frost model to the rendering/appearance notes; reinforce the "no hardcoded color/style — read from theme/config" rule with the gradient as an example; note the `BackgroundLayer` is a heca-renderer primitive (headless), not a `heca-grid-ui` widget.
  - Relations: 4.1.
  - Check: review.

- [ ] **4.4 `.planning/research/ARCHITECTURE.md`** — document the z-layer model, `BackgroundLayer`, static blur cache, and the cross-platform rationale (app vs compositor).
  - Relations: 4.1.
  - Check: review.

### Phase 4 exit — review together
- [ ] Docs consistent across all four files; no stale references to the tint approach in code-facing docs. Proceed to Phase 5.

---

## Phase 5 — Visual tuning (with user)

**Goal:** iterate the frost to a user-approved look. This is the loop that
determines success — the GPU work is deterministic, the *look* is not.

### Tasks

- [ ] **5.1 Tiled frost baseline** — user sets `background_blur`, `background_transparency`, `background_gradient_top/bottom`, `terminal_transparency`; reload (`prefix+Shift+r`); report whether the tiled frost looks like real frosted glass.
  - Relations: Phase 3.
  - Check: user confirms "tiled frost looks good."

- [ ] **5.2 Blur strength tuning** — if the frost is too weak (shapes too sharp) or too strong (flat wash), tune `BLUR_PASSES` / `MAX_BLUR_PX` / the `background_blur_radius()` mapping in `appearance.rs`. Re-test.
  - Relations: 5.1.
  - Check: user confirms blur strength is right.

- [ ] **5.3 Floating frost + collision check** — user sets `terminal_floating_blur` + `terminal_floating_transparency`; confirm the floating terminal's text **does not collide** with the tiled content behind (the backdrop-100% fix) and the frost is visible.
  - Relations: 5.1.
  - Check: user confirms "floating looks good, no collision."

- [ ] **5.4 Resize correctness** — resize the window (and toggle chrome); confirm z=0 recomputes (no stale/blurry-at-wrong-resolution artifact).
  - Relations: 5.2.
  - Check: user confirms "resize is ok."

### Phase 5 exit — user sign-off
- [ ] User confirms tiled + floating frost look good, no collision, resize works. Proceed to Phase 6.

---

## Phase 6 — Phase-end review + cleanup + ship

**Goal:** quality gates, then ship (with user approval — no commit until the user
has tested, per AGENTS.md).

### Tasks

- [ ] **6.1 Rust-skill review** — load `/Users/antonio/.agents/skills/rust/SKILL.md` and run a formal review against the full diff (all phases). Fix findings.
  - Relations: Phase 5.
  - Check: review notes addressed; no HIGH/MED findings open.

- [ ] **6.2 Clippy clean** — `cargo clippy --workspace --all-targets --all-features`. Must be clean of **new** lints (the 9 pre-existing are allowed; add `#[allow(dead_code)]` + explanatory comments to the 8 `selection_model.rs` ones if not already done).
  - Relations: 6.1.
  - Check: clippy green (only known pre-existing, or zero).

- [ ] **6.3 Test sweep** — `cargo test --workspace` green; add/adjust any tests touched by the refactor (appearance resolvers, background dirty-flag, render behavior).
  - Relations: 6.2.
  - Check: all tests green.

- [ ] **6.4 Planning docs update** — update `.planning/STATE.md` + `PLAN.md` — mark the blur refactor done; reference this plan file.
  - Relations: 6.3.
  - Check: docs updated.

- [ ] **6.5 Commit + push + PR** — stage the change in clean, behavior-preserving slices; commit with a clear message; push `feature/compositor-blur-refactor`; open PR to `main`. **Do not commit until the user has tested and approved** (AGENTS.md gate).
  - Relations: 6.3 + user approval.
  - Check: PR open; CI green (or the repo's equivalent gate).

### Phase 6 exit — merged (after user approval + review)
- [ ] Blur refactor shipped; PR merged.

---

## 5. Risk register

| Risk | Mitigation | Where |
|---|---|---|
| Static blur cache stale after resize | `resize()` sets dirty; Phase 5.4 verifies | background.rs, events.rs |
| Gradient banding (2-color, no blur) | Blur hides it; add dithering only if user reports | gradient.rs |
| wgpu resource leak (2 new fullscreen textures) | Recreate on resize; review in Phase 1 + 3 | background.rs |
| Stencil state leaks between tiled/floating/z0 passes | z=0 blit is **pre-stencil**; Phase 3 critical review confirms | render.rs |
| Shared `Blur` reused for z0 + floating in one dirty frame | Copy z0 blurred view → cache **before** the floating `blur.process` call; documented in `BackgroundLayer::render` | background.rs, render.rs |
| Removing `terminal_blur` breaks user config | Migration note in keybindings.toml (Phase 4) | docs |

---

## 6. Success criteria (definition of done)

- [ ] **Cross-platform:** tiled + floating frost render identically via wgpu (no OS-vibrancy dependency) — verified by code review (no platform API in the frost path) + user visual check.
- [ ] **Tunable:** `background_blur` visibly changes the frost strength; the user confirms in Phase 5.
- [ ] **No collision:** floating terminal text does not collide with tiled content behind (backdrop-100%); user confirms in Phase 5.3.
- [ ] **Clean:** `cargo build --workspace --all-targets`, `cargo test --workspace`, `cargo clippy --workspace --all-targets --all-features` all green (only known pre-existing lints).
- [ ] **Documented:** z-layer model + knobs documented in keybindings.toml, README, AGENTS, ARCHITECTURE.
- [ ] **Shipped:** PR merged after user approval.