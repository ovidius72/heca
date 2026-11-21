//! A region's list: the order it was told, a dock the user moved, removing and keeping by name,
//! and the changes refused out loud.

use super::*;
use crate::chrome::regions;
use crate::providers::ChromeCtx;
use crate::providers::WorkspacesContainerProvider;

fn ws(id: &str) -> WorkspacesContainerProvider {
    WorkspacesContainerProvider::new(id)
}

fn ids(host: &ChromeHost, r: RegionId) -> Vec<String> {
    host.contributions(r)
        .iter()
        .map(|c| c.id().to_string())
        .collect()
}

/// Apply the queue, against a store of its own — what startup does with the app's.
fn mount(host: &mut ChromeHost) {
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    host.mount_pending(&ChromeCtx::new(&store), &[]);
}

/// Start from an empty queue: build a host (which opens it) and apply whatever was left.
fn fresh() -> ChromeHost {
    let mut h = ChromeHost::new(ChromeEventBus::default());
    mount(&mut h);
    ChromeHost::new(ChromeEventBus::default())
}

/// **A region is fetched by name and holds a list** — append, prepend, one or several, in the
/// order the calls were made. The line a plugin author writes.
#[test]
fn a_region_fetched_by_name_holds_a_list_in_the_order_it_was_told() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("b"));
    regions("sidebar.left").append([ws("c"), ws("d")]);
    regions("sidebar.left").prepend(ws("a"));
    regions("sidebar.right").prepend([ws("x"), ws("y")]);
    mount(&mut host);

    assert_eq!(ids(&host, RegionId::LeftSidebar), ["a", "b", "c", "d"]);
    assert_eq!(
        ids(&host, RegionId::RightSidebar),
        ["x", "y"],
        "a prepended list keeps the order it was written in",
    );
    assert_eq!(host.placement("x"), Some(RegionId::RightSidebar));
}

/// **What the code adds is a default; where the user puts it is where it stays.**
///
/// A dock another crate adds to `sidebar.right` starts there. When the user moves it, the move
/// is what the host answers from — nothing about the line that added it pulls it back, whether
/// the host is asked once or many times. (Placement is in memory only today: it is not saved
/// across runs, so "the user's choice" lasts as long as the session.)
#[test]
fn a_dock_the_user_moved_stays_where_they_put_it() {
    let mut host = fresh();
    regions("sidebar.right").append(ws("notes"));
    mount(&mut host);
    assert_eq!(
        host.placement("notes"),
        Some(RegionId::RightSidebar),
        "the default"
    );

    host.move_container("notes", RegionId::LeftSidebar).unwrap();

    assert_eq!(host.placement("notes"), Some(RegionId::LeftSidebar));
    assert_eq!(ids(&host, RegionId::LeftSidebar), ["notes"]);
    assert!(ids(&host, RegionId::RightSidebar).is_empty());
    // Nothing re-applies the code's default: a second look gives the same answer.
    mount(&mut host);
    assert_eq!(host.placement("notes"), Some(RegionId::LeftSidebar));
}

/// **Remove and retain act by name**, on what is there when they run — so a plugin can take out
/// a built-in the app added before it.
#[test]
fn remove_and_retain_take_out_containers_by_name() {
    let mut host = fresh();
    regions("sidebar.left").append([ws("workspaces"), ws("docker"), ws("notes"), ws("git")]);
    regions("sidebar.left").remove("workspaces");
    regions("sidebar.left").retain(|id| id != "docker");
    mount(&mut host);

    assert_eq!(ids(&host, RegionId::LeftSidebar), ["notes", "git"]);
    assert_eq!(
        host.placement("workspaces"),
        None,
        "a removed container is nowhere"
    );
    assert_eq!(host.placement("docker"), None);
}

