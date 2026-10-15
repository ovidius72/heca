# P082(F003)/T474 — Column identity and authoring API

Full task description and completion history, preserved from the planner on 2026-10-12.
Antonio corrected the future capability to **Pinned Column**: whole columns only.

⚠️ THIS TASK NOW CARRIES TWO BODIES OF WORK. The authoring-API section immediately below is what is being done FIRST (Antonio, 2026-09-11); the column-identity work it was opened for — Parts A and B — starts further down and is unstarted.

⚠️ WHAT LANDED 2026-09-14/15 IS IN THIS TASK'S **NOTES**, and in full in
`.planner/docs/t474-library-capabilities-2026-09-15.md`. Four library capabilities beyond the
checklist: a space is one property; `.accent(Color)` overrides the accent for a widget and its
subtree; a container's letter no longer depends on how many children it holds; an exit animation now
ends. **Read the notes before Part A** — the second of those removed the blocker Part A reverted on
twice, and the trap list below is written as if it were still there.

═══════════ THE AUTHORING API — library-wide, and the current work ═══════════

WHY NOW, AND NOT LATER. P094(F011)'s next tasks (T449 Terminal-as-a-component, T450 the header) are NEW code. If the API lands first they get written right once; if it does not, they get written in the spellings being deleted and then swept. That is the whole reason this jumped the queue — it is not needed by Parts A/B and does not block them.

THE ORDER. Checklist items 1–6 are ADDITIVE to callers — `impl Into<..>`, `.child` taking a box, `.border_style` beside the existing builders, one spacing builder taking either kind — so nothing breaks and no call site is forced to change. Item 7 (the sweep) goes last and in its own commits, because a mechanical change across hundreds of sites mixed into a design commit is a diff nobody can review.

⚠️ THIS DESCRIPTION IS THE PLAN; THE CHECKLIST IS THE STATE. They disagreed once — this text said the wire must move while the code deliberately had not moved it — because one was updated and the other was not. Trust the checklist and the commits; correct this text when they diverge.

THE DECISIONS, with the reasoning and the plugin impact:

- SIZES — DONE (a73034e). `Length` already read `"200px"` / `"100%"` / `"auto"` and a bare number — its serde impl does exactly that — but `.width()` took `Length` by value, so none of it reached a call site and `Length::Px/Percent` is written by hand hundreds of times. The six sizing builders take `impl Into<Length>`; `Length::FULL/HALF/THIRD/QUARTER` are the named fractions. NO WIRE CHANGE: the described side already speaks these spellings, so this is the native side catching up.
  ⚠️ `Percent` is a FRACTION 0.0..=1.0, not 0-100. Half is `Percent(0.5)`. Renamed from `Pct` because the abbreviation hid the question.
  ⚠️ Unset ALREADY stretches, like CSS — so `.width(Length::FULL)` on a child that fills says nothing. Deleting those is a JUDGEMENT per call site (it depends on the parent), not part of the sweep, and Antonio drives it.

- CHILDREN — DONE (69e8e05). `Box<dyn Component>` is not itself `Component`, which is the only reason sixteen `*_boxed` twins existed. `.child(..)` takes `impl IntoComponent` covering both and they all went AT ONCE — one widget at a time is how a library ends up half-converted. Native-only: `child_boxed` was realize's own seam and had no described name.

- BORDER STYLE — DONE (350e0d8). `PaneFrame`'s three modes were all about the border — none, a continuous line, corner brackets — so the builder is named for what it is. `.border_style(..)` and NOT `.border(..)`, because `StyleExt::border(Color, width)` already exists; CSS separates `border-style` from colour and width for the same reason. It takes the theme's own `FrameStyle`, which the config's `BorderStyle` converts into — so `PaneFrame` was a THIRD copy of one vocabulary and is deleted, and `chrome::apply_pane_frame` with it. `Pane` is not a `WidgetKind`, so nothing on the wire changed.

