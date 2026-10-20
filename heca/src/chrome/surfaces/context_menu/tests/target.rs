//! Turning the identity a menu's declarer published into the target its providers read.

use crate::chrome::surfaces::context_menu::open::*;
use crate::chrome::surfaces::context_menu::*;

/// **A menu says what it is about, and the host never hit-tests to find out.** ⚠️ Ran red first.
///
/// A pane publishes `pane:<id>` about itself already — the same declaration the keyboard
/// cursor, the right-click target and a drag all read — so a menu declared on a pane arrives
/// carrying it, and this is the whole of turning that back into the thing it names. It is what
/// let the pane's menu stop being hand-built by the mouse handler, and with it went the
/// ordering that made right-clicking a pane never focus it.
///
/// Only the reading is asked here: resolving the hyperlink needs a live terminal, and there is
/// no headless `AppState` to give it one (see `heca/tests/by_id_actions.rs`, written as a
/// source lint for exactly that reason).
#[test]
fn a_menu_declared_on_a_pane_says_which_pane_it_is_about() {
    assert_eq!(
        pane_id_from_key("pane:7"),
        Some(PaneId(7)),
        "read from the identity the pane already publishes, not searched for by position",
    );
}

/// **A declarer that publishes no identity, or one that is not a pane's, is a contribution** —
/// entries that act on app state rather than on a particular thing.
///
/// Guarded because the tempting fix for the test above is to require an identity, and `key` is
/// optional everywhere in this codebase; and because a loose parse would read a sidebar row as
/// a pane and build it the wrong menu.
#[test]
fn an_identity_that_is_not_a_panes_is_never_mistaken_for_one() {
    assert_eq!(pane_id_from_key("sidebar.row"), None);
    assert_eq!(
        pane_id_from_key("pane:"),
        None,
        "a prefix alone names nothing"
    );
    assert_eq!(pane_id_from_key("pane:abc"), None);
    assert_eq!(
        pane_id_from_key("workspace:7"),
        None,
        "a different kind of thing"
    );
}
