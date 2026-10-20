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
    host.mount_pending(&ChromeCtx::new(&store));
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

/// **After startup a change is refused out loud**, not dropped in silence — regions are set up
/// once, as the app starts.
#[test]
fn a_change_after_startup_is_refused_out_loud() {
    let mut host = fresh();
    regions("sidebar.left").append(ws("early"));
    mount(&mut host);

    regions("sidebar.left").append(ws("late"));
    mount(&mut host);

    assert_eq!(ids(&host, RegionId::LeftSidebar), ["early"]);
    assert!(
        crate::chrome::input::identity::said()
            .iter()
            .any(|m| m.contains("after startup")),
        "the late call is reported",
    );
}

/// Every spelling of a region reads through the one parser RPC and config use.
#[test]
fn the_dotted_names_and_the_old_ones_mean_the_same_region() {
    for (name, region) in [
        ("sidebar.left", RegionId::LeftSidebar),
        ("left-sidebar", RegionId::LeftSidebar),
        ("left", RegionId::LeftSidebar),
        ("sidebar.right", RegionId::RightSidebar),
        ("bar.top", RegionId::TopBar),
        ("bar.bottom", RegionId::BottomBar),
    ] {
        assert_eq!(name.parse::<RegionId>(), Ok(region), "{name}");
    }
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
    fn default_region(&self) -> RegionId {
        RegionId::LeftSidebar
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
    host.mount_pending(&ChromeCtx::new(&store));
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
