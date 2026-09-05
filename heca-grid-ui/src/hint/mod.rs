//! Universal leader / vimium **hint targets** over a laid-out component tree.
//!
//! The host's global picker (a leader like `prefix+/`) lights a letter over **every region that
//! said what a pick does to it** and runs that region's declaration on the keypress. This module is
//! the framework's half: [`collect_hints`] walks the **retained** widget tree (whose `Base.bounds`
//! are filled in by layout each frame) and enumerates every declaration with the rect its letter
//! goes over, and [`fire_hint`] runs one.
//!
//! **Nothing is registered and no id exists.** A widget declares the behaviour itself with
//! [`KeyHint::on_hint`](crate::widgets::KeyHint::on_hint); a target is addressed by its **path**
//! from the root, which lives exactly as long as the frame it was collected in. That is what makes
//! the picker something a plugin can join: there is no host registry to reach and no host-private
//! intent type to name. It replaced an opaque `HintTargetId` the host mapped back to a
//! `pub(crate)` enum — a shipped feature a plugin could only have a second-class version of.


mod actions;
mod collect;
mod declaration;
mod fire;
mod offer;
#[cfg(test)]
mod tests;

pub use actions::{collect_actions, fire_action, DeclaredAction};
pub use collect::{collect_hints, collect_hints_by_surface, hint_targets_of, SurfaceHints};
pub(crate) use collect::{is_target, narrowed, out_of_view, skip};
pub use declaration::Hint;
pub use fire::{fire_hint, hint_intent};
pub use offer::{clear_hints, offer_hint, offer_hint_by_key, set_selected_by_key, set_text_by_key};
