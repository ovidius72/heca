Decided by Antonio, 2026-08-18; amended with the approved 2026-10-24/25 decisions. This document is the current phase contract.

Current approved ownership (2026-10-25): P067/T517 owns generic parts/templates before P067/T398, which owns the global public API/registry and standalone native Terminal::new("shell")/binding. T450 owns content-composed optional terminal headers; T452 consumes the foundation for open managed creation/lifecycle/restore. T453/T455 audit current float/input geometry, T454 proves a second content and T457 converges the picker. T550 moved intact to P106/F011, dependent on T452. Completed T448/T449/T451/T456 stay done; T360/F012/P022 ownership and I009 IDEA-only status are preserved. The open-content phase decision remains 502849a5-af36-4331-8788-249dd6629c28. Cold-start: P067 and P094 canonical verified handoffs.

═══ THE ARCHITECTURE ═══

    Scrolling area (generic container of panes, in splittable columns)
      └─ Pane  — letter, zoom/float/split/remove, and the bounds it was given
           └─ Flex
                ├─ header (fixed)      ← the CONTENT's, absent for Neovim
                └─ Terminal            ← owns its viewport, controls and scrollback

Antonio: "the only responsibility of the pane is to draw the key hint letter, zoom in/out,
float/unfloat, split/unsplit, remove". And: "Terminal should be a reusable component similar to a
DOM <terminal/> we can put in any container in the app, even in a plugin or in a sidebar."

Content uses the existing Component contract and ordinary Parent/IntoChildren composition. Native authors place arbitrary Component values directly, including several children or a nested layout. Sandbox plugins compose typed descriptions of host-exposed primitives; they do not pass a native Component implementation across the boundary. Plugin identity handles are acceptable, while native construction uses ordinary constructors. Do not invent a pane-specific replacement widget trait or require arbitrary UI content to implement a terminal backend. Inspect the current lifecycle/restoration seams before adding a small missing contract.

═══ THE CODE, AS IT SHOULD READ ═══

The target authoring shape uses ordinary constructors and children. TerminalHeader is the component T450 extracts; `header` below stands for that component's concrete built widget, not a new public type imposed on callers. T449's Terminal already owns its viewport/scrollback controls, so do not wrap it in a second ScrollRegion:

    Pane::new().child(Terminal::new("shell"))

    Pane::new().child(
        Grid::new()
            .template_row("auto 1fr")
            .template_area(["header", "body"])
            .child(header.area("header"))
            .child(Terminal::new("shell").area("body"))
    )

    Pane::new().child(component_a).child(component_b)

    // Arrays/Vec may still contain a common element type.

These are the plain authoring requirements. Standalone constructors/binding belong to P067/T398; managed content creation/restoration is T452 work; the presence of these constructors alone is not proof that columns already host arbitrary content.

THE TREE IS THE CONTENT'S, NOT THE PANE'S — so a content component composes whatever it needs:
`Flex`, `Grid`, `Surface`, `ScrollRegion`, any of them, nested to any depth. The pane does not care
and must not be told. The sample above is a `Grid` because THIS content has a header and a body —
two parts with different jobs, where a track template says the whole arrangement in one line
(`auto` for what sizes itself, `1fr` for what takes the rest). A `Flex` is right for a list of like
things, a `Surface` for something that is one box. Pick per case.

The frame must not count children to infer their roles. That was the earlier defect; current PaneShell already uses named Grid areas but still owns a special optional header. T450 removes that ownership/reservation, not a now-absent child-count branch. Full rule: docs/widgets.md -> Grid.

The pane shell — heca/src/chrome/panes/pane/mod.rs. It composes the grid-ui `Pane` widget for its frame,
carries its own identity and letter, and accepts ordinary one-or-many children or a nested subtree:

    Pane::new()
        .border_style(appearance.effective_pane_border_style())
        .key(pane_key(id))                      // identity — never an index
        .hint_placement(HintPlacement::Center)   // the letter sits centred over the pane
        .on_hint(fires(focus_pane(id), emit))    // what picking it does
        .child(content)                          // Terminal, Browser, Neovim, Agent, a plugin's

