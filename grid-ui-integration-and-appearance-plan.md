# heca — grid-ui Integration + Appearance (Transparency/Blur) + Pluggable-Chrome Plan

**Date:** 2026-06-13
**Author:** architecture pass (Claude) — coordinated with PR #99 (terminal/content, other agent)
**Status:** Forward plan. Organized as **parallel workstreams** with hard file-ownership boundaries so
multiple agents (Claude + delegated + the #99 author) can work simultaneously without merge collisions.

---

## 0. Architecture snapshot (the ground truth this plan is built on)

Crates and their role (do not blur these boundaries):

| Crate | Role | GPU? |
|---|---|---|
| `heca-core` | layout engine (NIRI scrolling tiling), backend trait, **canonical geometry** (`layout/types.rs`: `Rectangle`/`Point`/`Size`) | no |
| `heca-config` | config.toml model + loader + live reload, themes | no |
| `heca-grid-ui` | **GPU-free** widget library: `Component`/`Base`, `Scene`/`DrawCommand`, `LayoutEngine`, the chrome vocabulary (`ChromeRegion`, `DockFrame`, `Item`, `Row`, `RailCell`, …), `FocusManager`, overlay layer | no |
| `heca-renderer` | wgpu: `GridRenderer` (SDF rounded-rect + glow + brackets), `PrimitiveRenderer`, `TextRenderer` (glyph atlas, retained geometry), `Compositor` (offscreen scene texture + blit), `scene::enqueue_scene` (grid-ui `Scene` → renderer calls), `TerminalRenderer` (PR #99) | yes |
| `heca` | the app: `app_state`, `app/render.rs::render_frame`, input/keymap/action pipeline, sidebar (hand-drawn), mouse | host |

**Current integration reality (from the knowledge graph):**
- The app and the widget library are **two disconnected hubs**. `heca` uses `heca-grid-ui` only for `drag::*` types. **All chrome (sidebar/status) is hand-drawn** via `PrimitiveRenderer.draw_rect` + `TextRenderer.queue_text` in `render.rs`. `Scene`/`enqueue_scene`/`GridRenderer` are exercised **only by the showcase**.
- The single seam where grid-ui will plug in is **`heca/src/app/render.rs::render_frame`** (render) and **`handle_window_event`** (input routing). That's the only hot file shared with PR #99.

**PR #99 (other agent) — coordination fact:** synced on main, builds, mergeable. It owns **pane *content*** (terminal backend, `TerminalRenderer`, `terminal_host`, snapshot contract) and heavily rewrites `render_frame`'s **content pass**. It does **not** touch `sidebar/render.rs`, the grid-ui `Scene` bridge, or `GridRenderer`. **My chrome work stacks on #99** (branch off `refactor/paneid-newtype`) so we share one reconciled `render_frame`.

**The one factoring agreement that makes parallel work safe** (request to #99 author):
> Split `render_frame` into independent passes: `render_pane_content(...)` (terminal, `PrimitiveRenderer`) and `render_chrome(...)` (grid-ui `Scene` → `GridRenderer`+`TextRenderer`). Don't reshape the chrome region; I own it.

---

## 1. Workstream map (ownership + parallelism)

Each workstream (WS) names the **crate(s) + files it owns**. Two WS never write the same file except the
explicitly-flagged shared seam, which is serialized via the #99 stack.

| WS | Title | Owns (files/crates) | Depends on | Can start now? | Suggested owner |
|----|-------|---------------------|-----------|----------------|-----------------|
| **A** | grid-ui chrome render integration | `heca/src/chrome/**` (new), `app/render.rs` *chrome pass only*, `app_state.rs` (+`GridRenderer`), `startup.rs` | #99 merged | ⛔ after #99 | **Claude** (wrote the renderer) |
| **B** | Window transparency + OS vibrancy (backdrop blur) | `heca/src/app/startup.rs` (window+surface), `main.rs` (apply vibrancy), `app/render.rs` *clear-color only* | WS-D config contract | 🟡 mostly (window/surface yes; clear-color coord w/ #99) | Claude or delegated |
| **C** | In-app GPU blur (frosted overlays, true blur amount) | `heca-renderer/src/blur.rs` (new) + `Compositor` | app adopts `Compositor` (part of WS-A) | 🟡 renderer part yes; app wiring after WS-A | Claude (renderer) |
| **D** | Appearance config (transparency/blur in config.toml) | `heca-config/src/appearance.rs` (new) or `settings.rs` fields, `defaults.rs`, loader live-reload, themes | — | ✅ now | delegated (isolated crate) |
| **E** | Pluggable-chrome program (the architecture) | see E1–E8 below — mostly **new** modules + `heca-grid-ui` | per-substream | ✅ several now | Claude + delegated |

**Hot-file rule:** `heca/src/app/render.rs` is the *only* file WS-A, WS-B, and #99 all touch. It is serialized: #99 lands first → WS-A/B stack on it → the `render_frame` factoring keeps content/chrome/clear edits in separate functions. Everything else is collision-free by construction.

**Transparency/blur ownership split (answers "terminal up to the other agent?"):**
- **Window + surface transparency, OS vibrancy, the config contract** → **shared app concern = me** (WS-B/D). One window, one surface, one vibrancy call.
- **Chrome background alpha** (sidebar/status drawn via grid-ui) → **me** (WS-A reads `Appearance.opacity`).
- **Pane *content* background alpha** (terminal cell-grid bg, terminal blur participation) → **the #99 author** (their `TerminalRenderer` reads the same `Appearance` contract I define in WS-D). **Yes — terminal is theirs; I provide the config + window plumbing they consume.**

---

## 2. WS-A — grid-ui chrome render integration

**Goal:** turn the missing `render_frame → enqueue_scene → GridRenderer` edge into a real one; chrome becomes grid-ui `Component`s. This is pluggable-chrome **Phase 5/7 begin**, and the foundation everything chrome-side sits on.

**Base:** branch off `refactor/paneid-newtype` (stacked on #99).

- **A0. Renderer set.** Add a `GridRenderer` to `AppState` next to `PrimitiveRenderer`/`TextRenderer` (init in `startup.rs`). The showcase is the reference for owning both.
- **A1. Chrome scene module** — `heca/src/chrome/mod.rs` (new): a function `build_chrome_scene(state, regions) -> heca_grid_ui::Scene` that constructs a grid-ui `Component` tree (`ChromeRegion` + content), runs `LayoutEngine::compute` against the region rects, and paints to a `Scene`. **No app-state mutation; pure projection** (state → component tree → scene).
- **A2. Render pass** — add `render_chrome(...)` to `render.rs` (the factored chrome pass): `begin_frame` (already required post-#97), `enqueue_scene(&mut grid, &mut text, scene.base_layer())` + overlay segments, `grid.render(...)`, `text.render(...)`. **Status/bottom bar first** (cheapest, no DnD) to prove the pipe end-to-end inside the app.
- **A3. Sidebar → `ChromeRegion`.** Replace `render_sidebar_*` (primitive) with a `ChromeRegion::vertical().with_mode_signal(...)` hosting a workspace-tree component built from session state. Drag/selection stays inside the container (uses the already-shipped `drag::*` framework). This is the literal Phase 5 "sidebar becomes one container."
- **A4. Input routing.** Route pointer/key events to the chrome component tree via grid-ui `FocusManager::dispatch`/`offer_to_overlay` (the palette already does this in the showcase). `handle_window_event` gains a chrome-dispatch arm *before* the WM keymap, gated so it only claims events over chrome rects.

**Guardrails:** chrome component tree is **rebuilt-from-state each frame** (immediate-mode projection) until WS-E3 introduces retained chrome state; widgets stay presentation-only (canonical state in `AppState`).

**Deliverable PRs (stacked on #99):** A0–A2 (status bar proof) → A3 (sidebar) → A4 (input). Each independently reviewable.

**Progress (branch `grid-ui-chrome-integration`, local commits):**
- ✅ **A0** — `GridRenderer` on `AppState` + init/resize wiring (`15e1638`).
- ✅ **A1** — `crate::chrome::build_chrome_scene` status-bar projection + unit test (`4e73329`).
- ✅ **A2** — factored `render_chrome` pass; status bar through the grid-ui scene (`4118266`). *Still renders to the swapchain — must move onto the `Compositor` in F2.*
- ⏪ **A3a/A3b reverted 2026-06-14** (reset to `a62d35a`). They rendered the sidebar **as the workspace tree** — the wrong framing. The sidebar is a **SHELL** (see §2.1 of `pluggable-chrome-plugin-plan.md`); rebuild it styled + content-agnostic *after* the foundation.

### ⚠️ Reorganized 2026-06-14 — FOUNDATION-FIRST (decided with the user)

The original WS-A "chrome render first" order was wrong: visible chrome was built before the
transparency/compositor + shared-state foundation it must sit on (`build-future-proof`, and the
plan's own §7/§10 sequencing). New order:

- **F1 — Appearance config contract (WS-D).** `[appearance]` in `heca-config`. *(in progress)*
- **F2 — Main scene → `Compositor` + transparency (WS-B).** App renders offscreen → blit; transparent window + OS vibrancy. *This is "adapt the main scene first." A2's `render_chrome` moves onto this path.*
- **F3 — In-app blur primitive (WS-C).** Separable Gaussian for frosted chrome/overlays.
- **F4 — Shared chrome/UI state (Ph 2 / E3).** `heca/src/chrome/state.rs` — region visibility, collapse/mode, selection, scroll. Replaces the immediate-mode rebuild hack.
- **F5 — Sidebar SHELL styled like GRIDCN sidebar-nav.** Bracket frame, header, collapse toggle, `position:fixed` left, frosted; content-agnostic. *Then* mount a WorkspacesContainer inside.

*Note: #99 did **not** factor `render_frame` into content/chrome passes. `render_chrome` takes renderer fields individually (not `&mut AppState`) because `render_frame` holds `let theme = &state.theme;` across its body.*

---

## 3. WS-B — Window transparency + OS vibrancy (backdrop blur)

**Goal:** the translucent + blurred-backdrop look (macOS-vibrancy / Windows-acrylic) showing through transparent regions. This is the *backdrop* blur (blurs what's behind the window — desktop/other apps), an **OS-compositor feature**, not a wgpu effect.

**Mechanism (precise):**
- **winit:** create the window with `.with_transparent(true)`.
- **wgpu surface:** pick a transparency-capable `CompositeAlphaMode` (`PreMultiplied` or `PostMultiplied`, **not** `Opaque`) from `surface.get_capabilities(...).alpha_modes` (the app currently takes `alpha_modes[0]` blindly — make it appearance-aware).
- **Clear/background alpha:** the base clear (and any "app background" fill) must write `alpha < 1.0` where translucency is wanted. (This is the *one* `render_frame` line shared with #99 — coordinate.)
- **OS vibrancy/backdrop-blur:** add the **`window-vibrancy`** crate (Tauri's; supports macOS `NSVisualEffectView`, Windows `apply_acrylic`/`apply_blur`/`apply_mica`, Linux = best-effort/no-op). Apply once after window creation in `main.rs`, gated by config + `cfg!(target_os=...)`.

**Honest platform constraints (must be in the config UX):**
- OS backdrop blur is **material-based**, not a free numeric radius — macOS gives `NSVisualEffectMaterial` variants (`Sidebar`, `HudWindow`, `UnderWindowBackground`, …); Windows acrylic gives a tint+blur but limited radius control; Linux/Wayland generally cannot do it portably. So **`blur = on/off` + `material`/`vibrancy` choice** is the faithful backdrop-blur control. A free **`blur_amount`** numeric only fully applies to **in-app blur (WS-C)**.

**Files:** `startup.rs` (window/surface), `main.rs` (vibrancy apply), `render.rs` clear-color (coord w/ #99), `Cargo.toml` (`window-vibrancy`). Independent of WS-A except the clear-color line.

---

## 4. WS-C — In-app GPU blur (frosted overlays + true `blur_amount`)

**Goal:** blur the **app's own content** behind a translucent surface (frosted-glass command palette / modal / dropdown; optionally a frosted chrome panel). This is the only path that honors a **numeric `blur_amount`**, because the app owns those pixels (unlike the desktop behind a transparent window, which the OS owns).

**Mechanism:** the renderer already has `Compositor` (renders the scene to an offscreen texture, then blits). Add a **separable Gaussian blur** (`heca-renderer/src/blur.rs`: two passes H+V into a ping-pong texture) applied to a *region* of the scene texture before compositing a translucent panel over it.

**Dependency:** the **app must render through the `Compositor`** (offscreen texture) for this to work — today `render_frame` draws straight to the swapchain. Adopting the compositor is part of WS-A's maturation (the showcase already does it). So: **WS-C renderer code can be written now (against `Compositor`), but its app wiring lands after WS-A adopts the compositor.**

**Scope discipline:** start with **overlay frosting** (palette/modal) — bounded, high-value. "Frosted pane backgrounds" is a later extension and overlaps the terminal (the #99 author would frost the terminal bg using the same blur primitive).

**Files:** `heca-renderer/src/blur.rs` (new) + `Compositor` extension. **Zero conflict with #99** (separate renderer files; `Compositor` is app-unused today).

---

## 5. WS-D — Appearance config (config.toml)

**Goal:** one config contract that WS-A (chrome alpha), WS-B (window/vibrancy), WS-C (blur amount), and the **#99 terminal** (pane content alpha) all read. Live-reloadable (heca already reloads config).

**Proposed schema** (`[appearance]` section; new `heca-config/src/appearance.rs`):
```toml
[appearance]
transparent      = false      # master switch: transparent surface + alpha clears
opacity          = 0.95       # 0.0..1.0 global app/background opacity
pane_opacity     = 1.0        # 0.0..1.0 pane CONTENT bg (terminal reads this — #99)
chrome_opacity   = 0.92       # 0.0..1.0 sidebar/status bg (chrome reads this — WS-A)
blur             = false      # backdrop blur on/off (OS vibrancy)
vibrancy         = "sidebar"  # macOS material / win acrylic flavor (when blur=true)
blur_amount      = 12.0       # in-app blur radius (WS-C; ignored by OS backdrop blur)
```

**Contract rule:** `Appearance` is a plain `Copy`/`Clone` struct exposed from `heca-config`; **every consumer reads it, none owns it.** Terminal (content) reads `pane_opacity`; chrome reads `chrome_opacity`; window/surface read `transparent`/`blur`/`vibrancy`; the blur pass reads `blur_amount`. This is the same dispatch-only discipline the chrome-plugin plan mandates for state.

**Files:** `heca-config/src/appearance.rs` (new), `settings.rs` (add `pub appearance: AppearanceConfig` — **additive**, mild coord with #99's settings.rs additions), `defaults.rs`, loader reload, themes. **Isolated crate → ideal to delegate. Start now.**

---

## 6. WS-E — Pluggable-chrome program (parallelized)

The `pluggable-chrome-plugin-plan.md` Phases 1–11, re-cut into **independently shippable substreams** with crate/module ownership so they run in parallel and never collide with #99 or each other. The plan's own dependency arrows are preserved.

| Sub | Plan phase | Deliverable / owns | Depends | Parallel-safe now? | Owner |
|----|-----------|--------------------|---------|--------------------|-------|
| **E1** | Ph 1 | **Architecture contract doc** (ChromeHost / regions / shared-state / provider / overlay / dynamic-action contracts; geometry on `layout/types.rs`) | — | ✅ now | Claude (writing) |
| **E2** | Ph 7 | `heca-grid-ui` chrome widgets: `Sidebar` shell, `SidebarContainerFrame`, list/scroll primitive (needs renderer `PushClip` — **already merged in #94**) | E1 shape | ✅ now (isolated crate) | **delegated agent** |
| **E3** | Ph 2 | **Shared UI/chrome state** — `heca/src/chrome/state.rs` (region visibility, per-container collapse/search/selection, scroll offsets, drag, overlay stack). Lives outside widgets. | E1 | ✅ now (new module) | delegated/Claude |
| **E4** | Ph 3 | **ChromeHost + region hosts** — `heca/src/chrome/host.rs` (region registries, container placement, mount/unmount, move/reorder, drop targets) | E3 | after E3 | Claude |
| **E5** | Ph 4/5 | **Provider model + `WorkspacesContainerProvider`** — migrate sidebar logic into one built-in provider/container | E4 + **WS-A render** | after E4 + #99 | Claude |
| **E6** | Ph 6 | **Dynamic ActionRegistry** — string-id actions, metadata, register/unregister, args; compat shim over `WmAction`; chrome placement actions (`chrome.container.move_*`) reachable from key/mouse/RPC | E1 | 🟡 now (touches `actions.rs`/`input.rs`; mild coord w/ #99 `keyboard.rs`) | delegated/Claude |
| **E7** | Ph 8 | **Host-owned overlay APIs** (modal/dropdown, focus trap, result-returning async) — grid-ui has the *widgets*; this is the host API | E4 + E6 | after E4 | Claude |
| **E8** | Ph 8.1/8.2/9/10/11 | placeholder tokens, config.toml simple plugins, **WASM runtime**, multi-region proofs, full keybinding/palette/RPC integration | E4/E6/E7 | far future | TBD |

**Key parallelization wins:**
- **E2 (grid-ui widgets)** and **E1 (contract)** and **E3 (chrome state)** and **E6 (dynamic actions)** are **all startable immediately** — different crates/modules, zero overlap with #99 or WS-A's render seam.
- **E6 (dynamic actions)** is the highest-leverage early win: it's independent of rendering, unblocks the chrome placement actions, and the static→dynamic `WmAction` migration is exactly the codebase change the graph flagged (the `ActionRegistry / WmAction Dispatch → Dynamic ActionRegistry` edge).

---

## 7. Sequencing (the dependency spine)

```
NOW (parallel, no #99 dependency):
  WS-D  appearance config              [delegated]
  WS-B  window transparency+vibrancy   [Claude/delegated]  (clear-color line waits for #99)
  WS-C  blur.rs renderer primitive     [Claude]            (app wiring waits for WS-A)
  E1    architecture contract doc      [Claude]
  E2    grid-ui chrome widgets         [delegated]
  E3    shared chrome state            [delegated/Claude]
  E6    dynamic ActionRegistry         [delegated/Claude]

GATE: #99 merges → render_frame reconciled + factored (content/chrome passes)

THEN (stacked on the merged render_frame):
  WS-A  chrome render integration (A0→A4)   [Claude]
  WS-B  clear-color/alpha finalize           [Claude]
  WS-C  app blur wiring (overlay frosting)    [Claude]
  E4    ChromeHost                            [Claude]      (needs E3)
  E5    WorkspacesContainerProvider migration [Claude]      (needs E4 + WS-A)
  E7    overlay host APIs                     [Claude]      (needs E4 + E6)

FUTURE:
  E8    tokens / config-plugins / WASM / proofs / palette+RPC integration
```

---

## 8. Conflict-avoidance rules (hard constraints for every agent)

1. **One owner per file.** The table in §1 is authoritative. If a WS needs another WS's file, raise it — don't dual-write.
2. **`render_frame` is serialized via the #99 stack.** Only WS-A and WS-B edit it, only after #99, only in their factored pass (`render_chrome` / clear-color). Never reshape `render_pane_content`.
3. **`heca-config/src/settings.rs`** — WS-D and #99 both add fields. Additive only; if conflict, rebase WS-D onto #99.
4. **grid-ui stays presentation-only.** Canonical state in `AppState`/chrome-state; widgets never own domain state (chrome-plugin §5.6).
5. **Geometry contract.** All new chrome/appearance APIs use `heca-core/src/layout/types.rs` (`Rectangle`/`Point`/`Size`). Do not propagate legacy `heca_core::types::Rect` (chrome-plugin §5.7).
6. **Appearance is read-only config** consumed by all renderers; the terminal (content) and chrome (grid-ui) read the same struct so opacity/blur stay consistent across the window.
7. **Dispatch-only mutation** (chrome-plugin §2.3): chrome/providers never mutate `AppState` directly — they dispatch actions (ties into E6).

---

## 9. Risks

- **#99 keeps reshaping `render_frame`.** Mitigation: the §0 factoring agreement + stacking WS-A on #99.
- **Backdrop blur is not portable / not a numeric radius.** Mitigation: be explicit in config UX (on/off + material for OS blur; numeric `blur_amount` only drives in-app WS-C). Linux/Wayland = graceful no-op.
- **In-app blur needs the app on the `Compositor`.** Mitigation: WS-A adopts the compositor (showcase-proven) before WS-C app wiring; WS-C renderer primitive is built independently in the meantime.
- **Doing the chrome-plugin architecture sidebar-first instead of chrome-wide** (the plan's headline risk). Mitigation: E1 contract before E4/E5; ChromeRegion is already region-generic.
- **Dynamic-action migration breaking existing keybinds.** Mitigation: E6 ships a compat shim over `WmAction` (string-id wrapper) before removing the static enum.

---

## 10. Immediate next actions (what to kick off the moment we decide)

- **Send the `render_frame` factoring request** to the #99 author (one line, §0).
- **Start now, in parallel (no #99 dependency):** WS-D (appearance config), E1 (contract doc), E2 (grid-ui chrome widgets — delegate), E6 (dynamic ActionRegistry).
- **Hold for #99 merge:** WS-A (chrome render), WS-B clear-color, WS-C app wiring, E4/E5/E7.
