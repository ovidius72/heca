# P094(F011)/T457 — one picker, not seven

## Current state (verified 2026-11-24 on main; review approved by Antonio)

The defect below still stands, with these corrections to the file references:

- `InputMode` now lives in heca/src/app_state/input_mode.rs. Its pick variants today: `PaneSelect`, `PaneSwap`, `PaneTake`, `WorkspacePick`, `ColumnPick` (:153), `DockPick` (:172), `HintPick`. `FollowLink` is no longer one of them. `awaits_pick_letter` is at :223.
- The handlers are in heca/src/app/input/pick_modes.rs: `handle_pane_select_mode` :13, `handle_hint_pick_mode` :54, `handle_pane_swap_mode` :75, `handle_pane_take_mode` :126, `handle_workspace_pick_mode` :156, `handle_column_pick_mode` :232, `handle_dock_pick_mode` :261 — still one copy of "match the letter, end the mode" each.
- Candidates: `collect_all_pane_candidates` / `collect_workspace_candidates` / `collect_column_candidates` (heca/src/app/selection.rs:49/71/99).
- Letters: the per-widget hint signals are gone; heca/src/chrome/hint/ offers letters by ownership. Two doors remain: `offer_hint` by path and `offer_hint_by_key` by key (heca-grid-ui/src/hint/offer.rs:86/130; 5 by-key call sites in the app), with `wanted(mode)` (heca/src/chrome/hint/letters.rs:62) mapping each mode to its offers and `offer_in_every_tree` (:172).

## New since the plan was written — use it

- `List` (heca-grid-ui widgets/list: `render_item`, `selected`, `on_choose`, virtualized) and palette lists open to extensions (`ext.palette_list(..)`, heca/src/chrome/surfaces/palette/lists.rs) — both from P087(F011)/T469. A pick that is a searchable list (workspaces, docks) can be a palette list rather than a letter mode; a pick of on-screen targets (panes, columns, sidebar rows) stays a KeyHintGroup.
- `.open(signal | "action")` on KeyHintGroup (T469) replaces the `open_when` written below.
- P045(F006)/T145 (pick a column as the subject of swap/move) must be built on this picker, not as an eighth mode.

Decide with Antonio before building which picks become palette lists and which stay letters.

---

## The plan (2026-08-18) — still the design

STEP 8 of P094. Decided with Antonio, 2026-08-18, from the question that exposed it: "I see hardcoded
InputMode. What if a plugin want to show and select some other entity? How do they do?"

They cannot. That is the defect.

═══ THE DEFECT ═══

`InputMode` (heca/src/app_state.rs) is a CLOSED ENUM IN THE CORE, and its pick variants NAME THE
ENTITY TYPE:

    InputMode::PaneSelect  { candidates: Vec<(char, PaneId)> }
    InputMode::PaneSwap    { candidates: Vec<(char, PaneId)>, focus_after: bool }
    InputMode::PaneTake    { candidates: Vec<(char, PaneId)>, focus_after: bool }
    InputMode::WorkspacePick { candidates: Vec<(char, usize)>, target: … }
    InputMode::ColumnPick  { candidates: Vec<(char, usize, usize)> }
    InputMode::FollowLink  { candidates: Vec<LinkHint> }
    InputMode::HintPick    { candidates: Vec<(char, HintTarget)> }

A plugin cannot add a variant, cannot build any of those candidate lists, and so cannot have the host
letter its containers, its files, its git hunks. **There is no line a plugin author could write.**
⭐⭐ RULE ZERO. No filter or argument fixes it — the shape is closed by construction.

Each variant also drags its own handler (handle_pane_select_mode, handle_pane_swap_mode,
handle_pane_take_mode, handle_follow_link_mode, handle_hint_pick_mode, + the workspace/column ones,
all in heca/src/app/input.rs), each re-implementing "match the typed letter, end the mode". Seven
copies of one rule — which is how the modifier guard came to be missing from four of them
(F011/P094/T451, 2026-08-18).

═══ THE ANSWER: A PICKER IS A WIDGET, AND IT ALREADY EXISTS ═══

    let open = signal(false);                 // flipped from an action
    KeyHintGroup::new(my_subtree).open_when(open)

That is the whole thing. The group holds focus while open — and holding focus IS taking the keyboard,
so there is no mode to enter, no gate to write and nothing the host must be told. It letters its own
descendants, claims the typed letter in the CAPTURE phase before the subtree sees it, and runs the
picked widget's own declaration. **The entity is never named in the core, because the picker letters
WIDGETS, not entities** — a plugin's Docker container is just a row that declared what picking it
does.