FIVE SPELLING RULES, each one a real trap:
1. Use the actual fluent Flex/Grid APIs documented in docs/layout.md; a child is supplied through `.child(..)`. Never name a UI box
   `Column`: `Column` already means a column of panes in heca-core.
   ⚠️ THIS NAMING CONVENTION MUST BE WELL DOCUMENTED — in docs/widgets.md and in AGENTS.md, not
   only here. It is re-derived and re-proposed every time it is not written where a reader looks:
   the obvious name for a vertical box is `Column`, so the reason it is forbidden has to be in front
   of whoever reaches for it.
2. **Spacing is ONE builder that takes either.** `.gap(..)` accepts a number (px) or a `Spacing`
   token, and so do `.padding(..)` / `.margin(..)`. Prefer the TOKEN: it is resolved from the
   inherited font at layout, so it scales with the font, the size variant and UI zoom, while a raw
   px gap is tuned for one font size and wrong at every other — use a number only when you can say
   why it should not move with the font.
   ⚠️ DECIDED — the twin builders go. `.gap_spacing(Spacing)` beside `.gap(f32)`, and
   `.pad_all/.pad_x/.pad_y(Spacing)` beside `.padding(px)`, are two builders for one property, which
   is the "never a second path" rule broken in the library itself. They collapse into one that takes
   `impl Into<Spacing>`. ⚠️ THE WIRE MOVES WITH THEM: `"gap"` and `"gap_spacing"` are BOTH described
   props (heca-view/src/build.rs), so `"gap"` must accept a number or a token name and
   `"gap_spacing"` stays only as an alias — merging the native builders alone would leave plugins on
   the old pair.

3. Properties are STRUCT FIELDS and the constructor is a struct literal (AGENTS.md § 0b-bis rule 2).
   Not `Header::new(title, icon, buttons, active, cb)` — providers/workspaces/pane_card() at eleven
   positional arguments is the counter-example nobody can read.
4. Terminal content owns any optional header/display metadata; Pane has no special title/header reservation. T450 retains main's terminal-title display/rename requirement, with its concrete interaction reviewed before implementation. A headerless terminal remains valid in a sidebar or arbitrary composition.
5. The icon buttons are NOT hand-listed. They come from the dynamic vector `pane_header_buttons`,
   which is what makes config-added, config-hidden and future plugin-added buttons work with no
   change here. Do not re-introduce a per-action match in a render loop.

═══ THE FOLDER STRUCTURE ═══

Per AGENTS.md § 0b-bis. heca/src/chrome/expose/ is the worked example to copy — 7 files including
testing.rs (mod.rs 733, model.rs 282, expose_grid.rs 327, workspace_row.rs 374, column_card.rs 149,
pane_card.rs 433, testing.rs 119).

    heca/src/chrome/panes/
      pane/
        mod.rs            host wiring — the ONLY file that may touch AppState
        model.rs          the session reduced to plain data (id, rect, active, zoomed, floating)
        pane.rs           the shell component: frame + key + letter + ordinary component children
        testing.rs        #[cfg(test)] fixtures
      terminal/
        mod.rs            host wiring — the ONLY file that may touch AppState
        model.rs          PaneInfoView and friends, reduced from the session
        terminal.rs       the <Terminal/> component
        terminal_header.rs the info bar: content segments + pane-action buttons
        testing.rs
      browser/            later — same shape
      neovim/             later — same shape, and NO header
      agent/              later — same shape

RULES THAT COME WITH THE SHAPE:
- mod.rs is the only file that may see AppState. Everything below it takes plain data and is
  testable headless.
- The seams travel as ONE group — a single callbacks struct built once and passed by reference, not
  threaded one argument at a time (expose::ExposeCallbacks is the pattern).
