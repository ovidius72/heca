Decided by Antonio, 2026-08-18, in discussion. This phase is the record.

═══ THE ARCHITECTURE ═══

    Scrolling area (generic container of panes, in splittable columns)
      └─ Pane  — letter, zoom/float/split/remove, and the bounds it was given
           └─ Flex
                ├─ header (fixed)      ← the CONTENT's, absent for Neovim
                └─ ScrollRegion        ← the CONTENT scrolls, not the pane
                     └─ Terminal

Antonio: "the only responsibility of the pane is to draw the key hint letter, zoom in/out,
float/unfloat, split/unsplit, remove". And: "Terminal should be a reusable component similar to a
DOM <terminal/> we can put in any container in the app, even in a plugin or in a sidebar."

All content components implement one common trait so any of them can sit in a pane; a plugin author
can build a pane and use these components.

═══ THE CODE, AS IT SHOULD READ ═══

The terminal's own composition — heca/src/chrome/terminal/mod.rs, the ONLY file here that may
touch AppState:

    Grid::new()
        .rows([Track::Auto, Track::Fr(1.0)])    // header sizes itself, body takes the rest
        .gap(Spacing::Sm)
        .cell(TerminalHeader { pane_id, info, buttons, shortcuts, cb }.build(), 1, 1, 1, 1)
        .cell(ScrollRegion::new()
                  .child(Terminal { pane_id, surface, cb }.build()), 1, 2, 1, 1)

THE TREE IS THE CONTENT'S, NOT THE PANE'S — so a content component composes whatever it needs:
`Flex`, `Grid`, `Surface`, `ScrollRegion`, any of them, nested to any depth. The pane does not care
and must not be told. The sample above is a `Grid` because THIS content has a header and a body —
two parts with different jobs, where a track template says the whole arrangement in one line
(`auto` for what sizes itself, `1fr` for what takes the rest). A `Flex` is right for a list of like
things, a `Surface` for something that is one box. Pick per case.

The one thing that is never right, whichever container is chosen: a parent that COUNTS ITS CHILDREN
to tell them apart. The pane holds `[header, content]` today and the host asks "does it have two
children?" to decide whether there is a header, so putting two things in the body makes the first
look like one. Full rule: docs/widgets.md -> Grid.

The pane shell — heca/src/chrome/pane/mod.rs. It composes the grid-ui `Pane` widget for its frame,
carries its own identity and letter, and holds exactly ONE content child:

    Pane::new()
        .border_style(appearance.effective_pane_border_style())
        .key(pane_key(id))                      // identity — never an index
        .hint_placement(HintPlacement::Center)   // the letter sits centred over the pane
        .on_hint(fires(focus_pane(id), emit))    // what picking it does
        .child(content)                          // Terminal, Browser, Neovim, Agent, a plugin's

FIVE SPELLING RULES, each one a real trap:
1. `Flex::column()` / `Flex::row()` — there is no `Flex::new(child)`. And never name a UI box
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
4. `Terminal` has NO title and NO icon. Those are the header's facts. A terminal takes what it needs
   to draw its content and nothing more — which is exactly what lets it mount in a sidebar, where
   there is no title.
5. The icon buttons are NOT hand-listed. They come from the dynamic vector `pane_header_buttons`,
   which is what makes config-added, config-hidden and future plugin-added buttons work with no
   change here. Do not re-introduce a per-action match in a render loop.

═══ THE FOLDER STRUCTURE ═══

Per AGENTS.md § 0b-bis. heca/src/chrome/expose/ is the worked example to copy — 7 files including
testing.rs (mod.rs 733, model.rs 282, expose_grid.rs 327, workspace_row.rs 374, column_card.rs 149,
pane_card.rs 433, testing.rs 119).

    heca/src/chrome/
      pane/
        mod.rs            host wiring — the ONLY file that may touch AppState
        model.rs          the session reduced to plain data (id, rect, active, zoomed, floating)
        pane.rs           the shell component: frame + key + letter + one content child
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
- Return the CONCRETE widget when the parent still has to size it; box only at the top.
- Sizes are shares. Never take a number out of the data model and multiply it by a scale of your
  own to make it fit: a column is 400 logical px in the session, and `Px(400.0 * zoom)` hands this
  component ownership of every term — the window, the margins, the padding, the gaps, the child
  count. Say the FRACTION instead (that column is 40% of the strip) and the engine solves it at
  every window size. `grow` is a share of what is LEFT OVER after the fixed children, so "a share of
  the widest sibling" is a percentage against one denominator, not a grow weight. And a child that
  simply fills its parent says NOTHING AT ALL — unset already stretches, like CSS.
  ⚠️ SPELLING: written `Length::Pct(0.4)` today. The sizing builders take `Length` by value, so the
  spellings `Length`'s own serde impl already reads — `"40%"`, `"200px"`, `"auto"`, a bare number —
  cannot reach them. They become `impl Into<Length>` (plus `Width::Full`/`Half`), after which a call
  site writes `.width("40%")` or `.width(200)` and `Length::Pct` stops appearing in app code. The
  wire already speaks these, so this is the native side catching up — no plugin change.
- One headless test per component, plus the box test: lay it out in a box and assert it never
  exceeds it.
- It moves to heca/src/components/ only when a SECOND surface needs it — a move, never a copy. That
  folder is an empty shell today (mod.rs only).