/// Removing a name the region does not hold is said out loud, with what it does hold.
#[test]
fn removing_a_name_that_is_not_there_is_reported() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("notes"));
    regions("sidebar.left").remove("dokcer");
    mount(&mut host);

    assert_eq!(ids(&host, RegionId::LeftSidebar), ["notes"]);
    assert!(
        crate::chrome::input::identity::said()
            .iter()
            .any(|m| m.contains("'dokcer'") && m.contains("notes")),
        "the mistake and what the region holds are both named",
    );
}

/// **A name nobody knows is reported and changes nothing** — a plugin's typo must neither take
/// the app down nor vanish.
#[test]
fn a_region_nobody_knows_is_reported_and_changes_nothing() {
    let mut host = fresh();
    regions("sidebar.lft").append(ws("lost"));
    mount(&mut host);

    assert_eq!(host.placement("lost"), None);
    assert!(
        crate::chrome::input::identity::said()
            .iter()
            .any(|m| m.contains("'sidebar.lft'") && m.contains("sidebar.left")),
        "the bad name and the real ones are both named",
    );
}

/// **A change made while the app runs is applied** — a plugin adds a dock after startup and it
/// appears at the next pass ([`ChromeHost::apply_pending`]).
#[test]
fn a_change_made_while_the_app_runs_is_applied() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("early"));
    mount(&mut host);

    regions("sidebar.left").append(ws("late"));
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    assert!(
        host.apply_pending(&ChromeCtx::new(&store)),
        "something was applied"
    );
    assert_eq!(ids(&host, RegionId::LeftSidebar), ["early", "late"]);
    assert!(
        !host.apply_pending(&ChromeCtx::new(&store)),
        "nothing queued, nothing done"
    );
}

/// **A region keeps the air it was given between its containers** — the one thing it says about
/// arranging them. Sizes are each container's own (`.flex` on its body).
#[test]
fn a_region_holds_the_gap_it_was_given() {
    let mut host = fresh();
    regions("sidebar.left").gap("md").append(ws("a"));
    mount(&mut host);

    assert_eq!(
        host.layout(RegionId::LeftSidebar).gap,
        Some(heca_grid_ui::style::Space::from("md")),
    );
    assert_eq!(
        host.layout(RegionId::TopBar).gap,
        None,
        "a region nobody spoke to keeps the default"
    );
}