The exposé already works exactly this way and proves it: it declares `heca.expose.pick`, config binds
`[[keys.surface]] pick = "s"`, and `s` letters its cards only. There is no `InputMode::ExposePick`.

So `InputMode`'s pick variants are the LEGACY: they predate KeyHintGroup. The destination is that
every picker is a KeyHintGroup — a component's, a plugin's, and the core's, where `prefix+/` is that
widget at surface scope and `prefix+q` is the same with an eligibility filter.
docs/hint-architecture.md § 2c already says this in as many words.

═══ WHAT `prefix+q` VS `prefix+/` COSTS TODAY ═══
They are two implementations of one gesture, differing at every step:

| | prefix+/ (HintPick) | prefix+q (PaneSelect) |
| candidate is | a WIDGET (HintTarget{surface,path}) | a DOMAIN ID (PaneId) |
| targets found by | collect_hints, walking the laid-out tree | collect_all_pane_candidates, walking the SESSION |
| letter delivered by | offer_hint(root, &path, label) — BY PATH | offer_hint_by_key(root, key, label) — BY KEY STRING |
| what a pick does | fire_hint runs THE WIDGET'S declaration | the handler dispatches WmAction::FocusPane itself (input.rs:565) |

Row 3 is why the sidebar keycap is right-aligned under `prefix+/` and centred under `prefix+q`
(Antonio, driving, 2026-08-18): by-path lands on the widget that DECLARED the pick (the row's KeyHint
wrapper, carrying CenterRight); by-key lands on whatever node carries that key, with the default
placement. Same letter, two widgets, two looks. Neither placement is wrong.
Row 4 is why a plugin's pane could never join `prefix+q` however correctly it used the API: it is
never asked.

═══ WHAT TO BUILD ═══
1. `prefix+q` becomes the hint picker with an eligibility filter, not a mode. Its letters already come
   from the shared `assign_letters` (T451); this is the rest.
2. The picker's continuation is the caller's. Swap / take / move-to-column / move-to-workspace are
   "pick a target, THEN do X with it" — one picker that yields the chosen target, not five modes.
3. Delete: the InputMode pick variants and their handlers; collect_all_pane_candidates /
   collect_workspace_candidates / collect_column_candidates (heca/src/app/selection.rs);
   `offer_hint_by_key` + `offer_in_every_tree` + `wanted` (heca/src/chrome/hint.rs) — every target is
   then addressed the one way, by where it is in the tree, and the library stops having two doors for
   handing a widget its letter.
4. `InputMode::awaits_pick_letter` (app_state.rs:201) goes with them — the family it names stops
   existing.

═══ DEPENDENCIES — VERIFIED, AND NARROWER THAN FIRST CLAIMED ═══
- **T436 (P082/F003)** is the real prerequisite for the plugin half: KeyHintGroup has no `WidgetKind`
  and `on_action` has no declarative form, so a plugin can BE PICKED but cannot OWN A PICKER.
- **T432 (P082/F003)** is needed only for the GENERAL filter ("letter every target whose pick does
  X") and for dropping picks the policy would refuse — both need the pick to carry an `Intent`
  instead of an opaque `Box<dyn Fn()>`.
- **NEITHER is needed to fold `prefix+q` into the hint picker for panes**: a surface filter over the
  `HintSurface::Pane` added by T451 expresses "only panes" today, with no string parsing. Caveat: the
  sidebar row showing that pane lives in the Chrome surface, so a surface filter alone would not
  letter it — decide whether that is wanted before choosing.

═══ ALSO SETTLE HERE ═══
The pick's `InteractionSource`. T451 labels a pane pick `Keyboard` rather than `MouseContent`, on the
grounds that a pick carries no pointer (the same reasoning already applied to Rpc). It changes
behaviour in exactly one case — with a dock focused the domain becomes `Container`, so the pick is
allowed where `MouseContent` would resolve to `Floating` and refuse. NOT established by tracing: a
pick and a real click emitted the same source before that change, so no trace can separate them.

═══ VERIFICATION ═══
Antonio drives. `prefix+q` and `prefix+/` must be indistinguishable in look and placement, on panes
and on sidebar rows alike; swap/take must still work; and a second content kind (T454) must be
pickable by a picker it declares itself, with nothing added to the core.

Approved prerequisite map: consume T398's described/registry contract and T454's reusable second-content proof; T432/T436 are completed prerequisites, not new work. The implementation must reuse the existing KeyHintGroup/picker mechanisms.
