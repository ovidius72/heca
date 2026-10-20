use std::rc::Rc;

use heca_grid_ui::reactive::SignalGet;
// The fixed value sets are used by the tests, not by the mapper: a widget's own enum parses the
// name, so `realize` never names these types.
use heca_grid_ui::{Justify, LayoutExt, Length, SetProp};
use heca_view::{
    ViewLabelSide, ViewMarker, ViewOrientation, ViewScrollAxes, ViewSeverity, ViewTextAlign,
    ViewVariant, WidgetKind,
};

use super::*;

/// **The SDK's source, whole** — every file of `heca-view/src/build/`, in name order. The guards
/// that read it ask what the builders say, not which file says it, so a split of the SDK moves
/// nothing under them.
fn sdk_source() -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../heca-view/src/build");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("the SDK source folder is where it is expected")
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("an SDK source file is readable"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// **What the framework's picker will find in a realized tree**, in document order: one path
/// per node that declared what a pick does to it. There is no registry to interrogate any more
/// — the declarations are in the tree, so the tests read them from the tree.
fn hints(root: &dyn Component) -> Vec<Vec<usize>> {
    heca_grid_ui::collect_hints(root)
        .into_iter()
        .map(|(path, _)| path)
        .collect()
}


/// A confirm-dialog-shaped tree: a column with a message label + a row of two action
/// buttons (Cancel / Delete), each carrying a `"press"` intent.
fn confirm_tree() -> ViewNode {
    ViewNode::new(WidgetKind::VStack)
        .prop("gap", PropValue::Int(8))
        .child(ViewNode::new(WidgetKind::Label).text("Delete pane?"))
        .child(
            ViewNode::new(WidgetKind::HStack)
                .child(
                    ViewNode::new(WidgetKind::Button)
                        .text("Cancel")
                        .prop("variant", PropValue::Variant(ViewVariant::Secondary))
                        .on_press(Intent::new("confirm_cancel")),
                )
                .child(
                    ViewNode::new(WidgetKind::Button)
                        .text("Delete")
                        .prop("variant", PropValue::Variant(ViewVariant::Destructive))
                        .on_press(Intent::new("confirm_ok")),
                ),
        )
}

fn noop_emitter() -> IntentEmitter {
    Rc::new(|_| {})
}

/// An emitter that keeps every intent a realized tree fires, so a test can pick a letter (or
/// click) and read back exactly what the host would have been handed.
fn recording_emitter() -> (IntentEmitter, Rc<std::cell::RefCell<Vec<Intent>>>) {
    let sink = Rc::new(std::cell::RefCell::new(Vec::new()));
    let out = sink.clone();
    (Rc::new(move |i| sink.borrow_mut().push(i)), out)
}


/// A representative node for `kind` — enough of one that a correct arm produces a *live* widget.
///
/// The match is **exhaustive on purpose**: adding a `WidgetKind` without adding an arm to
/// `realize` cannot compile past this point, so the coverage test below fires instead of the new
/// kind silently rendering an empty container. That failure mode is exactly what this guards —
/// it is invisible in review and invisible at runtime.
fn sample_node(kind: WidgetKind) -> ViewNode {
    let node = ViewNode::new(kind);
    match kind {
        // Containers: give them a child, which a correct arm attaches.
        WidgetKind::VStack
        | WidgetKind::HStack
        | WidgetKind::CardGrid
        | WidgetKind::Card
        | WidgetKind::Scroll
        | WidgetKind::Panel
        | WidgetKind::Surface
        | WidgetKind::Grid
        | WidgetKind::MarkerGroup
        | WidgetKind::ItemGroup
        | WidgetKind::DockFrame
        | WidgetKind::Overlay => node
            .text("TITLE")
            .child(ViewNode::new(WidgetKind::Label).text("child")),

        // Option pickers: their children are `Choice` nodes.
        WidgetKind::Select | WidgetKind::Tabs => node.child(
            ViewNode::new(WidgetKind::Choice)
                .prop("value", PropValue::Text("a".into()))
                .text("A"),
        ),
        WidgetKind::Choice => node.prop("value", PropValue::Text("a".into())).text("A"),

        // The interactive row: a container that is also a control, so it needs both a child
        // and a press intent — without the intent it is deliberately inert (no focus, no
        // hover), which would read as "realized to nothing" here.
        WidgetKind::Row => node
            .child(ViewNode::new(WidgetKind::Label).text("row"))
            .on_press(Intent::new("noop")),

        // A slotted row: the label is a prop, the slots are children.
        WidgetKind::Item => node.text("row").child(
            ViewNode::new(WidgetKind::StatusDot)
                .prop("slot", PropValue::Text("leading".into())),
        ),

        // Leaves that carry text.
        WidgetKind::Label
        | WidgetKind::Button
        | WidgetKind::Badge
        | WidgetKind::BadgeButton
        | WidgetKind::Tag
        | WidgetKind::Alert
        | WidgetKind::Toast
        | WidgetKind::Input
        | WidgetKind::Checkbox => node.text("TEXT"),

        // Leaves that carry a glyph.
        WidgetKind::Icon | WidgetKind::IconButton | WidgetKind::RailCell => {
            node.prop("icon", PropValue::Glyph("terminal".into()))
        }

        // Leaves with their own state.
        WidgetKind::Toggle => node.prop("on", PropValue::Bool(true)),
        WidgetKind::Gauge => node.prop("value", PropValue::Float(0.5)),
        WidgetKind::Progress => node.prop("value", PropValue::Float(0.5)),
        WidgetKind::NfIcon => node.prop("glyph", PropValue::Text("command".into())),
        // Its children are buttons and nothing else, and each needs the pair the group reads:
        // the words for its menu row and hover bubble, the icon for when there is no room.
        WidgetKind::Tile => node.child(
            ViewNode::new(WidgetKind::Label)
                .text("title")
                .prop("slot", PropValue::Text("title".into())),
        ),
        WidgetKind::ButtonGroup => node.child(
            ViewNode::new(WidgetKind::Button)
                .text("Close")
                .prop("icon", PropValue::Glyph("close".into()))
                .on_press(Intent::new("noop")),
        ),
        // It has no properties at all: it paints from the clock, and a root node has no
        // container to take a size from, so give it the one every other leaf gets implicitly.
        WidgetKind::Spinner => node
            .prop("width", PropValue::Float(28.0))
            .prop("height", PropValue::Float(28.0)),
        WidgetKind::StatusDot => node,
        WidgetKind::Splitter => node
            .prop("line", PropValue::Bool(true))
            .prop("width", PropValue::Float(8.0))
            .prop("height", PropValue::Float(120.0)),
        WidgetKind::LandingSlot => node
            .prop("label", PropValue::Text("a".into()))
            .prop("filled", PropValue::Bool(true)),

        // A rule normally stretches to its container; as a root it has none, so give it a
        // span — and take the chance to drive both of its properties, in the order that would
        // have been wrong before the widget started recomputing from the pair.
        WidgetKind::Separator => node
            .prop("length", PropValue::Float(120.0))
            .prop("orientation", PropValue::Text("vertical".into())),

        // A picker: its children are what it letters, and one of them must be pickable for
        // the picker to be worth anything — so the sample carries a hint, not a press.
        WidgetKind::KeyHintGroup => node
            .prop("opens_on", PropValue::Text("sample.pick".into()))
            .child(
                ViewNode::new(WidgetKind::Label)
                    .text("target")
                    .on_hint(Intent::new("noop")),
            ),

        // Host-only — see the coverage test.
        WidgetKind::ScrollBar => node,
    }
}

mod overlay;
mod basics;
mod appearance;
mod sdk_reach;
mod values;
mod keys_rows;
mod options;
mod groups;
mod grid;
mod slots;
mod coverage;
mod picking;
mod universal;
mod spacing_tooltips;
mod cards;
mod forms;
mod layout;
mod generated;
mod composed;
