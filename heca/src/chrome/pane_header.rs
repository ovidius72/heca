//! **The in-pane header** — its info bar, its action buttons, and the retained trees behind them
//! (F003/P082/T427 split).
//!
//! It owns what a pane says about itself: the composed name and program icon, the chips (cwd, git,
//! and any a program built on heca adds), the action buttons and their shortcuts, and the per-pane retained widgets the host
//! keeps in step with the session. It owns **nothing** about the chrome around it — the regions,
//! the layer stack and the letters all live beside it, not here.
//!
//! Split out of a 5236-line `chrome/mod.rs` that held every kind of logic at once.

use super::*;
use heca_grid_ui::builders::StyleExt as _;
use heca_grid_ui::widgets::{Button, ButtonGroup, ButtonVariant};

mod env;
pub(crate) use env::{HeaderEnv, PaneHeader, give_header_facts};

mod build;
mod info;
mod inputs;
mod segments;
mod shortcuts;
#[cfg(test)]
mod tests;

pub(crate) use build::*;
pub(crate) use info::*;
pub(crate) use inputs::*;
pub(crate) use segments::*;
pub(crate) use shortcuts::*;
