use heca_view::build::{Parent as _, Style as _};
use super::*;

pub(super) fn described_tree() -> build::Panel {
    // Written through the typed builders rather than as a bag of properties. The
    // difference is what does not compile: `Label::new(..).title(..)` is an error because a label
    // has no title, and `Separator::new().orientation(..)` will not take a misspelt word.
    //
    // They are namespaced because they deliberately share their names with the widgets they
    // describe — `build::Row` is the description, `Row` is the thing on screen.
    build::Panel::new()
        .title("DESCRIBED")
        .width(240.0)
        .child(
            build::Row::new()
                // Padding, like every other Row on this page. A row whose content reaches its own
                // edges leaves the badge's glow halo (which paints outside its box, by design)
                // crossing the hover pill's edge, and it reads as the badge overflowing.
                .padding(6.0)
                .on_press(Intent::new("showcase.select").arg("id", PropValue::Text("nginx".into())))
                // **A tooltip is a declaration, not a box around the row**. It is
                // universal like `.key(..)`, so a described node says it exactly as its native twin
                // below does — hover either and the same bubble appears, on the same side, after
                // the same rest. There is no `WidgetId::Tooltip` to construct and anchor.
                .tooltip("Running · 3 days")
                .tooltip_side(heca_view::ViewTooltipSide::Bottom)
                // **Where the letter goes is a declaration too**. A wide list row
                // wants its cap on the right, where it does not sit on the row's own label — and
                // until now saying so meant wrapping the row in a `KeyHint`, which a description
                // cannot do. Press `prefix+/` and compare it with the native twin.
                .hint_placement(heca_view::ViewHintPlacement::CenterRight)
                .child(build::Label::new("nginx"))
                .child(build::Badge::new("UP")),
        )
        .child(build::Separator::new())
        .child(build::Label::new("colour by token name").color("danger"))
        // **The two ways of not showing something**. Neither needs a wrapper on
        // either authoring path any more. The first keeps its box — the gap below the rule is it —
        // and the second is gone from the layout, so nothing marks where it was.
        .child(build::Label::new("ink hidden, box kept").visible(false))
        .child(build::Label::new("gone from the layout").hidden(true))
        // **Three kinds a described tree could not name before.** The spinner takes no properties
        // at all; the bar's fill eases toward whatever value it is given, so re-describing the tree
        // animates it; and the key glyph comes from the keyboard font, its own vocabulary, because
        // the pictogram set has no keys whatsoever.
        .child(
            build::HStack::new()
                .gap(8.0)
                .child(build::Spinner::new().width(16.0).height(16.0))
                .child(build::Progress::new().value(0.4).width(120.0))
                .child(
                    build::NfIcon::new()
                        .glyph(heca_view::ViewNfGlyph::Command)
                        .size(14.0),
                )
                .child(build::Label::new("K")),
        )
        // **The pane header's own action row, described**. It shows icons, says
        // its words on hover, and collapses whatever does not fit into a ⋮ that runs the same
        // actions — none of which is written here. A plugin could only hand-build this before, and
        // would have got the collapse and the overflow menu approximately right.
        .child(
            build::ButtonGroup::new()
                .display(heca_view::ViewDisplay::IconOnly)
                .child(
                    build::Button::new()
                        .text("Close")
                        .icon(ViewGlyph::Close)
                        .on_press(Intent::new("showcase.close")),
                )
                .child(
                    build::Button::new()
                        .text("Split")
                        .icon(ViewGlyph::SquareSplitHorizontal)
                        .on_press(Intent::new("showcase.split")),
                ),
        )

}

/// The same four widgets built by hand, to compare against [`described_tree`] with the eye.
pub(super) fn native_twin(theme: &Theme) -> Panel {
    Panel::titled("NATIVE")
        .width(240.0)
        .child(
            Row::new()
                .padding(6.0)
                .on_activate(|| println!("[showcase] native row activated"))
                // The native half of the pair above: one builder, on the widget.
                .tooltip("Running · 3 days")
                .tooltip_side(TooltipSide::Bottom)
                // One builder on the widget — no `KeyHint` wrapper, which is the point.
                .hint_placement(HintPlacement::CenterRight)
                .child(Label::new("nginx"))
                .child(Badge::new("UP")),
        )
        .child(Separator::horizontal())
        .child(Label::new("colour by token name").color(theme.colors.danger))
        // The native half of the pair above — one builder each, on the widget.
        .child(Label::new("ink hidden, box kept").visible(false))
        .child(Label::new("gone from the layout").hidden(true))
        .child(
            Flex::row()
                .gap(8.0)
                .child(Spinner::new().width(16.0).height(16.0))
                .child(ProgressBar::new().value(0.4).width(120.0))
                .child(NfIcon::new(NfGlyph::Command).size(14.0))
                .child(Label::new("K")),
        )
        .child(
            ButtonGroup::new()
                .display(heca_grid_ui::widgets::Display::IconOnly)
                .child(
                    Button::new("Close")
                        .icon(Glyph::Close)
                        .on_click(|| println!("[showcase] native close")),
                )
                .child(
                    Button::new("Split")
                        .icon(Glyph::SquareSplitHorizontal)
                        .on_click(|| println!("[showcase] native split")),
                ),
        )
}

/// A described tree beside its hand-built twin. They should be indistinguishable.
///
/// This section exists because `realize` lives in `heca-view-realize`, **below**
/// this example, so a `ViewNode` can be rendered here at all. While it sat inside the `heca` binary
/// the showcase could not call it, the only described tree in the whole app was one modal body, and
/// every widget added to the vocabulary had to be taken on trust from a test.
pub(super) fn described_vs_native(theme: &Theme) -> Flex {
    // What a click means is the host's business — `realize` only hands back the node's own intent.
    let emit: IntentEmitter = Rc::new(|intent: Intent| {
        println!(
            "[showcase] described intent: {} {:?}",
            intent.action, intent.args
        );
    });
    let mut forms = FormBindings::default();
    let described = realize(&described_tree(), theme, &emit, &mut forms)
        .expect("the compiled showcase fixture must match the library registry");

    Flex::row().gap(28.0).align("start")
        .child(described)
        .child(native_twin(theme))
}