- SPACING — DONE. Native half in dbbab0d; the rest on 2026-09-14. `.gap(f32)` and `.gap_spacing(Spacing)` wrote to two different fields, and `.padding(px)` sat beside `.pad_all/.pad_x/.pad_y(Spacing)` — two paths over one property, in the library itself. The counts show what that cost: `.gap` 112 uses against `.gap_spacing` 8, and `.padding` 19 against the three token builders' 11, while the docs said PREFER THE TOKEN. Advice loses to whichever name is shorter.
  `.gap` / `.padding` / `.padding_x` / `.padding_y` / the four per-side paddings / `.margin` / `.margin_x` / `.margin_y` all take a number, a `Spacing` step, or a string (`"8px"` / `"sm"`) through `style::Space`. The twins are deleted.
  ⚠️ MARGIN IS DONE TOO (Antonio, 2026-09-14). It had no token twin to merge, so it was skipped at first; he decided it should speak a space like the other two rather than leave one of the three answering a different question.
  ⚠️ THE AUTHORED STEP NOW SURVIVES LAYOUT. The old code resolved `gap_spacing`/`pad_spacing_*` into the px fields on every pass, which destroyed the authored value — a font, size-variant or zoom change then had nothing left to re-resolve. Pixels are produced only where the font is known (`Space::resolve`, the four padding-cascade accessors, `to_taffy`), and the rounding-to-whole-pixels rule moved there with it.

- THE WIRE — DONE (Antonio decided 2026-09-14). One name, every spelling: `"gap": 8` and `"gap": "sm"` both land, and so does `"padding": "md"`. The retired names — `gap_spacing`, `pad_spacing_x`, `pad_spacing_y` — are still READ, so no existing tree breaks; nothing writes them any more.
  HOW, and why there was only ever one shape: the described vocabulary IS the layout field list (docs/plugins.md § 8, "Layout properties are read from the layout type itself — no list in the app"). A property is accepted when its value fits that field's TYPE. So the fields themselves hold a `Space`, exactly as `width` holds a `Length` and has always read `"50%"`. No name table was added and none should be.
  The guard that asserted a string on `"padding"` never parses now says what is true: a step name lands, nonsense still degrades to nothing.
  ⚠️ The retired names are mapped in ONE place, in the merge that already resolves property names. That map is the cost of renaming a described field, and it should stay short.
  ⚠️ A guard on the SDK side pins the property name a builder writes. Without it nothing can catch the builder emitting the retired name, because the retired name still works — that guard passed with its own fix deleted until it was rewritten.

CONTAINERS, while touching this. A component composes whatever it needs — Flex, Grid, Surface, ScrollRegion, nested to any depth, picked per case. What is never right, whichever container is chosen, is a PARENT THAT COUNTS ITS CHILDREN to tell them apart: the pane holds `[header, content]` and the host asks "does it have two children?" to decide whether there is a header, so two things in the body make the first look like one. Documented in docs/widgets.md → Grid and Flex.
  ⚠️ The picker was doing exactly this and was fixed 2026-09-15 — see the notes. It decided "this node is a mere wrapper" by counting children, so a container's letter appeared and vanished with the number of things inside it.

NAMING. Never name a UI box `Column` — `Column` already means a column of panes in heca-core. That convention must be written in docs/widgets.md and AGENTS.md, not only in a phase description, because the obvious name for a vertical box is `Column` and the reason it is taken has to sit in front of whoever reaches for it.

GATES. `./scripts/lint-changed.sh test`, then `./scripts/lint-changed.sh`. Never `--workspace`. Grep that output for `warning:` as well as `error` — a warning was missed by grepping for errors only. Per-file rustfmt count must not rise; a longer name pushes lines over the limit, which happened six times across this work. Sabotage every new guard: across this work three passed with their own fix deleted, and one did not exist until a sabotage passed.

═══════════ PART A — the column owns its identity (UNSTARTED) ═══════════

