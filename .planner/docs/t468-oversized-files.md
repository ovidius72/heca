Found while building F003/P082/T432 (2026-08-21) and reported to Antonio, who asked for them to be filed rather than left in a message. AGENTS § 0 rule 4: aim under 400 lines, **above 600 stop and agree a split** — and the line count is the smoke alarm, not the rule: **the defect is always the second job**.

Two were already split in T432 and are NOT in scope here:
- `heca-grid-ui/src/hint.rs` (807) → `hint/` — `declaration.rs` 75 · `collect.rs` 89 · `fire.rs` 51 · `actions.rs` 61 · `offer.rs` 175 · `tests.rs` 370 · `mod.rs` 30.
- `heca/src/chrome/hint.rs` (759) → `hint/` — `letters.rs` 235 · `surfaces.rs` 191 · `visibility.rs` 139 · `targets.rs` 212 · `mod.rs` 40.

Measured 2026-08-21 (code lines exclude `#[cfg(test)]` modules). Split each by **job**, one file per member, same shape as `widgets/overlay/`, `chrome/layers/` and the two above.

| code / test | file | the jobs in it |
|---|---|---|
| 1899 / 21 | `heca-grid-ui/src/component.rs` | `Base` · the `Component` trait · the whole router (`dispatch`, `deliver`, `focus_path`, `deliver_to_path`, `broadcast`, `run_pick`) · painting (`paint_child`, `shift_subtree`) |
| 1938 / 1267 | `heca/src/chrome/mod.rs` | host logic + UI composition — **already T439** |
| 1296 / 1318 | `heca/src/app/interaction.rs` | policy classification (`action_policy`) · domain computation (`domain_for`, `session_domain`) · routing (`route_in_domain`, `policy_allows`) · dispatch (`dispatch_intent`, `dispatch_action`, `dispatch_view_intent`) · view-intent resolution (`builtin_of`, `view_intent_allowed`) |
| 1157 / 2138 | `heca-view-realize/src/lib.rs` | every kind's realize arm, plus a 2138-line test module in the same file |
| 895 / 764 | `heca/src/providers/workspaces/mod.rs` | the provider · its action table · `perform` · key helpers · menus |
| 973 / 0 | `heca-grid-ui/src/event.rs` | `Event` · `EventKind` · `EventCx` · `Handled` · `GridKey` · `Modifiers` · `PointerEvent` · `RawPointer` · `WidgetIntent` |
| 827 / 0 | `heca-grid-ui/src/builders.rs` | `ComponentExt` + `LayoutExt` + `StyleExt` + `Parent` — a **family**, which § 0.4 says is a folder, one file per member |
| 608 / 89 | `heca/src/chrome/overlay.rs` | `ModalSpec`/`ModalAction`/`ModalResult` · `OverlayHost` · `open_modal` · the dropdown/context-menu builders · `resolve` |

`heca/src/app/registry.rs` (~2600) is already **T466**.

## Order, and why

1. **`heca-grid-ui/src/builders.rs` → `builders/`** — the one with no judgement call in it: a family, one file per trait (`component_ext.rs`, `layout.rs`, `style.rs`, `parent.rs`, `mod.rs`). Do it first as the worked example.
2. **`heca-grid-ui/src/component.rs`** → `base.rs` / `component.rs` (the trait) / `dispatch.rs` (the router — `dispatch`, `deliver`, `broadcast`, `focus_path`, `deliver_to_path`, `run_pick`) / `paint.rs` (`paint_child`, `shift_subtree`). The biggest file in the library and essentially untested in-file; the router is a whole job that reads as an appendix to `Base`.
3. **`heca/src/app/interaction.rs`** → `policy.rs` / `domain.rs` / `route.rs` / `dispatch.rs`. Watch the pure/unit-tested boundary: `route_in_domain` and `policy_allows` are pure and must stay so.
4. **`heca-view-realize/src/lib.rs`** — at minimum the test module moves to `tests.rs`; the realize arms then group by kind family.
5. **`heca/src/chrome/overlay.rs`**, **`heca/src/providers/workspaces/mod.rs`**, **`heca-grid-ui/src/event.rs`**.

## Rules for the work

- **Agree each split with Antonio before doing it** (AGENTS § 0 rule 4) — a large file quietly rearranged is a diff nobody can review. This task is the list, not a licence.
- **A move, never a rewrite.** No behaviour change in the same commit; the test counts before and after must match.
- Watch for the trap hit twice while splitting the two hint files: a doc comment left at the end of a segment documents the *next* file's first item, and a helper used by two of the new modules (`skip`, `is_target`, `hint_surface_root`, `HintLayer`) needs `pub(super)` and one owner — put it with the job that defines it, not in the module root.
- Gates: `cargo test -p <crate>` and `./scripts/lint-changed.sh` per split, never `--workspace` in the edit loop.

---

## ⚠️ CENSUS REFRESHED 2026-09-04 — `.planner/docs/oversized-files-2026-09-04.md`

The table above was measured 2026-08-21 and is now **incomplete**, not wrong: it lists the files
found while building T432, not every file over the limit. A full sweep on 2026-09-04 found **65 files
over 600 lines, twelve of them over 2000**. The census — every file, with which task already covers
it and which have none — is in that doc. Read it rather than re-measuring.

**Six of the twelve largest have no task at all**, and they are the gap the census exists to close:
`heca/src/handlers.rs` (3430), `heca/src/actions.rs` (3282), `heca/src/notification.rs` (3147),
`heca-core/src/backend/terminal/engine.rs` (2249), `heca/src/input.rs` (2126), and
`heca-grid-ui/tests/phase_a.rs` (6654). The doc carries a proposed split for the first three.
**They are proposals — filing is Antonio's call (AGENTS § 0), not something to create while passing
through.**

Raised because P097/T497 touched nineteen files that were already over the limit; AGENTS § 0 says to
say so rather than quietly reorganise (Antonio, 2026-09-04: *"do we have a task already in the
planner that aims to rewrite large files? If so add those large files to the task description"*).
