//! What an extension built on heca writes to group sessions by project and show a folder — using
//! only heca's public API, the way heca-pro does.

use heca::{ActionResult, FolderLine, ProjectCx, Projection, ServerEvent, Service, ServiceCx, ServiceResult, SessionChange};
use heca_grid_ui::builders::Parent;
use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
use heca_grid_ui::style::{Spacing, WidgetSize};
use heca_grid_ui::theme::Theme;
use heca_grid_ui::widgets::{Flex, Label};
use heca_grid_ui::{Component, LayoutEngine, Size};

#[derive(Default)]
struct Rooms {
    groups: Vec<Vec<String>>,
}

impl Service for Rooms {
    type Message = ();
    type Snapshot = Vec<Vec<String>>;
    type Delta = ();
    const SCHEMA_VERSION: u32 = 1;

    fn receive(&mut self, _: &mut ServiceCx<'_, ()>, _: ()) -> ServiceResult<()> {
        Ok(())
    }

    fn project(&self, cx: &ProjectCx<'_>) -> Option<Projection<Self::Snapshot, ()>> {
        // A window reads the sessions on its own project, and nothing else.
        let mine = cx.session_key();
        let names = cx.sessions().same_project(mine).map(|s| s.name().to_owned()).collect();
        Some(Projection::Snapshot { revision: 0, value: vec![names] })
    }
}

/// **The extension's whole wiring compiles from outside heca**: a service, THE EVENTS listener on
/// `session.changed` with its kind, and an action reading the session's project.
#[test]
fn an_extension_listens_for_session_changes_and_reads_projects() {
    let pro = heca::extension("rooms-demo");
    let rooms = pro.service("rooms", |_| Ok(Rooms::default()));
    rooms.on("session.changed", |rooms: &mut Rooms, cx, ev: &ServerEvent| {
        let Some(ev) = ev.as_session() else { return };
        match ev.kind() {
            SessionChange::Created | SessionChange::Deleted | SessionChange::Renamed => {
                let folder = ev.session().folder().map(std::path::Path::to_path_buf);
                rooms.groups = vec![match folder {
                    Some(folder) => ev.sessions().of_project(&folder).map(|s| s.name().to_owned()).collect(),
                    None => Vec::new(),
                }];
                cx.republish();
            }
            SessionChange::Terminated | SessionChange::Restarted => {}
        }
    });
    rooms.action(pro.action("where"), |_, cx, _| {
        let folder = cx.sessions().folder_of(cx.session_key()).map(|f| f.display().to_string());
        ActionResult::Done(folder)
    });
}

/// **`FolderLine` is placed like any widget**, and its path follows a signal: empty hides it.
#[test]
fn an_extension_places_the_folder_line_and_updates_it_in_place() {
    let line = FolderLine::new(&Theme::default()).path("~/projects/heca").size(WidgetSize::Caption).indent(Spacing::Sm);
    let path = line.path_signal();
    let mut card = Flex::column().child(Label::new("member")).child(line);
    LayoutEngine::new().base_font(15.0).compute(&mut card, Size::new(400.0, 200.0));
    let folder = &card.base().children[1];
    assert!(folder.base().bounds.size.h > 0.0, "a path takes a line");

    path.set(String::new());
    LayoutEngine::new().base_font(15.0).compute(&mut card, Size::new(400.0, 200.0));
    assert_eq!(card.base().children[1].base().bounds.size.h, 0.0, "an empty path takes nothing");
    assert_eq!(path.get_untracked(), "");
}

/// **A dock body shows a list in a service's window-side value** with `.child(shown.map(..))`, which
/// keeps its rows in step when the value changes — no rebuild of the dock, nothing to wire.
#[test]
fn an_extension_lists_its_service_value_in_a_dock_body() {
    let pro = heca::extension("rooms-list-demo");
    let rooms = pro.service("rooms", |_| Ok(Rooms::default()));
    struct View { members: Vec<String> }
    let shown = rooms.client(std::rc::Rc::new(View { members: Vec::new() }), |view, update| {
        if let heca::Update::Snapshot(groups) = update { *view = std::rc::Rc::new(View { members: groups.concat() }); }
    });
    let mut body = Flex::column().child(shown.map(|view| &view.members, |name: &String| Label::new(name.clone())));
    LayoutEngine::new().base_font(15.0).compute(&mut body, Size::new(300.0, 200.0));
    assert!(body.base().children.is_empty(), "nothing shown before the first snapshot");
}