- Return concrete/ordinary components or an ergonomic opaque component. Public authoring parameters and returns must not expose Box/casts or require storage conversions; internal allocation/type erasure is a framework detail.
- Sizes are shares. Never take a number out of the data model and multiply it by a scale of your
  own to make it fit: a column is 400 logical px in the session, and `Px(400.0 * zoom)` hands this
  component ownership of every term — the window, the margins, the padding, the gaps, the child
  count. Say the FRACTION instead (that column is 40% of the strip) and the engine solves it at
  every window size. `grow` is a share of what is LEFT OVER after the fixed children, so "a share of
  the widest sibling" is a percentage against one denominator, not a grow weight. And a child that
  simply fills its parent says NOTHING AT ALL — unset already stretches, like CSS.
  Use the current sizing builders and CSS spellings from docs/layout.md (`.width("40%")`, numbers and semantic tokens). T474's completed authoring work is reused, not planned again.
- One headless test per component, plus the box test: lay it out in a box and assert it never
  exceeds it.
- It moves to heca/src/components/ only when a SECOND surface needs it — a move, never a copy. That
  rule is about actual second callers, not predicted sharing.

═══ THE WIDGETS THIS IS BUILT FROM (all documented in docs/widgets.md) ═══
- `Pane` (heca-grid-ui/src/widgets/pane/mod.rs) — the frame, three modes via PaneFrame:
  frameless/bordered/bracketed. Width, radius and colour from the theme and [appearance.pane].
  ⚠️ DECIDED: all three modes are BORDER STYLES (none / a continuous line / corner brackets), so the
  builder is named for what it is — `.border_style(..)`, taking the config's `BorderStyle` directly.
  It is `.border_style` and not `.border` because `StyleExt::border(Color, width)` already exists;
  CSS separates `border-style` from colour and width for the same reason. `crate::chrome::
  apply_pane_frame` — a six-line host match translating the config enum into `.frameless()` /
  `.bordered()` / `.bracketed()` — DELETES: the library owns the mapping and no caller translates.
- `Flex` — the arrangement; `.gap`, `.padding`, `.grow`, `.align`. No StyleExt on it by design.
- T474 removed the public `*_boxed` twin builders. Reuse the ordinary `.child(..)` and slot APIs. P067/T398 removes the remaining public Box-based parameter/return exposure and WidgetKind dispatch, rather than recreating the SDK here. Internal storage may still allocate or erase types.
- `ScrollRegion` — the viewport the content scrolls inside. Owns wheel, thumb drag, click-in-track
  paging, ensure_visible and the eight Scroll* WidgetIntents. Binds keyboard_target so only the
  focused region answers scroll keys.
- `IconButton` (+ `WidgetSize::Header`) and `Tooltip` — the header buttons; tip and shortcut derived
  from the action NAME via action_tooltip + ActionShortcuts, never a literal shortcut string.
- `Tag` (multi-segment), `Icon`, `Label`, `BadgeButton` — the header's content and the "N lines
  above" chip.
- The hint machinery: `Base::hint` / `hint_label` / `hint_style`, `HintPlacement::Center`,
  `component::paint_child` (which DRAWS the letter — a widget painted directly can never show one),
  `collect_hints` / `fire_hint` / `offer_hint`, `DEFAULT_LETTERS` (52, home row first),
  `KeyHintGroup` for a surface-scoped picker, and `paint_keycap`/`keycap_size` as the only sanctioned
  way to stamp a cap over something that is not a widget.
- `ComponentExt` — `.key(..)`, `.hintable(bool)`, and every handler. `Base::activatable` is what makes
  something pickable; `.hintable(false)` opts out.
- `FocusScope` — if a pane's content needs to be a keyboard area rather than a control.
- `Spacing` / `WidgetSize` tokens — the semantic sizes; the caller picks a variant, the widget owns
  the pixels.

═══ WHAT THE PANE DOES NOT DO ═══
It never computes its own geometry. Zoom, float, split, remove are performed by heca-core/src/layout/
(Session / ScrollingSpace / Column) via handlers in heca/src/handlers.rs reached through
ActionRegistry. The pane ASKS (dispatches the action), the core DOES, the renderer draws the new
rects. taffy never positions panes (AGENTS.md).

Buttons in the header simply dispatch actions, so they are not coupling — the same action is
reachable from a keybinding, the command palette and RPC. That is the documented chrome-button
pattern (name the action, never the styling).

