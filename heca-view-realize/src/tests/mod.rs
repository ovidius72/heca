use std::rc::Rc;

use heca_grid_ui::reactive::SignalGet;
// The fixed value sets are used by the tests, not by the mapper: a widget's own enum parses the
// name, so `realize` never names these types.
use heca_grid_ui::{Justify, LayoutExt, Length};
use heca_view::{
    ViewLabelSide, ViewMarker, ViewOrientation, ViewScrollAxes, ViewSeverity, ViewTextAlign,
    ViewVariant, WidgetId,
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
    ViewNode::new(WidgetId::VStack)
        .prop("gap", PropValue::Int(8))
        .child(ViewNode::new(WidgetId::Label).text("Delete pane?"))
        .child(
            ViewNode::new(WidgetId::HStack)
                .child(
                    ViewNode::new(WidgetId::Button)
                        .text("Cancel")
                        .prop("variant", PropValue::Variant(ViewVariant::Secondary))
                        .on_press(Intent::new("confirm_cancel")),
                )
                .child(
                    ViewNode::new(WidgetId::Button)
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
/// The match is **exhaustive on purpose**: adding a `WidgetId` without adding an arm to
/// `realize` cannot compile past this point, so the coverage test below fires instead of the new
/// kind silently rendering an empty container. That failure mode is exactly what this guards —
/// it is invisible in review and invisible at runtime.
fn sample_node(kind: WidgetId) -> ViewNode {
    let node = ViewNode::new(kind.clone());
    match kind.as_str() {
        "dialog" => node.text("Title").prop("opened", true.into()).child(ViewNode::new("label").text("Body")),
        "command_palette" => node.prop("opened", true.into()).child(ViewNode::new("command").text("Run").on_press(Intent::new("run"))),
        "toast_stack" => node.child(ViewNode::new("toast").key("1").text("Ready")),
        "image" => node.prop("svg", PropValue::Text("<svg xmlns='http://www.w3.org/2000/svg' width='16' height='16'><rect width='16' height='16'/></svg>".into()))
            .prop("width", 16.0.into()).prop("height", 16.0.into())
            .child(ViewNode::new("label").text("loading").prop("slot", PropValue::Text("loading".into()))),
        // Containers: give them a child, which a correct arm attaches.
        "tree_layout"
        | "message_list"
        | "resizable"
        | "v_stack"
        | "h_stack"
        | "card_grid"
        | "card"
        | "scroll"
        | "panel"
        | "surface"
        | "grid"
        | "marker_group"
        | "item_group"
        | "dock_frame"
        | "overlay" => node
            .text("TITLE")
            .child(ViewNode::new("label").text("child")),

        // Option pickers: their children are `Choice` nodes.
        "select" | "tabs" => node.child(
            ViewNode::new("choice")
                .prop("value", PropValue::Text("a".into()))
                .text("A"),
        ),
        "choice" => node.prop("value", PropValue::Text("a".into())).text("A"),

        // The interactive row: a container that is also a control, so it needs both a child
        // and a press intent — without the intent it is deliberately inert (no focus, no
        // hover), which would read as "realized to nothing" here.
        "row" => node
            .child(ViewNode::new("label").text("row"))
            .on_press(Intent::new("noop")),

        // A slotted row: the label is a prop, the slots are children.
        "item" => node.text("row").child(
            ViewNode::new("status_dot")
                .prop("slot", PropValue::Text("leading".into())),
        ),

        // Leaves that carry text.
        "label"
        | "button"
        | "badge"
        | "badge_button"
        | "tag"
        | "alert"
        | "toast"
        | "input"
        | "checkbox" => node.text("TEXT"),

        // Leaves that carry a glyph.
        "icon" | "icon_button" | "rail_cell" => {
            node.prop("icon", PropValue::Glyph("terminal".into()))
        }

        // Leaves with their own state.
        "toggle" => node.prop("on", PropValue::Bool(true)),
        "gauge" => node.prop("value", PropValue::Float(0.5)),
        "progress" => node.prop("value", PropValue::Float(0.5)),
        "nf_icon" => node.prop("glyph", PropValue::Text("command".into())),
        // Its children are buttons and nothing else, and each needs the pair the group reads:
        // the words for its menu row and hover bubble, the icon for when there is no room.
        "tile" => node.child(
            ViewNode::new("label")
                .text("title")
                .prop("slot", PropValue::Text("title".into())),
        ),
        "button_group" => node.child(
            ViewNode::new("button")
                .text("Close")
                .prop("icon", PropValue::Glyph("close".into()))
                .on_press(Intent::new("noop")),
        ),
        // It has no properties at all: it paints from the clock, and a root node has no
        // container to take a size from, so give it the one every other leaf gets implicitly.
        "spinner" => node
            .prop("width", PropValue::Float(28.0))
            .prop("height", PropValue::Float(28.0)),
        "line" => node.prop("width", PropValue::Float(100.0)).prop("height", PropValue::Float(20.0)),
        "status_dot" => node,
        "splitter" => node
            .prop("line", PropValue::Bool(true))
            .prop("width", PropValue::Float(8.0))
            .prop("height", PropValue::Float(120.0)),
        "landing_slot" => node
            .prop("label", PropValue::Text("a".into()))
            .prop("filled", PropValue::Bool(true)),

        // A rule normally stretches to its container; as a root it has none, so give it a
        // span — and take the chance to drive both of its properties, in the order that would
        // have been wrong before the widget started recomputing from the pair.
        "separator" => node
            .prop("length", PropValue::Float(120.0))
            .prop("orientation", PropValue::Text("vertical".into())),

        // A picker: its children are what it letters, and one of them must be pickable for
        // the picker to be worth anything — so the sample carries a hint, not a press.
        "key_hint_group" => node
            .prop("opens_on", PropValue::Text("sample.pick".into()))
            .child(
                ViewNode::new("label")
                    .text("target")
                    .on_hint(Intent::new("noop")),
            ),

        // Live state is supplied through the typed native adapter.
        "scroll_bar" => node.prop("content_extent", 1000.0.into()).prop("viewport_extent", 100.0.into()).prop("offset", 50.0.into()).prop("height", 120.0.into()),
        "pane" | "focus_scope" | "chrome_region" | "keyed" => node
            .child(ViewNode::new("button").text("Content").on("press", Intent::new("noop"))),
        _ => panic!("missing behavioral sample for {kind}"),
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

fn registered_ids() -> Vec<WidgetId> {
    WidgetRegistry::builtins().expect("valid built-in definitions").definitions()
        .filter(|definition| !definition.metadata().declaration)
        .map(|definition| definition.metadata().id.clone()).collect()
}
