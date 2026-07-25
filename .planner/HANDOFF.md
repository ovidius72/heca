# Handoff

**Created at:** 2026-07-25
**Reason:** Session end at ~75% context. PR #246 MERGED. Nothing in flight, nothing uncommitted.

## State — clean

- On `main`, synced with `origin/main` (`df4cdd5`, the PR #246 merge). Feature branch `fix/search-bar-widgets` deleted.
- **899 workspace tests green, 0 failures**; clippy clean under `--all-targets --all-features --locked -- -D warnings`.
- **No task is in-progress.** All three tasks opened this session are `done`.
- ⚠️ A planner **guard bypass** was authorized this session (20 min, expired). If Edit is blocked, open a task — do not reach for a bypass by default.

## Done this session

**F003/P011/T014** — overlay follow-ups. `place_beside` (the third placement authority: rect anchor, *centered* cross axis, all four sides) + `PanelElevation { Panel | Hover }` on `PanelChrome`. `TooltipSide` is now an alias of `BesideSide` — one four-sided vocabulary.

**F003/P011/T015** — overlay sizing + scrollable modal body. Closed; was already delivered and verified.

**F003/P011/T016** — glyph halos. `TextCmd` carries a glow declaratively; `Atlas::glyph_blurred` blurs the glyph's coverage mask into its own cache entry and emission draws **one** quad. An earlier ring-of-offset-copies version was built, rejected on sight (visible spokes) and removed — **do not reintroduce it**, docs say so.

**theming-05/T5** — scrollback search: bar rebuilt from widgets, search made per-pane, query became a real `Input`, Enter/click dead ends fixed, keybinding docs. Full detail in that task.

**Also fixed** (each has its own commit):
- `LayoutEngine::compute` silently dropped a margin on the **root**. Now applied. This is why a `place_beside`-style offset box no longer needs a wrapper.
- `heca-core` PTY tests flaked ~2 runs in 6. Three causes: a temp dir shared across concurrent tests, a fixed 200ms sleep where `sh`'s `tcsetattr(TCSAFLUSH)` discards early input, and PTY contention. Now 0 failures in 8 parallel runs.

## Open — nothing started

1. **`Icon::glow` is unused in the app.** The capability exists and `RailCell` publishes a rest glow, but no chrome icon opts in. Aesthetic call for the user.
2. **Per-pane search has no unit test.** `AppState` owns a `wgpu::Surface<'static>` (77 fields) and cannot be built in a test. A fixture needs the surface made optional across the render path — substantial, own task, **approach agreed before starting**. I got this wrong once: I proposed a fixture, hit the blocker, and instead of reporting it I extracted a `Searches` type and committed it. The user had it reverted.
3. **F004/P001/T008** — unify scrollbar behaviour across four widgets (three different thumb widths). Audited in that task.

## Working rules the user enforced this session

Read these before touching anything; they were each raised more than once.

- **Search before writing.** The repeated failure was starting to type before looking for the thing that already does the job. Known owners: text entry → `Input` + `widget_keymap`; overlay placement → `place_anchored_on`/`place_at_point`/`place_beside`; panel surface + depth → `paint_panel_chrome` + `PanelElevation`; positioning at an app-chosen rect → a `Flex` sized to the region; any size/colour/padding → `Theme`, `Spacing`, or a widget variant. A literal is a bug.
- **A caveat is not a fix.** Writing "⚠️ never do X" about existing behaviour means a defect was found. The only allowed outcomes are fix it now or file a task — never prose. The user's words: *"who could have noticed it in a commit or remember to fix it?"* Nobody. That is why the root-margin bug was fixed in the engine rather than documented around.
- **Discuss architectural changes first.** Do not substitute a different design when the asked-for one is blocked. Report the blocker.
- **Plain English.** The user repeatedly could not follow jargon-dense explanations. Short sentences, no type names or file paths unless they carry the meaning.
- **Rendering changes need the user's eyes.** Green tests are not verification for anything visual. Build, hand it over, wait.

## Where things are

- Widgets + the declarative model: `docs/widgets.md` (updated for everything above, both native and `ViewNode` examples).
- Project rules: `AGENTS.md` — the ⛔ STOP block and the ⭐ SETTLED widget architecture.
- Planner is the **only** source of truth for status. `BACKLOG.md` and the root `*-plan.md` files are stale; their design rationale is still useful, their checkboxes are not.