Found by Antonio driving T438, 2026-08-24: "prefix+shift+c works but show letters only in the sidebar. not in the scrolling area. not sure this is what we deciced." It is not. It is the ownership inversion already decided against, still open for columns.

THE RULE, ALREADY SETTLED — P082(F003)/T447: "WHOEVER OWNS THE THING DECLARES ITS IDENTITY; anything else displaying it is a view. A pane owns 'pane:7'. A sidebar row showing that pane is a second view of it, not the owner."

That was fixed FOR PANES by P094(F011)/T451: the pane shell carries `.key(pane_key(id))`, `pane_key` was given ONE home in heca/src/chrome/mod.rs, and the sidebar row became a view of it.

STATUS — HALF LANDED. `heca/src/chrome/column/` exists: the column in the scrolling area owns `col:<id>`, declares what picking it means, and wears its letter (6ab3058, 9bd6e8d, 518d4ba). What remains is THE COLUMN BECOMING THE PARENT OF ITS PANES — chosen over a sibling shell because a pane should move between containers like a div, with **Pinned Column** support later. The user chooses a whole column to keep visible while the other columns scroll. Its panes stay inside that column; independent sticky or pinned panes are not part of this feature.

⚠️ THE PARENT MOVE WAS TRIED ONCE AND REVERTED. It was done as one lump — ownership + sizing + placement + painting + focus — in the one part of the app no test can see; 1765 tests stayed green through every defect. FOUR TRAPS:
1. Flex stacking the panes DROPS THE GAP between them, so every pane below the first is drawn higher than its terminal content and its header sits over the text. The WM owns pane geometry: place children at the slots `columns_with_positions` gives.
2. Moving placement to the column skips the per-frame `shell::focus_state_to` write, so THE ACTIVE PANE LOSES ITS HIGHLIGHT. Nothing else writes it.
3. A pane's key sits on the `Pane` INSIDE its `KeyHint` wrapper, so a naive `base().key` lookup on the column's child finds nothing — every tiled pane looks unbuilt and is REBUILT EVERY FRAME (100% CPU idle). Resolve through the wrapper.
   ⚠️ 2026-09-14: this wrapper is gone from the SIDEBAR's rows — a `DockFrame`, a `MarkerGroup` and a `Row` are widgets and now declare their own pick — but the PANE shell is still `KeyHint::new(pane)`, so the trap stands here.
4. ✅ **TRAP 4 IS GONE (2026-09-15).** It read: panes are painted individually under a per-pane theme, so a column painting its children either flattens them to one accent or double-paints them. The cause was that a pane re-tinted its contents by being painted under a COPY OF THE THEME with the accent swapped — which is what forced one paint call per pane, and therefore what stopped any container from painting its own children. A pane publishes its hue on itself now (`.accent`), so painting is the same walk for every widget and a column may own and paint its panes. See the notes and `.planner/docs/t474-library-capabilities-2026-09-15.md` § 2.

⚠️ OWNERSHIP AND PAINTING CANNOT BE SEPARATED — the column's `paint_child` walks its children, so the moment panes are children they are painted twice. A first attempt at "ownership only, painting unchanged" was wrong for this reason, and its replacement — a host lookup function `pane::shell(state, id)` — was a registry by another name and was reverted too. With trap 4 gone this is no longer a blocker but it is still the shape of the work: ownership and painting move together, in steps Antonio drives one at a time (four commits after the first revert, each verified before the next).

Also asked for, not started: a "new column" PLACEHOLDER — while the column pick is open, a slim empty slot right of the current column wearing its own letter, gone when the pick closes. The same affordance a drag insertion marker needs later; `move_pane_to_new_column` already exists to run it.

═══════════ PART B — move the current pane to a NEW column (DONE) ═══════════
Antonio, 2026-08-24: "A new action to move the current pane to a new column."