/// A dock that counts every event it hears, from what it subscribed to when it was started.
struct Listening {
    id: &'static str,
    heard: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Provider for Listening {
    fn id(&self) -> &str {
        self.id
    }
    fn supported_regions(&self) -> crate::chrome::RegionSet {
        crate::chrome::RegionSet::sidebars()
    }
    fn title(&self) -> &str {
        self.id
    }
    fn build_contribution(&self, _ctx: &ChromeCtx<'_>) -> crate::chrome::Contribution {
        unimplemented!("only its start is asked about")
    }
    fn on_activate(&mut self, ctx: &ChromeCtx<'_>) -> crate::providers::ProviderHandles {
        let heard = self.heard.clone();
        let mut handles = crate::providers::ProviderHandles::default();
        handles.keep(ctx.app().on("*", move |_| heard.set(heard.get() + 1)));
        handles
    }
}

/// **A placed dock is started, and stays listening while it is seated.** Its `on_activate` runs when
/// the app applies the queue, and what it subscribed to is kept with the seating — so a dock that was
/// added and then removed hears nothing.
#[test]
fn a_placed_dock_is_started_and_hears_events_while_it_is_seated() {
    let mut host = fresh();
    let kept = std::rc::Rc::new(std::cell::Cell::new(0));
    let removed = std::rc::Rc::new(std::cell::Cell::new(0));
    regions("sidebar.left").append(Listening {
        id: "kept",
        heard: kept.clone(),
    });
    regions("sidebar.left").append(Listening {
        id: "removed",
        heard: removed.clone(),
    });
    regions("sidebar.left").remove("removed");

    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    host.mount_pending(&ChromeCtx::new(&store), &[]);
    store.events().emit(ChromeEvent::ContainerPlacementChanged {
        container_id: "kept".to_string(),
        region: RegionId::LeftSidebar,
    });

    assert_eq!(
        kept.get(),
        1,
        "the seated dock was started and heard the event"
    );
    assert_eq!(removed.get(), 0, "a removed dock does not listen");
}

/// `[[docks]]` entries for workspaces docks, one per `(id, location)`, as the user writes them.
fn entries(docks: &[(&str, &str)]) -> Vec<heca_config::docks::DockEntry> {
    docks
        .iter()
        .map(|(id, location)| heca_config::docks::DockEntry {
            name: "workspaces".into(),
            id: Some(id.to_string()),
            location: Some(location.to_string()),
            order: None,
            flex: None,
        })
        .collect()
}

/// Apply the queue with `[[docks]]`, as startup does with the user's config.
fn mount_with(host: &mut ChromeHost, docks: &[(&str, &str)]) {
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    host.mount_pending(&ChromeCtx::new(&store), &entries(docks));
}

/// **The user's config says where a dock starts; the code's region is only the fallback.** An
/// extension adds its dock to `sidebar.right`; a `[[docks]]` entry for `rooms` with `location = "sidebar.left"` puts it on the
/// left. A dock the config does not name starts where its code said.
#[test]
fn the_config_says_where_a_dock_starts_and_the_code_is_the_fallback() {
    let mut host = fresh();
    regions("sidebar.right").append(ws("rooms"));
    regions("sidebar.right").append(ws("notes"));
    mount_with(&mut host, &[("rooms", "sidebar.left")]);
    assert_eq!(ids(&host, RegionId::LeftSidebar), ["rooms"]);
    assert_eq!(
        ids(&host, RegionId::RightSidebar),
        ["notes"],
        "a dock not named keeps the code's region"
    );
}

/// **`none` leaves a dock out** — not seated anywhere.
#[test]
fn none_in_the_config_leaves_a_dock_out() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("workspaces"));
    mount_with(&mut host, &[("workspaces", "none")]);
    assert!(ids(&host, RegionId::LeftSidebar).is_empty());
    assert_eq!(host.placement("workspaces"), None);
}

/// **A region nobody knows keeps the code's region** (and is said out loud, not silently dropped).
#[test]
fn an_unknown_region_in_the_config_keeps_the_codes_region() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("workspaces"));
    mount_with(&mut host, &[("workspaces", "sidebar.middle")]);
    assert_eq!(ids(&host, RegionId::LeftSidebar), ["workspaces"]);
}

/// Reload with new `[[docks]]`, as `prefix+Shift+r` does.
fn reload_with(host: &mut ChromeHost, docks: &[(&str, &str)]) {
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    host.reload_docks(&ChromeCtx::new(&store), &entries(docks));
}

/// **`remove` finds a dock by its id wherever it is** — here the user's `[[docks]]` moved it to the
/// right, and the call names the left.
#[test]
fn remove_finds_a_dock_by_id_wherever_it_is() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("rooms"));
    mount_with(&mut host, &[("rooms", "sidebar.right")]);
    assert_eq!(ids(&host, RegionId::RightSidebar), ["rooms"]);

    regions("sidebar.left").remove("rooms");
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    host.apply_pending(&ChromeCtx::new(&store));
    assert!(ids(&host, RegionId::RightSidebar).is_empty());
    assert_eq!(host.placement("rooms"), None);
}

/// **A reload moves the docks whose entry changed** and leaves the others where they are.
#[test]
fn a_reload_moves_the_docks_whose_entry_changed() {
    let mut host = fresh();
    regions("sidebar.left").append([ws("a"), ws("b")]);
    mount_with(&mut host, &[]);
    host.move_container("b", RegionId::RightSidebar).unwrap();

    reload_with(&mut host, &[("a", "sidebar.right")]);
    assert_eq!(ids(&host, RegionId::RightSidebar), ["b", "a"]);
    assert!(
        ids(&host, RegionId::LeftSidebar).is_empty(),
        "b, moved by hand, stays: its entry did not change"
    );
}

