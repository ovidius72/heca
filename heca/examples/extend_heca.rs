//! **A program built on heca** — the whole of what another crate writes to add its own dock, action
//! and overlay to heca. Run it with `cargo run -p heca --example extend_heca`.
//!
//! Plain heca is `fn main() { heca::run(); }`. This says who it is (`demo`) and adds three things first, each through the door
//! heca already has for it, and then calls the same `heca::run()`:
//!
//! - a **dock** in the right sidebar — placed by you like any dock: move it, hide it, and your
//!   choice wins over the line below;
//! - an **action**, `demo.add`, reachable from the palette, a key binding, a menu and RPC;
//! - an **overlay**, `demo.detail`, opened with `toggle_layer name=demo.detail` from any of them.

use heca::{
    ActionCategory, ChromeCtx, ContainerContribution, Contribution, Provider, RegionId, RegionSet,
    regions,
};
use heca_grid_ui::builders::Parent;
use heca_grid_ui::widgets::{Flex, Glyph, Label, Overlay};
use heca_grid_ui::{Component, Handled};

/// A dock with a list of notes. `id` is its name in this placement; a second `Notes::new("..")` is a
/// second placement of the same dock.
struct Notes {
    id: String,
}

impl Notes {
    fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

impl Provider for Notes {
    fn id(&self) -> &str {
        &self.id
    }

    fn supported_regions(&self) -> RegionSet {
        RegionSet::sidebars()
    }

    fn default_region(&self) -> RegionId {
        RegionId::RightSidebar
    }

    fn title(&self) -> &str {
        "Notes"
    }

    fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> Contribution {
        Contribution::Container(ContainerContribution {
            id: self.id().to_string(),
            title: self.title().to_string(),
            supported_regions: self.supported_regions(),
            default_region: self.default_region(),
            movable: self.movable(),
            collapsible: self.collapsible(),
            build: Box::new(|_ctx, _build| {
                Box::new(Flex::column().child(Label::new("No notes yet"))) as Box<dyn Component>
            }),
        })
    }
}

fn main() {
    // The dock. A default: where you put it, and whether it shows, is yours to decide.
    regions("sidebar.right").append(Notes::new("notes"));

    // The action. It belongs to no dock, so it is declared here; the handler can only read the app
    // and ask for other actions, which go out the same door a key press does.
    let demo = heca::extension("demo");
    demo.action("add")
        .label("Add note")
        .description("Open the notes overlay to write a new note.")
        .icon(Glyph::Plus)
        .category(ActionCategory::Chrome)
        .run(|cx, _| {
            cx.dispatch("toggle_layer", name_arg("demo.detail"));
            Handled::Yes
        });

    // A button in every pane header that runs `demo.add`: its icon, words and tooltip come from the
    // action. Shown by default; your own `title_actions` list decides once you write one.
    demo.pane_button("add");

    // A chip in every pane header: the text is worked out from facts about the pane and again only
    // when they change. Shown by default; your own `title_segments` list decides once you write one.
    demo.pane_chip("cwd_name", |facts| {
        let dir = facts
            .cwd
            .as_ref()?
            .file_name()?
            .to_string_lossy()
            .into_owned();
        Some(heca::PaneChip::new(Glyph::Folder, format!("in {dir}")))
    });

    // A line under every pane's name in the sidebar. Shown by default; your own
    // `[appearance.sidebar] pane_lines` list decides once you write one.
    demo.pane_line("folder", |facts| {
        let dir = facts
            .cwd
            .as_ref()?
            .file_name()?
            .to_string_lossy()
            .into_owned();
        Some(heca::PaneLine::new(Glyph::Folder, format!("demo: {dir}")))
    });

    // The overlay. Built again each time it is opened, so what it shows is current.
    demo.layer("detail").view(|_cx| {
        Box::new(
            Overlay::new()
                .panel(Flex::column().child(Label::new("A note")))
                .default_open(false),
        )
    });

    heca::run();
}

/// `name=<layer>` — the argument `toggle_layer` takes.
fn name_arg(name: &str) -> heca::PropMap {
    let mut args = heca::PropMap::new();
    args.insert("name".into(), heca::PropValue::Text(name.into()));
    args
}
