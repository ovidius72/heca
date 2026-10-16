use heca_view::{DropdownItem, Intent, PropValue};

use crate::app::interaction::{InteractionIntent, InteractionSource};
use crate::chrome::ChromeIntentEmitter;

/// **A contributed row runs its own action, exactly as a row the declarer wrote does.**
/// ⚠️ Ran red against its own bug.
///
/// Rows contributed to somebody else's menu used to be built by hand here and wired to
/// `SubmitOverlay`, which resolves the overlay and hands the chosen id to a *completion*. A
/// menu presented from a widget's own declaration has no completion — nobody registers one —
/// so a contributed row opened fine and then did **nothing at all** when chosen, and nothing
/// failed anywhere.
///
/// It went unnoticed while contributions were the rare case. The moment a pane's whole menu
/// arrived this way, every entry in it was dead. This pins that
/// the merge goes through the one conversion, so the two kinds of row are wired the same way.
#[test]
fn a_contributed_row_carries_the_action_it_runs() {
    let fired = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = fired.clone();
    let emit = ChromeIntentEmitter::of(InteractionSource::MouseContent, move |_, intent| {
        if let InteractionIntent::View(i) = intent {
            sink.borrow_mut().push(i.action.clone());
        }
    });

    let menu = crate::chrome::surfaces::context_menu::menu_from_items(
        "",
        "",
        "",
        vec![heca_view::DropdownItem::with_intent(
            "close",
            "Close pane",
            Intent::new("close_pane_by_id").arg("pane_id", PropValue::Int(4)),
        )],
        &crate::actions::ActionCatalog::default(),
        &emit,
    );

    let rows = menu.into_items();
    assert_eq!(rows.len(), 1, "one entry in, one row out");
    rows[0].activate();
    assert_eq!(
        fired.borrow().as_slice(),
        &["close_pane_by_id".to_string()],
        "choosing the row runs the action the entry declared — not an overlay-resolve that \
         lands on a completion nobody registered",
    );
}

#[test]
fn dropdown_item_builders() {
    // The plain constructor: the entry's id IS the action it runs.
    let close = DropdownItem::new("close", "Close pane").danger(true);
    assert_eq!(close.id, "close");
    assert_eq!(close.label, "Close pane");
    assert_eq!(close.intent.action, "close", "id doubles as the action");
    assert!(
        close.danger && close.enabled,
        "danger set, enabled by default"
    );

    let disabled = DropdownItem::new("dup", "Duplicate").enabled(false);
    assert!(!disabled.enabled && !disabled.danger);
}

/// An entry may run something other than its id — the id keeps the icon/label identity while
/// the intent carries the real action + args. This is what lets a plugin entry dispatch a
/// plugin action, which has no `WmAction` variant at all.
#[test]
fn dropdown_item_can_run_an_action_other_than_its_id() {
    let item = DropdownItem::with_intent(
        "close",
        "Delete pane",
        Intent::new("close_pane_by_id").arg("pane_id", PropValue::Int(7)),
    );
    assert_eq!(item.id, "close", "identity (icon) stays `close`");
    assert_eq!(item.intent.action, "close_pane_by_id", "behaviour differs");
    assert_eq!(item.intent.args.get("pane_id"), Some(&PropValue::Int(7)));
}
