# Every file over 600 lines — census, 2026-09-04

The list [`P082(F003)/T468`](../) works from. Measured with `find . -name '*.rs' -not -path
'./target/*' | xargs wc -l`, so these are **total lines including tests** — unlike the 2026-08-21
table in T468's own description, which separated code from tests. Compare shapes, not numbers,
between the two.

AGENTS § 0 rule 4: aim under 400, **above 600 stop and agree a split** — and the line count is the
smoke alarm, not the rule. **The defect is always the second job.** A 250-line file mixing host
wiring with a component is worse than a 900-line file doing one thing.

**65 files are over 600.** Twelve are over 2000.

## Over 2000 — a file nobody can hold in their head

| lines | file | already filed |
|---|---|---|
| 6654 | `heca-grid-ui/tests/phase_a.rs` | — a test file, but it is the library's whole behaviour in one place |
| 3921 | `heca-view-realize/src/lib.rs` | T468 (its 2138-line test module is most of it) |
| 3430 | `heca/src/handlers.rs` | — **not filed**; every action handler in one file |
| 3349 | `heca-renderer/examples/showcase.rs` | T470 |
| 3282 | `heca/src/actions.rs` | — **not filed**; the registry, the catalog, the descriptors, the arg checker |
| 3147 | `heca/src/notification.rs` | — **not filed** |
| 2686 | `heca/src/app/interaction.rs` | T468 |
| 2614 | `heca/src/app/registry.rs` | T466 |
| 2565 | `heca-grid-ui/src/widgets/scroll_region.rs` | T471 |
| 2453 | `heca-grid-ui/src/component.rs` | T468 |
| 2249 | `heca-core/src/backend/terminal/engine.rs` | — **not filed** |
| 2126 | `heca/src/input.rs` | — **not filed**; `WmAction`, the parsers, the priorities |

## 1000–2000

`heca/src/chrome/mod.rs` 2049 · `heca/src/providers/workspaces/mod.rs` 1792 ·
`heca-renderer/src/text.rs` 1744 · `heca-core/src/layout/scrolling.rs` 1737 ·
`heca/src/app/terminal_render.rs` 1694 · `heca/src/rpc.rs` 1619 ·
`heca-core/src/backend/terminal.rs` 1608 · `heca-config/src/appearance.rs` 1485 ·
`heca-view/src/lib.rs` 1455 (T472) · `heca/src/chrome/state.rs` 1427 ·
`heca/src/app/selection_model.rs` 1392 · `heca-grid-ui/src/widgets/command_palette.rs` 1367 ·
`heca/src/app/terminal_host.rs` 1273 · `heca-view/src/build.rs` 1252 ·
`heca/src/chrome/pane_header.rs` 1221 · `heca/src/app_state.rs` 1192 ·
`heca/src/app/render.rs` 1167 · `heca-renderer/src/terminal.rs` 1145 ·
`heca/src/chrome/layers/tests.rs` 1126 · `heca-grid-ui/src/widgets/context_menu.rs` 1108 ·
`heca/src/app/input.rs` 1027 · `heca-grid-ui/src/event.rs` 1002 (T468)

## 600–1000

`heca-theme/src/theme.rs` 961 · `heca-grid-ui/src/builders.rs` 955 (T468, and the one to do first) ·
`heca-grid-ui/src/widgets/select.rs` 950 · `heca-grid-ui/src/pointer.rs` 926 ·
`heca-grid-ui/src/search.rs` 910 · `heca-grid-ui/src/widgets/button.rs` 900 ·
`heca/src/chrome/scene.rs` 876 · `heca-grid-ui/src/scene.rs` 823 · `heca-grid-ui/src/style.rs` 784 ·
`heca/src/chrome/palette.rs` 760 · `heca/src/chrome/expose/mod.rs` 754 ·
`heca/src/providers/mod.rs` 735 · `heca-grid-ui/src/widgets/key_hint.rs` 730 ·
`heca-grid-ui/src/nav.rs` 713 · `heca/src/chrome/overlay.rs` 695 (T468) ·
`heca/src/chrome/context_menu.rs` 687 · `heca-grid-ui/src/widgets/dialog.rs` 665 ·
`heca/src/providers/workspaces/model_tests.rs` 662 · `heca-grid-ui/src/drag/resolve.rs` 660 ·
`heca-grid-ui/src/widgets/input.rs` 656 · `heca-grid-ui/src/widgets/overlay/mod.rs` 653 ·
`heca/src/chrome/expose/pane_card.rs` 642 · `heca-config/src/loader.rs` 639 ·
`heca/src/providers/workspaces/model.rs` 627 · `heca-grid-ui/src/widgets/label.rs` 621 ·
`heca-grid-ui/tests/button_group.rs` 613 · `heca-grid-ui/src/widgets/key_hint_group.rs` 611 ·
`heca-core/src/layout/workspace.rs` 610 · `heca-core/src/layout/column.rs` 609 ·
`heca-config/src/settings.rs` 604

## Six that were over 2000 with no task — FILED 2026-09-04

Antonio asked for them, so they exist now:

| task | file | lines |
|---|---|---|
| `P082(F003)/T509` | `heca/src/handlers.rs` | 3430 |
| `P082(F003)/T510` | `heca/src/actions.rs` | 3282 |
| `P082(F003)/T511` | `heca/src/input.rs` | 2126 |
| `P082(F003)/T512` | `heca/src/notification.rs` | 3147 |
| `P082(F003)/T513` | `heca-core/src/backend/terminal/engine.rs` | 2249 |
| `P082(F003)/T514` | `heca-grid-ui/tests/phase_a.rs` | 6654 |

Each carries the jobs found inside it, a proposed shape, the order to move things in, and the traps
specific to that file. The reasoning that produced them:

- `heca/src/handlers.rs` (3430) — every action's handler. The obvious split is by the same
  categories the action catalog already declares (Navigation / Layout / Pane / Workspace / System),
  which would make the file layout and the catalog agree.
- `heca/src/actions.rs` (3282) — at least three jobs: the registry and its dispatch, the catalog and
  its descriptors, and the argument model (`ArgSpec`, `check_args`, `ArgProblem`). The argument
  model is now shared with the command line (`heca/src/app/cli.rs`), which makes it a thing of its
  own rather than an appendix to actions.
- `heca/src/input.rs` (2126) — `WmAction` itself, `action_from_name`, `build_action`,
  `action_priority`, and the small enums with their parsers.
- `heca/src/notification.rs` (3147), `heca-core/src/backend/terminal/engine.rs` (2249),
  `heca-grid-ui/tests/phase_a.rs` (6654).

## The rule for doing any of it

Unchanged from T468: **agree each split first**, a move and never a rewrite, no behaviour change in
the same commit, and the test counts must match before and after.
