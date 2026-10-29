//! An external native consumer, including declarations, aliases and app-owned resources.
use std::{collections::BTreeSet, path::Path};
use heca_grid_ui::{widgets::*, Component, LayoutExt, builders::Parent, reactive::signal};

fn tokens(source: &str) -> Vec<&str> {
    source.split(|c: char| !c.is_alphanumeric() && c != '_').filter(|token| !token.is_empty()).collect()
}

fn component_types(directory: &Path, types: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() { component_types(&path, types); }
        else if path.extension().is_some_and(|ext| ext == "rs") {
            let source = std::fs::read_to_string(path).unwrap();
            for part in tokens(&source).windows(3) {
                if part[0] == "Component" && part[1] == "for" { types.insert(part[2].into()); }
            }
        }
    }
}

#[test]
fn every_exported_component_has_a_plain_external_construction_and_composition_example() {
    let mut covered = BTreeSet::new();
    macro_rules! widget {
        ($name:ident, $value:expr) => {{ covered.insert(stringify!($name).to_string()); $value }};
    }
    let root = Flex::column()
        .child(widget!(Alert, Alert::new("Alert")))
        .child(widget!(Badge, Badge::new("State")))
        .child(widget!(BadgeButton, BadgeButton::new("Run")))
        .child(widget!(Button, Button::new("Run").child(Label::new("Content"))))
        .child(widget!(ButtonGroup, ButtonGroup::new().child(Button::new("Run"))))
        .child(widget!(Card, Card::new("Card").child(Label::new("Body"))))
        .child(widget!(CardGrid, CardGrid::new().row(Vec::new(), Label::new("Row"))))
        .child(widget!(Checkbox, Checkbox::new()))
        .child(widget!(Choice, Choice::new("a").child(Label::new("A"))))
        .child(widget!(ChromeRegion, ChromeRegion::vertical().child(Label::new("Dock"))))
        .child(widget!(CommandPalette, CommandPalette::new().command(Command::new("Run", || {}))))
        .child(widget!(ContextMenu, ContextMenu::new("tools").child(Menu::new("Tools", "").child(MenuItem::new().child(|| Label::new("Entry"))))))
        .child(widget!(Dialog, Dialog::new("Question").width("60%").height("70%").child(Label::new("Body")).action(Button::new("OK"))))
        .child(widget!(DockFrame, DockFrame::new("Dock").child(Label::new("Body"))))
        .child(widget!(Flex, Flex::row().child(Label::new("A")).child(Button::new("B"))))
        .child(widget!(FocusScope, FocusScope::new(Button::new("Focus"))))
        .child(widget!(Gauge, Gauge::new()))
        .child(widget!(Grid, Grid::new().template_column("1fr auto").child(Label::new("A")).child(Button::new("B"))))
        .child(widget!(Icon, Icon::new(Glyph::Terminal)))
        .child(widget!(IconButton, IconButton::new(Icon::new(Glyph::Terminal))))
        .child(widget!(Image, Image::new("<svg xmlns='http://www.w3.org/2000/svg'/>")))
        .child(widget!(Input, Input::new()))
        .child(widget!(Item, Item::new("Item").leading(Icon::new(Glyph::Terminal)).trailing(Button::new("Run"))))
        .child(widget!(ItemGroup, ItemGroup::new("Group").child(Label::new("Item"))))
        .child(widget!(KeyHint, KeyHint::new(Button::new("Pick"))))
        .child(widget!(KeyHintGroup, KeyHintGroup::new(Button::new("Pick"))))
        .child(widget!(Keyed, { let keyed = Keyed::new(); keyed.show("shape", || Some(Label::new("Kept"))); keyed }))
        .child(widget!(Label, Label::new("Label")))
        .child(widget!(LandingSlot, LandingSlot::new()))
        .child(widget!(Line, Line::new()))
        .child(widget!(MarkerGroup, MarkerGroup::new().child(Label::new("Item"))))
        .child(widget!(MessageList, MessageList::new().entry(Label::new("Message"))))
        .child(widget!(NfIcon, NfIcon::new(NfGlyph::Command)))
        .child(widget!(Overlay, Overlay::new().panel(Label::new("Overlay"))))
        .child(widget!(Pane, Pane::new().child(Grid::new().template_column("1fr auto").child(Label::new("A")).child(Button::new("B")))))
        .child(widget!(Panel, Panel::new().child(Label::new("Panel"))))
        .child(widget!(ProgressBar, ProgressBar::new()))
        .child(widget!(RailCell, RailCell::new(Icon::new(Glyph::Terminal))))
        .child(widget!(Resizable, Resizable::new(Label::new("Resize"))))
        .child(widget!(Row, Row::new().child(Label::new("Row"))))
        .child(widget!(ScrollBar, ScrollBar::new()))
        .child(widget!(ScrollRegion, ScrollRegion::new().child([Label::new("A"), Label::new("B")])))
        .child(widget!(Select, Select::new(["A", "B"])))
        .child(widget!(Separator, Separator::horizontal()))
        .child(widget!(Spinner, Spinner::new()))
        .child(widget!(Splitter, Splitter::vertical()))
        .child(widget!(StatusDot, StatusDot::new(DotStatus::Online)))
        .child(widget!(Surface, Surface::new().child(Label::new("Surface"))))
        .child(widget!(Tabs, Tabs::new(["A", "B"])))
        .child(widget!(Tag, Tag::new("Title").segment(Flex::row().child(Icon::new(Glyph::Terminal)).child(Label::new("Detail")))))
        .child(widget!(Tile, Tile::new().title(Label::new("Tile"))))
        .child(widget!(Toast, Toast::new("Message").action(Button::new("Retry"))))
        .child(widget!(ToastStack, ToastStack::new(signal(vec![ToastSpec::new(1, "Message").action("retry", "Retry")]))))
        .child(widget!(Toggle, Toggle::new()))
        .child(widget!(Tooltip, Tooltip::new(Button::new("Hover"), "Help")))
        .child(widget!(TreeLayout, TreeLayout::new(Label::new("Parent")).branch(Label::new("Child"))))
        .child(widget!(Visibility, Visibility::new(Label::new("Visible"), true)))
        .child(heca::Terminal::new("authoring-inventory").command("pwd").cwd("/tmp").title("Scratch"));
    assert_eq!(root.base().children.len(), covered.len() + 1);

    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../heca-grid-ui/src/widgets");
    let mut components = BTreeSet::new(); component_types(&directory, &mut components);
    let exports = std::fs::read_to_string(directory.join("mod.rs")).unwrap();
    let missing: BTreeSet<_> = exports.split("pub use ").skip(1)
        .flat_map(|declaration| tokens(declaration.split(';').next().unwrap()))
        .filter(|name| components.contains(*name) && !covered.contains(*name)).collect();
    assert!(missing.is_empty(), "exported widgets missing external constructor/composition coverage: {missing:?}");
}

#[test]
fn placement_aliases_and_custom_animations_need_no_storage_conversions() {
    let mut presence = heca_grid_ui::animation::Presence::new();
    presence.set_animation(heca_grid_ui::animation::Animation::of(heca_grid_ui::animation::Fade::new()).build());
    assert!(presence.is_animated());
    let _ = Container::column().child(Svg::new("<svg xmlns='http://www.w3.org/2000/svg'/>"));
}
