//! A whole composed surface, not one kind at a time.

use heca_view::{ViewEvent, WidgetId};

use super::*;

// ── A whole composed surface, not one kind at a time ───────────────────
//
// These two moved here when the app-mounted fixture they were written against was retired.
// The rest of that fixture's tests were duplicates of stronger ones already in this file —
// the picker walk and the paint-inside-the-box walk both ask the same questions of the whole
// vocabulary. These two ask something no per-kind test can: whether a surface a plugin
// *composes* behaves like one heca built.

/// A dialog-shaped described surface: a blocking overlay over a padded panel with a title, a
/// message, and a two-button action row.
///
/// Load-bearing, and not to be simplified away:
/// - **`blocking(true)`** is the only reason the swallow test means anything. A non-blocking
///   overlay lets a press through *by design*, so without this the test passes trivially.
/// - **two buttons reading differently**, each with its own press, so an assertion about one
///   cannot be satisfied by the other.
/// - **the panel's spacing steps**, which are the whole subject of the recipe test.
fn described_confirm() -> ViewNode {
    use heca_view::build::{Parent as _, Style as _};
    use heca_view::{ViewJustify, ViewSpacing, build};

    build::Overlay::new()
        .blocking(true)
        .default_focus("cancel")
        .child(
            build::Surface::new()
                .padding(ViewSpacing::Lg)
                .gap(ViewSpacing::Md)
                .child(build::Label::new("Delete pane?"))
                .child(build::Label::new("This cannot be undone."))
                .child(
                    build::HStack::new()
                        .gap(ViewSpacing::Sm)
                        .justify(ViewJustify::End)
                        .child(
                            build::Button::new()
                                .key("cancel")
                                .text("Cancel")
                                .on_press(Intent::new("myplugin.cancel")),
                        )
                        .child(
                            build::Button::new()
                                .key("delete")
                                .text("Delete")
                                .on_press(Intent::new("myplugin.delete")),
                        ),
                ),
        )
        .into_node()
}

/// **A composed surface swallows the press aimed past it.** ⚠️ Ran red against its own bug.
///
/// Nothing else here asks this: every other test in this file is about what a *kind* realizes
/// to, and none of them delivers a pointer event at all. It is the question this whole phase
/// exists because of — a surface once took its box for its input area, so an invisible one
/// swallowed every press in the app and the chrome beneath stopped answering the mouse, with
/// nothing failing anywhere.
///
/// Asserted in both directions, because half of it passes for the wrong reason: a blocking
/// surface must take the click, and a non-blocking one must let it through.
#[test]
fn a_composed_described_surface_swallows_only_what_it_declared_it_would() {
    use heca_core::layout::{Point, Size};
    use heca_grid_ui::event::{Event, Handled, PointerEvent};
    use heca_grid_ui::{LayoutEngine, Theme};

    let outside = || Event::PointerDown(PointerEvent::at(Point::new(4.0, 4.0)));
    let realized = |node: &ViewNode| {
        realize(
            node,
            &Theme::default(),
            &(Rc::new(|_| {}) as IntentEmitter),
            &mut FormBindings::default(),
        ).expect("valid described test widget")
    };

    let mut blocking = realized(&described_confirm());
    blocking.show();
    LayoutEngine::new().compute(blocking.as_mut(), Size::new(900.0, 600.0));
    assert_eq!(
        heca_grid_ui::dispatch(blocking.as_mut(), &outside()),
        Handled::Yes,
        "a blocking surface takes the click meant for what is behind it, so the panes \
         underneath never answer",
    );

    let open = described_confirm().prop("blocking", PropValue::Bool(false));
    let mut passthrough = realized(&open);
    passthrough.show();
    LayoutEngine::new().compute(passthrough.as_mut(), Size::new(900.0, 600.0));
    assert_eq!(
        heca_grid_ui::dispatch(passthrough.as_mut(), &outside()),
        Handled::No,
        "and one that did not declare coverage lets it through — otherwise the first half of \
         this test passes for any surface at all",
    );
}