═══ ALREADY TRUE (verified 2026-08-18) ═══
- heca-core's Pane (heca-core/src/layout/column.rs:392) is content-blind: id, title, custom_name,
  runtime, close_policy, preferred_height, animations. Its doc already says it "wraps the actual
  content (terminal, neovim, browser)".
- ScrollingSpace / Column (heca-core/src/layout/scrolling.rs, column.rs) know nothing about terminals.
- PaneBackend (heca-core/src/backend/mod.rs:187) is the process/IO seam, with terminal + fake impls.
- config already splits the info bar along this exact line (heca-config/src/appearance.rs):
  PaneSegment (Location/AppName/GitBranch/GitStatus) = content facts; PaneAction
  (Split/MoveLeft/MoveRight/Close) = pane actions.

═══ CURRENT MAIN AND REMAINING OWNERS — VERIFIED 2026-10-25, 6e3e6d36 ═══
- chrome/panes/workspace/gather.rs still creates every managed pane with terminal::view_of. The column/floating content path is still terminal-specific; open creation and restoration belong to T452. Browser/Editor are future examples, not an exhaustive list or new browser/editor implementations required here.
- chrome/panes/pane/shell/mod.rs composes an optional separate header above content. T450 removes this special pane-header slot/reservation and makes the terminal content compose its optional header with ordinary layout. Headerless content and several body children are valid.
- chrome/panes/pane_header/inputs.rs still reads host pane width; segments.rs already carries HeaderSegment { name, chip, fit } and composes Label::truncate plus Tag segment fitting. Character estimates/truncate_path_left are removed. T450 preserves that delivered fitting and live action/in-place refresh behavior while moving ownership.
- app/terminal_render/ and chrome/panes/terminal/ already implement the reusable retained terminal surface; preserve completed T449 rendering, process lifetime, focus/input/search/link/selection behavior and scene-order drawing. Do not rebuild this implementation or add another ScrollRegion around it.
- T453 retains shared-geometry/Search review; Search.terminal is currently read by keyboard_terminal/search and is not dead by assertion. T455 still migrates hint/targets.rs's pane_outer_frames occlusion walk, although the mouse hit-test helpers are gone. Their task/checklist evidence, not the older census, is current authority.
- T452 follows the accepted open-content decision: no closed PaneKind enum/exhaustive Terminal-Browser-Editor switch; serialized factory identifiers are distinct from ordinary placement. Reuse the P067/T398 registry and the settled server/session authority.
- T088 is historically done at Antonio's request. The generic Tabs widget is only a selector; multiple selectable content views/documents within a panel are not implemented by that selector. The explicit tab work is now P106(F011)/T550, moved from P094 with the requirement and its T452 dependency; it is not an implicit P094 constructor task.

═══ THE KEY DESIGN CALL ═══
Every content implementation is ordinary component composition: one-or-many children or a subtree, without a closed list of content kinds. For a
browser/agent/plugin it is a real tree (Flex, Grid, ScrollRegion). For the terminal it is a leaf
that paints one new scene command naming an externally-rendered surface + its rect; the renderer maps
that id to the texture it already blits. The library stays GPU-free — same split Text already has
(the scene says what, the renderer decides how; docs/widgets.md says exactly this about glyph glow).
Performance is preserved: retained terminal textures and damage tracking stay as they are. Drawing
terminal cells as ordinary scene commands is REJECTED — cell glyphs are the hottest path in the app.

═══ DOCS (mandatory, same change) ═══
docs/widgets.md records the actual scene/composition/lifecycle seams, with a native AND a declarative
example for each; AGENTS.md gets the chrome/<kind>/ structure beside the existing expose example and
its stale DrawCommand line fixed. Do NOT create new doc files (Antonio, 2026-08-17).

═══ VERIFICATION ═══
Rendering, layering and input. Green tests are not verification: Antonio drives every visual check.

═══ RELATIONSHIP TO EXISTING WORK ═══
- P089(F011) "The pane header and the info bar come out of chrome/mod.rs" is SWALLOWED by this phase
  (its work becomes chrome/terminal/terminal_header.rs). Fold or mark superseded.
- P082(F003)/T447 "The pane owns its letter" is the letter half of T451 here. Decide whether T447
  ships first as a narrow fix or folds in.