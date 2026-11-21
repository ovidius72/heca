//! **A subtree that is hidden is not written to, whichever key-addressed write it is** — the words,
//! the selection and the letter all go through one walk, so they cannot come to disagree about what
//! they may touch.

use crate::builders::{HintExt, IdentityExt, Parent};
use crate::component::Component;
use crate::hint::*;
use crate::reactive::{SignalGet, SignalUpdate};
use crate::widgets::{Flex, KeyHint, Label, Row, Surface};

#[test]
fn a_hidden_subtree_is_left_alone_by_every_write_addressed_by_key() {
    let label = Label::new("old").key("words");
    let words = label.text_signal();
    let row = Row::new().key("row");
    let selected = row.nav_state();
    let pick = KeyHint::new(Surface::new().key("target")).on_hint(|| {});

    let mut hidden = Flex::column().child(label).child(row).child(pick);
    hidden.base_mut().visible.set(false);
    let root = Flex::column().child(hidden);

    assert!(!set_text_by_key(&root, "words", "new"));
    assert_eq!(words.get_untracked(), "old");

    assert!(!set_selected_by_key(
        &root,
        &["row".to_string()],
        Some("row")
    ));
    assert!(!selected.get_untracked());

    assert!(!offer_hint_by_key(&root, "target", Some("a".to_string())));
}

/// A region's cursor lights its row by key and darkens the rest; rows outside the region are left
/// alone, so two docks each keep their own cursor.
#[test]
fn a_regions_cursor_lights_one_row_by_key() {
    let (a, b, other) = (Row::new().key("a"), Row::new().key("b"), Row::new().key("a"));
    let (lit_a, lit_b, lit_other) = (a.nav_state(), b.nav_state(), other.nav_state());
    let root = Flex::row()
        .child(Flex::column().scope_key("rooms").child(a).child(b))
        .child(Flex::column().scope_key("files").child(other));

    assert!(select_key_in_scope(&root, "rooms", Some("b")));
    assert!(!lit_a.get_untracked() && lit_b.get_untracked());
    assert!(!lit_other.get_untracked(), "another dock's row with the same key stays dark");

    select_key_in_scope(&root, "rooms", Some("a"));
    assert!(lit_a.get_untracked() && !lit_b.get_untracked(), "the cursor is single");
    select_key_in_scope(&root, "rooms", None);
    assert!(!lit_a.get_untracked() && !lit_b.get_untracked());
}