/// **A described dialog is spaced by the recipe a native one uses**, not by numbers of its own.
///
/// The two were built from the same library and looked like different products: buttons flush
/// together, no air between the message and the actions, the row left-aligned. `Dialog` kept
/// its panel recipe as private pixel constants, so nothing outside could match it and a plugin
/// could only guess.
///
/// It compares against the constants themselves rather than against copied values, so it fails
/// if either side moves — which is the only way the two can be kept from drifting apart on
/// screen. `a_described_tree_spaces_itself_from_the_theme` proves the steps *arrive*; this
/// proves they are the *right* steps.
#[test]
fn a_described_dialog_is_spaced_by_the_same_recipe_a_native_one_is() {
    use heca_grid_ui::Theme;
    use heca_grid_ui::widgets::{DIALOG_BTN_GAP, DIALOG_GAP, DIALOG_PAD};

    let surface = realize(
        &described_confirm(),
        &Theme::default(),
        &(Rc::new(|_| {}) as IntentEmitter),
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    // The overlay's single child is the panel.
    let panel = surface.base().children[0].as_ref();
    let layout = &panel.base().style.layout;

    use heca_grid_ui::style::Space;
    assert_eq!(
        layout.padding,
        Space::Step(DIALOG_PAD),
        "the panel's padding"
    );
    assert_eq!(
        layout.gap,
        Space::Step(DIALOG_GAP),
        "title to message to actions"
    );

    let actions = panel
        .base()
        .children
        .last()
        .expect("the action row is the panel's last child");
    assert_eq!(
        actions.base().style.layout.gap,
        Space::Step(DIALOG_BTN_GAP),
        "and the space between the buttons — the one that was missing entirely",
    );
    assert_eq!(
        actions.base().style.layout.justify,
        heca_grid_ui::style::Justify::End,
        "the buttons sit where every other dialog's buttons sit",
    );
}

/// **Every widget can be shown and closed — every one, not a chosen few.** ⚠️ Ran red first.
///
/// *"when i say on all widget i mean ALL widget… select, button, pane,
/// dockview etc.. etc..."*. Appearing and disappearing used to live on `Overlay` and on the
/// toast card, so it was a capability only panel-shaped widgets had: a `Dialog` hand-wrote five
/// methods forwarding to the overlay it composed, a `ContextMenu` and a `CommandPalette` could
/// not forward at all because they *are* their own panels, and everyone composing anything
/// later had to find that out and copy it. It is on `Base` now, so the answer is the same for
/// a `Select`, a `Button`, a `DockFrame` and a `Tabs` as for a dialog.
///
/// **It walks `WidgetId::ALL` rather than a list typed out here.** A hand-written list is the
/// same hardcoding one level up: it passes for the widgets someone remembered and says nothing
/// about the one added next month. Adding a variant without adding it to `ALL` is already a
/// compile error, so a new widget is covered by this the day it exists, and its author never
/// has to know this test is here.
#[test]
fn every_widget_kind_can_be_shown_and_closed() {
    use heca_grid_ui::reactive::SignalGet;

    let theme = Theme::default();
    let mut deaf: Vec<String> = Vec::new();
    for kind in registered_ids() {
        let mut w = realize(
            &sample_node(kind.clone()),
            &theme,
            &noop_emitter(),
            &mut FormBindings::default(),
        ).expect("valid described test widget");

        if w.presence().is_none() {
            deaf.push(format!("{kind:?}: has no arrival or exit at all"));
            continue;
        }
        w.show();
        if !w.base().open.get_untracked() {
            deaf.push(format!("{kind:?}: show() did nothing"));
        }
        w.close();
        if w.base().open.get_untracked() {
            deaf.push(format!("{kind:?}: close() did nothing"));
        }

        // **Let a declared exit finish before asking it to come back.** A widget that is still
        // leaving refuses to be re-opened, and that is the rule rather than a fault — it is
        // what stops a surface being resurrected halfway through its own dismissal. The toast
        // card slides out by default and was the one kind that caught this, which is the
        // reason the sweep ticks here instead of asserting straight through.
        let mut frames = 0;
        while w.presence().is_some_and(|p| p.is_leaving()) && frames < 600 {
            w.tick(1.0 / 60.0);
            frames += 1;
        }

        w.toggle();
        if !w.base().open.get_untracked() {
            deaf.push(format!("{kind:?}: toggle() did nothing"));
        }
    }
    assert!(
        deaf.is_empty(),
        "these widgets cannot be shown or closed, so whoever holds one has to know which kind \
         it is before they can put it on screen: {deaf:#?}",
    );
}

/// **A plugin's resizable split, end to end**: the drag is the widget's, and what it means goes
/// out as the intent the author named, carrying how far the edge moved.
#[test]
fn a_described_splitter_reports_its_drag_as_an_intent_with_the_distance() {
    use heca_core::layout::Size;
    use heca_grid_ui::{Event, LayoutEngine, Point, PointerButton};

    let (emit, fired) = recording_emitter();
    let node = ViewNode::new(WidgetId::Splitter)
        .prop("orientation", PropValue::Text("vertical".into()))
        .prop("width", PropValue::Float(8.0))
        .prop("height", PropValue::Float(100.0))
        .on(ViewEvent::Resize, Intent::new("mypanel.resize"));
    let mut widget = realize(
        &node,
        &Theme::default(),
        &emit,
        &mut FormBindings::default(),
    ).expect("valid described test widget");
    LayoutEngine::new().compute(widget.as_mut(), Size::new(200.0, 100.0));

    let at = |x: f64| Point::new(x, 50.0);
    heca_grid_ui::dispatch(
        widget.as_mut(),
        &Event::pointer_pressed(at(4.0), PointerButton::Left),
    );
    heca_grid_ui::dispatch(widget.as_mut(), &Event::pointer_moved(at(10.0)));
    heca_grid_ui::dispatch(widget.as_mut(), &Event::pointer_moved(at(13.0)));
    heca_grid_ui::dispatch(
        widget.as_mut(),
        &Event::pointer_released(at(13.0), PointerButton::Left),
    );

    let deltas: Vec<_> = fired
        .borrow()
        .iter()
        .map(|i| (i.action.clone(), i.args.get("delta").cloned()))
        .collect();
    assert_eq!(
        deltas,
        [
            ("mypanel.resize".to_string(), Some(PropValue::Float(6.0))),
            ("mypanel.resize".to_string(), Some(PropValue::Float(3.0))),
        ]
    );
    assert_eq!(
        heca_grid_ui::cursor_at(widget.as_ref(), at(4.0)),
        heca_grid_ui::Cursor::ResizeHorizontal,
        "a vertical edge moves left and right"
    );
}