Landed in 10bcdf8 as `move_pane_to_new_column` on `prefix+Shift+Enter`, with `act_refusal` for a key that cannot act. Decided while building it: a new column goes RIGHT OF THE CURRENT ONE, not at the end — "you keep your place in the strip"; and a pane alone in its column is NOT OFFERED a new column at all, rather than refused afterwards, the same rule that excludes the current column from the pick.

═══════════ VERIFICATION ═══════════
The authoring-API work is mechanical and test-covered: green tests, no newly unformatted line, and every guard run red against its own bug. Nothing should change on screen, which is why none of it is visually verified.

Verified on screen by Antonio 2026-09-14/15: panes look unchanged after the accent move; the workspaces dock's letter is consistent at one, two and three workspaces and no longer appears in `prefix+/`. NOT yet verified: the exposé no longer being lettered after it is closed.

Parts A/B are RENDERING. Green tests do not show where a letter is, and every defect in T451 was found by eye against a green suite. Antonio drives:
- `prefix+Ctrl+c` with two or more columns letters the scrolling area AND the sidebar; the typed letter moves the active pane into that column; the letters withdraw together; check with the sidebar shown and hidden.
- Watch for the same-letter-drawn-twice question — correct if the two are one target seen in two places, but confirm by eye.
- The parent move: panes keep their gaps, the active pane keeps its highlight, nothing rebuilds every frame (watch idle CPU), and no pane is painted twice.

⚠️ 45 commits sit on `feat/hint-collapses-to-the-tree` and are NOT PUSHED, plus a large uncommitted working tree. Pushing was never discussed; ask before doing it.

---
**Completion summary:**
The authoring-API half is COMPLETE: checklist 8 of 8. Sizes, children, border style, spacing (native and wire), the sweep and the docs. Four library capabilities landed with it — a space is one property, .accent overrides the accent for a widget and its subtree, the picker stopped counting children to decide what a node is, and an exit animation now ends. The column parent move, which this task was opened for, is still unstarted — but trap 4 is gone, because .accent removed the per-pane theme swap that forced one paint call per pane.

**Verification:**
1825 tests pass; clippy clean at the one pre-existing toast/stack.rs warning; no file left worse formatted than HEAD. Every new guard run red against its own bug by sabotage — except a_card_is_one_pick_target_not_two, which passes but could not be falsified and should be treated as unproven. Verified on screen by Antonio: panes unchanged after the accent move; the dock's letter consistent at one, two and three workspaces and absent from prefix+/; the exposé no longer lettered once closed. NOT yet checked on screen: one letter per card under the exposé's own s picker.

**Remaining or unverified:**
The column parent move (three traps left). Six KeyHint wrappers still in the app, candidates for the same fix the exposé card got. Per-widget pickable defaults, untouched. The single dock letter under prefix+Shift+e when two mounts exist. The layer record's second visible flag, still unsettled after an animated close.

**Files touched:**
- heca-grid-ui/src/style.rs
- heca-grid-ui/src/builders.rs
- heca-grid-ui/src/component.rs
- heca-grid-ui/src/layout.rs
- heca-grid-ui/src/animation/presence.rs
- heca-grid-ui/src/hint/collect.rs
- heca-grid-ui/src/hint/offer.rs
- heca-grid-ui/src/hint/mod.rs
- heca-grid-ui/src/hint/tests.rs
- heca-grid-ui/src/widgets/key_hint_group.rs
- heca-grid-ui/tests/phase_a.rs
- heca-view/src/lib.rs
- heca-view/src/build.rs
- heca-view-realize/src/lib.rs
- heca/src/chrome/pane/shell.rs
- heca/src/chrome/scene.rs
- heca/src/chrome/mod.rs
- heca/src/chrome/expose/pane_card.rs
- heca/src/app/terminal_render.rs
- heca/src/app/render.rs
- docs/widgets.md
- docs/plugins.md
- docs/hint-architecture.md

---
**Completion summary:**
DONE 2026-09-23, every part confirmed on screen by Antonio.

═══ WHAT SHIPPED ═══