/// **`"none"` on a reload takes a dock out; back to a region makes it again from its recipe.**
#[test]
fn a_reload_takes_a_dock_out_and_makes_it_again_from_its_recipe() {
    let mut host = fresh();
    regions("sidebar.left").append(|| ws("workspaces"));
    mount_with(&mut host, &[]);

    reload_with(&mut host, &[("workspaces", "none")]);
    assert_eq!(host.placement("workspaces"), None, "\"none\" takes it out");

    reload_with(&mut host, &[("workspaces", "sidebar.right")]);
    assert_eq!(
        ids(&host, RegionId::RightSidebar),
        ["workspaces"],
        "made again, where it now says"
    );
}

/// **A dock handed over as itself cannot be made again** once out — said, not silently lost.
#[test]
fn a_dock_added_as_itself_cannot_come_back_from_none_and_says_so() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("notes"));
    mount_with(&mut host, &[("notes", "none")]);

    reload_with(&mut host, &[("notes", "sidebar.left")]);
    assert_eq!(host.placement("notes"), None);
    assert!(
        crate::chrome::input::identity::said()
            .iter()
            .any(|m| m.contains("'notes'") && m.contains("restart")),
        "the user is told to restart",
    );
}

/// **A recipe may say the dock's share and order too**, and still makes it again on a reload.
#[test]
fn a_recipe_with_flex_and_order_comes_back_with_them() {
    use crate::chrome::PlaceDock;
    let mut host = fresh();
    regions("sidebar.left").append(|| ws("logs").flex(2.0).order(-1));
    mount_with(&mut host, &[("logs", "none")]);
    reload_with(&mut host, &[("logs", "sidebar.right")]);
    assert_eq!(ids(&host, RegionId::RightSidebar), ["logs"]);
}

/// The order a region shows its docks in, by id — what `order` decides.
fn shown_order(host: &ChromeHost, r: RegionId) -> Vec<String> {
    let mut docks: Vec<_> = host
        .contributions(r)
        .iter()
        .enumerate()
        .map(|(added, c)| {
            (
                c.placement().order.unwrap_or_default(),
                added,
                c.id().to_string(),
            )
        })
        .collect();
    docks.sort();
    docks.into_iter().map(|(_, _, id)| id).collect()
}

/// **`order` in `[[docks]]` decides which dock comes first**, over the order they were added in.
#[test]
fn the_config_order_puts_a_dock_first() {
    let mut host = fresh();
    regions("sidebar.left").append([ws("x"), ws("y")]);
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    let docks = vec![heca_config::docks::DockEntry {
        name: "workspaces".into(),
        id: Some("y".into()),
        location: None,
        order: Some(-1),
        flex: None,
    }];
    host.mount_pending(&ChromeCtx::new(&store), &docks);
    assert_eq!(shown_order(&host, RegionId::LeftSidebar), ["y", "x"]);

    // A reload that drops the order gives the code's order back.
    host.reload_docks(&ChromeCtx::new(&store), &[]);
    assert_eq!(shown_order(&host, RegionId::LeftSidebar), ["x", "y"]);
}

/// **An entry without an id is for every dock of the kind**; one naming an id wins for that dock.
#[test]
fn an_entry_for_the_kind_places_every_dock_of_it_and_an_id_wins() {
    let mut host = fresh();
    regions("sidebar.left").append([ws("a"), ws("b")]);
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    let docks = vec![
        heca_config::docks::DockEntry {
            name: "workspaces".into(),
            id: None,
            location: Some("sidebar.right".into()),
            order: None,
            flex: None,
        },
        heca_config::docks::DockEntry {
            name: "workspaces".into(),
            id: Some("b".into()),
            location: Some("bar.top".into()),
            order: None,
            flex: None,
        },
    ];
    host.mount_pending(&ChromeCtx::new(&store), &docks);
    assert_eq!(ids(&host, RegionId::RightSidebar), ["a"]);
    assert_eq!(ids(&host, RegionId::TopBar), ["b"]);
}

