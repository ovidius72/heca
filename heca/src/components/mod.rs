//! **App-level components** — compositions of `heca-grid-ui` widgets that bind an app concept
//! (F011/P087/T373).
//!
//! The bricks are shared and the walls were not: every composed shape — a dock row, a pane header,
//! an action button, a card in the map — lived inside the one function that needed it, so the only
//! way for a second surface or a plugin to have the same row was to read that file and copy it.
//! This module is where a shape goes once it is wanted by name.
//!
//! # What belongs here, and what does not
//!
//! | | where it lives |
//! |---|---|
//! | Generic, self-contained, complete on its own — `Row`, `Item`, `ScrollRegion`, `Dialog` | **`heca-grid-ui`.** If it can be built inside the library, it belongs in the library — the test is capability, not size. |
//! | A composition that **also binds an app concept** — an `Intent`, an action **name**, a drag id, a chrome signal, a `key` | **a component.** That binding is the *only* thing that justifies leaving the library. |
//! | A composition that binds **none** of them | a widget in the wrong crate. Send it down; do not keep it up here. |
//!
//! # Where a component lives — beside its surface first
//!
//! A component is born **next to the surface that uses it**, one file each:
//!
//! ```text
//! heca/src/chrome/<surface>/
//! ├── mod.rs      host wiring (the part that needs `AppState`)
//! ├── model.rs    the surface's data, reduced from the session
//! └── <thing>_card.rs, <thing>_row.rs
//! ```
//!
//! It moves **here** the day a **second** surface wants it — DRY applied when the duplication is
//! real, not when it is predicted. Until something is shared, a global folder only puts distance
//! between a component and its only caller. **The second caller is the move**, and it is a move,
//! never a copy: two paths over one shape cannot stay identical, and nothing fails when they drift.
//!
//! # How to write one
//!
//! **The full recipe is `AGENTS.md` § 0b-bis, "How to structure and build a component"**, worked
//! through the exposé (`crate::chrome::expose`). What follows is its short form.
//!
//! 1. **Signature: plain data plus the seams it binds** — a [`ChromeIntentEmitter`], an
//!    [`Intent`], `&mut BuildCx` (drag ids, signals), an action **name**, a `&Theme`. **Never
//!    `&AppState`**: it needs a window, so a component that takes it is a component nobody can
//!    test. Same split that made `route_in_domain`, `cursor_follow` and `resolve_context_for`
//!    testable.
//! 2. **Return a component built from library widgets** — a [`WidgetModel`], or the concrete widget
//!    when the caller still has to size it. No `paint`, no measure, no hit-test, no scroll offset:
//!    every one of those is some existing widget's job, and writing one is the tell that the
//!    pre-flight was skipped.
//! 3. **Name the action, never the styling.** The icon comes from `ActionCatalog::icon`, the
//!    shortcut from `ActionShortcuts`, the tooltip from `action_tooltip`, every colour and size
//!    from the `Theme`. A component that spells a shortcut or picks a glyph has taken over a job
//!    that has one owner.
//! 4. **⚠️ Sizes are SHARES, never computed pixels.** The moment a composition multiplies model
//!    numbers by a scale of its own it has taken over the layout engine's job — and then it owns
//!    *every* term: the window, the overlay margin, the panel padding, the gaps, the row count,
//!    each row's height, the scroll centring. Miss one and everything is wrong by exactly that
//!    term. The exposé did this and took **seven attempts, six of them wrong, each missing a
//!    different term**. Express it as `Length::Pct` of a shared denominator and `grow` weights and
//!    taffy answers it exactly, at every window size. (`grow` alone always *fills* its container —
//!    that is what flex-grow means — so "a share of the widest sibling" is a percentage, not a
//!    grow weight.)
//! 5. **Register host ids through `BuildCx`**, so they stay monotonic and are released with the
//!    tree that made them.
//!
//! # The test rule — one headless test per component
//!
//! Headless, asserting the built tree or the painted scene, **never a flag**. This is what "never
//! `&AppState`" exists *for*: a component that cannot be constructed without a window cannot be
//! checked without the maintainer opening the app and photographing it, which is exactly how the
//! exposé came to be debugged by eye against a green suite.
//!
//! Two patterns to follow:
//!
//! - `providers/mod.rs::the_shared_context_carries_nothing_of_one_components_domain` — build
//!   through `ChromeCtx::for_build` and assert the children plus what the host registries received;
//! - `heca-grid-ui/tests/phase_a.rs` — lay the tree out, paint it into a `Scene`, assert the
//!   `DrawCommand`s.
//!
//! A component whose size is a share gets the test that share exists for: **lay it out in a box and
//! assert it never exceeds it**, at several sizes and child counts.
//!
//! [`ChromeIntentEmitter`]: crate::chrome::ChromeIntentEmitter
//! [`Intent`]: heca_view::Intent
//! [`WidgetModel`]: crate::chrome::WidgetModel

pub(crate) mod folder_line;
pub(crate) mod pane_name;

pub(crate) use folder_line::FolderLine;
pub(crate) use pane_name::PaneName;
