//! **Who is lettering what** — the host's whole half of every keycap on screen (F003/P082/T427).
//!
//! # The rule: whoever offers a letter owns it until they withdraw it
//!
//! A keycap is [`Base::hint_label`](heca_grid_ui::Base), and the widget that declared the pick
//! draws it. Several things offer letters — the universal picker, a
//! [`KeyHintGroup`](heca_grid_ui::widgets::KeyHintGroup) a surface opened for itself, and the
//! move/swap/take/pick input modes — and the only rule that keeps them from fighting is
//! **ownership**: this module remembers exactly which targets *it* lettered and withdraws exactly
//! those. It never clears a label it did not set.
//!
//! **This replaces a mode check, and that is the point.** The pick modes used to be projected onto
//! four per-widget signal lists (`pane_hint`, `ws_hint`, `col_hint`, `dock_hint`) every frame,
//! which wrote `None` over any letter the picker had offered — so `prefix+/` lit nothing in the
//! sidebar while working perfectly over a layer, which is not synced from here. The first fix was
//! `if matches!(state.input_mode, HintPick { .. }) { return }`, and that names a **host mode**: a
//! plugin opening its own group is not `HintPick`, so its letters were clobbered identically and
//! the plugin author had no way to add themselves to that match. Ownership has no such list.
//!
//! # Addressing
//!
//! A mode knows *what* it is lettering (this pane, that workspace) but not where the widget drawing
//! it sits, so it offers by the target's **`key`** — the identity the widget already declares
//! and that the cursor, the right-click and the drag all read. No second addressing scheme for the
//! same rows. The universal picker keeps the path form, because it collects paths, and both write
//! the same slot.

mod letters;
mod surfaces;
mod targets;
mod visibility;

#[cfg(test)]
pub(crate) use letters::Offer;
#[cfg(test)]
pub(crate) use letters::wanted_for_tests;
pub(crate) use letters::{OfferedLetters, sync_offered_letters};
pub(crate) use surfaces::{
    HintTarget, clear_hint_letters, fire_hint, fire_widget_action, target_identity,
};
pub(crate) use targets::active_hint_targets;