═══ THE WIDGETS THIS IS BUILT FROM (all documented in docs/widgets.md) ═══
- `Pane` (heca-grid-ui/src/widgets/pane.rs) — the frame, three modes via PaneFrame:
  frameless/bordered/bracketed. Width, radius and colour from the theme and [appearance.pane].
  ⚠️ DECIDED: all three modes are BORDER STYLES (none / a continuous line / corner brackets), so the
  builder is named for what it is — `.border_style(..)`, taking the config's `BorderStyle` directly.
  It is `.border_style` and not `.border` because `StyleExt::border(Color, width)` already exists;
  CSS separates `border-style` from colour and width for the same reason. `crate::chrome::
  apply_pane_frame` — a six-line host match translating the config enum into `.frameless()` /
  `.bordered()` / `.bracketed()` — DELETES: the library owns the mapping and no caller translates.
- `Flex` — the arrangement; `.gap`, `.padding`, `.grow`, `.align`. No StyleExt on it by design.
- ⚠️ DECIDED: `.child_boxed(..)` DELETES, here and in the fifteen other `*_boxed` setters across the
  library. `Box<dyn Component>` is not itself `Component`, which is the whole reason the twin
  exists; `.child(..)` takes `impl IntoComponent` covering both, and every `*_boxed` goes at once
  rather than one widget at a time. Native-only — `child_boxed` is realize's own seam and has no
  described name, so nothing on the wire changes.
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

═══ NOT TRUE YET — THE FILES THAT CHANGE ═══
- heca/src/app/terminal_render.rs (1700 lines) — paint_terminal_pane_shell:718 builds a grid-ui Pane
  fresh each frame and paints it with a DIRECT pane.paint(cx):783; the header is painted directly at
  :800; title_bar_reserve:626 / pane_title_top_inset:646 reserve the header's height on the pane;
  stable_tiled_content_rect:654 / stable_floating_content_rect:670 / pane_content_rect:903 compute
  the content rect by hand; sync_retained_terminal_layers:46, render_terminal_layer_update:399 and
  blit_retained_terminal_layer:294 are the GPU half that must be preserved.
- heca/src/app/render.rs — calls the shell from TWO sites, :691 (tiled) and :902 (floating); the
  48px host letter pass is at :951 (~43 lines, including a second loop for floats).
- heca/src/chrome/pane_header.rs (1086 lines) — sync_pane_headers:952 builds the retained per-pane
  bar; build_pane_info_bar:159; pane_info_view:76. state.pane_headers (app_state.rs:710) and
  state.pane_viewport_widgets (app_state.rs:725) are the two retained per-pane stores.
- heca/src/chrome/hint.rs — active_hint_targets:534 (the surface stack, layers → chrome → panes),
  offer_hint_letters:356, target_identity:321, fire_hint, clear_hint_letters. HintSurface::
  PaneHeader(pane_id) becomes the pane.
- heca/src/handlers.rs — handle_pane_select:776 (index-based, the defect), assign_letters:816 (pure,
  already correct, serves both pickers), handle_hint_pick:845.
- heca/src/app/selection.rs — collect_all_pane_candidates:41 is pure index; PANE_CANDIDATE_LIMIT:9
  and candidate_letter:17 already read DEFAULT_LETTERS.
- heca/src/mouse/hit_test.rs:17 — hand-written pane hit test with a hardcoded 40.0 sidebar width.
  mouse/ is 2644 lines across target.rs, drag.rs, release.rs, interactive.rs, resize.rs.
- heca-grid-ui/src/scene.rs — ⚠️ THE ENABLING GAP: DrawCommand is ONLY Rect / Brackets / Text /
  Scanline / PushClip / PopClip. AGENTS.md lists `Custom` and `Gradient` — BOTH ARE STALE, neither
  exists. Fix that line in AGENTS.md as part of this phase.
- No PaneKind enum anywhere. AGENTS.md specifies SpawnPane { kind, program, argv, float, width,
  height } with terminal|browser|nvim_gui as the target shape; nothing implements it.

═══ THE KEY DESIGN CALL ═══
Both content kinds collapse to ONE shape: the pane's content child is ALWAYS a component. For a
browser/agent/plugin it is a real tree (Flex, Grid, ScrollRegion). For the terminal it is a leaf
that paints one new scene command naming an externally-rendered surface + its rect; the renderer maps
that id to the texture it already blits. The library stays GPU-free — same split Text already has
(the scene says what, the renderer decides how; docs/widgets.md says exactly this about glyph glow).
Performance is preserved: retained terminal textures and damage tracking stay as they are. Drawing
terminal cells as ordinary scene commands is REJECTED — cell glyphs are the hottest path in the app.

═══ DOCS (mandatory, same change) ═══
docs/widgets.md gets the new scene command and the content trait, with a native AND a declarative
example for each; AGENTS.md gets the chrome/<kind>/ structure beside the existing expose example and
its stale DrawCommand line fixed. Do NOT create new doc files (Antonio, 2026-08-17).

═══ VERIFICATION ═══
Rendering, layering and input. Green tests are not verification: Antonio drives every visual check.

═══ RELATIONSHIP TO EXISTING WORK ═══
- P089(F011) "The pane header and the info bar come out of chrome/mod.rs" is SWALLOWED by this phase
  (its work becomes chrome/terminal/terminal_header.rs). Fold or mark superseded.
- P082(F003)/T447 "The pane owns its letter" is the letter half of T451 here. Decide whether T447
  ships first as a narrow fix or folds in.