**The authoring API** (checklist 8/8, done 2026-09-14/15): sizes, children, border style, spacing native and on the wire, the call-site sweep, the docs. Four library capabilities came with it — a space is one property, `.accent` overrides the accent for a widget and its subtree, the picker stopped counting children to decide what a node is, an exit animation now ends.

**Part B** (done earlier): `move_pane_to_new_column` on `prefix+Shift+Enter`.

**Part A — the column holds its panes** (ca9ea2b). A pane is a child of its column, placed with `at_rect` at the rect the layout engine gave it, so the space the WM leaves between panes survives; stacking them would close it and draw each pane over its own text. Children are reconciled, so a pane still there keeps its widget — its letter, a gesture in flight, an animation. `sync_panes` keeps only floating panes, which belong to no column. One place says where a pane's tree lives: `chrome::pane_roots` for the walks, `hint_surface_root` for a surface.

**Naming** (ca9ea2b). A key says WHICH THING, so one thing has one name wherever it is drawn: the pane in the scrolling area, the sidebar row and the exposé card are three views of one pane and wear one letter between them. A control is `pane:7-zoom`, not `zoom`. `target_identity` stops prefixing by the tree a target was found in; that prefix remains only for a widget with no key.

**The new-column offer** (a243f307), the last piece of Part A. A slim lettered slot right of the column the picked pane is in, gone when the pick closes. A destination is named rather than signalled by a magic id. It comes last so no existing letter moves. A pane alone in its column is not offered one — refused before the letter, not after.

**Carried out of the leftovers, all confirmed:** a placement is named and a clash warns (4f1661b1); `prefix_timeout_ms` is configurable and its deadline is booked (e24e3579); a warning fails the lint gate (28c0cc67).

═══ SUPPORTING WORK, IN ITS OWN COMMITS ═══
Keyed child reconciliation in the library (1e9775d) — `ToastStack` had written it by hand and the column needed the same. A derived name counts the named things, not what is inside them (dcbd6fb) — three identical rows were `×`, `×[2]`, `×[4]`, and adding a label inside the first renamed the other two. The dock caps and scrolls (af03262). Frame requests coalesce (a2f3326). The workspace formatted (7231e2db).

═══ VERIFICATION ═══
1883 tests, clippy clean, formatting clean, all three gates green — and the gate now fails on a warning rather than printing it.

Confirmed on screen by Antonio: panes keep their gaps; the active pane keeps its highlight; `prefix+/` letters the panes; `prefix+Ctrl+c` letters the columns and the typed letter moves the pane; a pane shows the same letter in the sidebar and the scrolling area while zoom/float/close differ per pane; the new-column slot appears and works; `prefix+Shift+e` letters all three dock placements and each focuses its own; idle CPU back to normal.

Every guard was broken on purpose and seen to fail before being put back.

═══ NOT DONE, AND WHY ═══
**Per-widget pickable defaults.** No symptom behind it, and it reverses a settled rule: `hint/collect.rs:111` says being pickable is NOT opt-in, because requiring a declaration made the picker show a curated handful and feel not worth having. Left alone deliberately.

⚠️ THE HANDOFF'S CPU HYPOTHESIS WAS WRONG — do not re-derive it. It said a layout pass that never settles. Measuring showed 0 frames drawn while spinning, so nothing was laying out. The cause was every widget posting its own frame request; see the notes.

═══ FILES ═══
heca/src/chrome/{column/*, pane/*, mod.rs, scene.rs, host.rs, identity.rs, focus.rs, pane_header.rs, hint/*}, heca/src/app/{lifecycle.rs, input.rs, selection.rs, startup.rs}, heca/src/{app_state.rs, handlers.rs}, heca-grid-ui/src/{reconcile.rs, nav.rs, builders.rs, layout.rs, style.rs, component.rs, widgets/dock_frame.rs}, heca-config/src/keys.rs, scripts/lint-changed.sh, keybindings.default.toml, docs/keybindings.md, README.md, AGENTS.md.