/// **An entry for a kind no dock has, or that cannot make another, does nothing.**
#[test]
fn an_entry_for_a_kind_that_cannot_make_another_does_nothing() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("workspaces"));
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    let docks = vec![heca_config::docks::DockEntry {
        name: "notes".into(),
        id: Some("notes.2".into()),
        location: Some("sidebar.right".into()),
        order: None,
        flex: None,
    }];
    host.mount_pending(&ChromeCtx::new(&store), &docks);
    assert_eq!(ids(&host, RegionId::LeftSidebar), ["workspaces"]);
    assert!(ids(&host, RegionId::RightSidebar).is_empty());
}

fn entry(
    id: &str,
    location: &str,
    order: Option<i64>,
    flex: Option<f32>,
) -> heca_config::docks::DockEntry {
    heca_config::docks::DockEntry {
        name: "workspaces".into(),
        id: Some(id.into()),
        location: Some(location.into()),
        order,
        flex,
    }
}

/// **`[[docks]]` adds a second workspaces dock to the same sidebar**, under the first, at twice
/// its height; taking the entry out on a reload takes the dock out, putting it back makes it again.
#[test]
fn the_config_adds_a_second_dock_of_a_kind_and_a_reload_takes_it_out_and_back() {
    let mut host = fresh();
    regions("sidebar.left").append(|| ws("workspaces"));
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    let two = vec![
        entry("workspaces", "sidebar.left", Some(0), None),
        entry("workspaces.2", "sidebar.left", Some(1), Some(2.0)),
    ];
    host.mount_pending(&ChromeCtx::new(&store), &two);
    assert_eq!(
        shown_order(&host, RegionId::LeftSidebar),
        ["workspaces", "workspaces.2"]
    );
    let second = host
        .contributions(RegionId::LeftSidebar)
        .iter()
        .find(|c| c.id() == "workspaces.2")
        .unwrap();
    assert_eq!(second.placement().flex, Some(2.0), "flex from the entry");

    host.reload_docks(&ChromeCtx::new(&store), &two[..1]);
    assert_eq!(
        ids(&host, RegionId::LeftSidebar),
        ["workspaces"],
        "its entry gone, the dock goes"
    );

    host.reload_docks(&ChromeCtx::new(&store), &two);
    assert_eq!(
        shown_order(&host, RegionId::LeftSidebar),
        ["workspaces", "workspaces.2"],
        "and comes back"
    );
}

/// **A reload that changes only `flex` resizes the dock** where it is.
#[test]
fn a_reload_that_changes_flex_resizes_the_dock() {
    let mut host = fresh();
    regions("sidebar.left").append([ws("a"), ws("b")]);
    let store = crate::chrome::SharedChromeState::new(300.0, 300.0);
    host.mount_pending(&ChromeCtx::new(&store), &[]);
    host.reload_docks(
        &ChromeCtx::new(&store),
        &[entry("b", "sidebar.left", None, Some(3.0))],
    );
    let b = host
        .contributions(RegionId::LeftSidebar)
        .iter()
        .find(|c| c.id() == "b")
        .unwrap();
    assert_eq!(b.placement().flex, Some(3.0));
}

/// **A move from any path changes what the window rebuilds on** — the bug was a dock moved by an
/// action (RPC said "ok") that stayed on screen where it was until something else rebuilt.
#[test]
fn a_moved_dock_changes_the_arrangement_the_window_rebuilds_on() {
    use std::hash::Hasher;
    let arrangement = |host: &ChromeHost| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        host.arrangement(&mut hasher);
        hasher.finish()
    };
    let mut host = fresh();
    regions("sidebar.left").append(ws("rooms"));
    regions("sidebar.left").append(ws("notes"));
    mount(&mut host);
    let before = arrangement(&host);
    host.move_container("rooms", RegionId::RightSidebar)
        .unwrap();
    let moved = arrangement(&host);
    assert_ne!(before, moved, "a move is a different arrangement");
    host.reorder("notes", None).unwrap();
    assert_eq!(
        arrangement(&host),
        moved,
        "putting a lone dock at its own end changes nothing"
    );
}
