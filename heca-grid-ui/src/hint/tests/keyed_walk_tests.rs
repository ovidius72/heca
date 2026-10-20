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
