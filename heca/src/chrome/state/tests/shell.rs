//! Region sizes and visibility, and each mount's scroll, cursor and focus.

use super::*;

#[test]
fn every_region_starts_shown() {
    let s = state();
    for region in RegionId::ALL {
        assert!(s.is_visible(region), "{region:?}");
    }
}

/// The bug: with `[settings] show_left_sidebar = false`, `prefix+b` could not bring the sidebar
/// back, because the setting and the toggle each kept their own state. Now there is one.
#[test]
fn config_sets_where_a_region_starts_the_toggle_flips_it_and_reload_resets_it() {
    use crate::chrome::shown_from_settings;
    use crate::input::RegionVisibility;
    use heca_config::settings::SettingsConfig;

    let s = state();
    let hidden_by_config = SettingsConfig {
        show_left_sidebar: false,
        ..Default::default()
    };
    let config = shown_from_settings(&hidden_by_config);

    // Startup: the setting is the starting state.
    s.apply_visibility(&config);
    assert!(!s.is_visible(RegionId::LeftSidebar), "starts hidden");
    assert!(
        s.is_visible(RegionId::RightSidebar),
        "the others are untouched"
    );

    // The toggle (`sidebar_left`) flips that same state, so the sidebar comes back.
    let region = RegionId::LeftSidebar;
    s.set_visible(region, RegionVisibility::Toggle.apply(s.is_visible(region)));
    assert!(s.is_visible(RegionId::LeftSidebar), "the toggle shows it");

    // Reload: the setting puts it back to where it starts.
    s.apply_visibility(&config);
    assert!(
        !s.is_visible(RegionId::LeftSidebar),
        "reload hides it again"
    );
}

#[test]
fn showing_or_hiding_a_region_says_so_once_and_only_when_it_changed() {
    let s = state();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_events = seen.clone();
    let _sub = s
        .events()
        .subscribe("chrome.region.visibility.changed", move |event| {
            if let ChromeEvent::RegionVisibilityChanged { region, visible } = event {
                seen_events.borrow_mut().push((*region, *visible));
            }
        });

    assert!(
        !s.set_visible(RegionId::TopBar, true),
        "already shown: no change"
    );
    assert!(s.set_visible(RegionId::TopBar, false));
    assert!(!s.set_visible(RegionId::TopBar, false));
    assert_eq!(seen.borrow().as_slice(), [(RegionId::TopBar, false)]);
}

#[test]
fn region_initial_sizes() {
    let s = state();
    assert_eq!(s.left_size(), 280.0);
    assert_eq!(s.right_size(), 260.0);
}

/// Resizing a sidebar is announced once, naming that sidebar; the same size again says nothing.
#[test]
fn a_sidebar_resize_is_announced_once_for_that_sidebar() {
    let s = state();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_events = seen.clone();
    let _sub = s
        .events()
        .subscribe("chrome.region.size.changed", move |event| {
            if let ChromeEvent::RegionSizeChanged { region, size } = event {
                seen_events.borrow_mut().push((*region, *size));
            }
        });

    s.set_left_size(300.0);
    s.set_left_size(300.0);
    s.set_right_size(200.0);

    assert_eq!((s.left_size(), s.right_size()), (300.0, 200.0));
    assert_eq!(
        seen.borrow().as_slice(),
        [
            (RegionId::LeftSidebar, 300.0),
            (RegionId::RightSidebar, 200.0)
        ],
    );
}

/// A mount's scroll offset round-trips and emits, keyed by its mount id.
#[test]
fn a_mounts_scroll_round_trips() {
    let s = state();
    assert_eq!(s.container_scroll("workspaces").get_untracked(), 0.0);
    s.set_container_scroll("workspaces", 42.5);
    assert_eq!(s.container_scroll("workspaces").get_untracked(), 42.5);
    // Asking again is the same signal, not a fresh one — otherwise a rebuild would reset it.
    assert_eq!(s.container_scroll("workspaces").get_untracked(), 42.5);
}

/// Every **placement** owns its own scroll offset.
///
/// Scrolling is per container: each nests its own scroll area, and the shell does not scroll at
/// all — it gives containers bounds and they take their shares of them. So the
/// offsets that exist are per placement, and placing one container twice must not make the two
/// move together, which is invisible until there are two of something.
///
/// Two placements show the same content, because that comes from the shared store. Their scroll
/// positions are their own, because those belong to the placement.
#[test]
fn every_placement_owns_its_own_scroll_offset() {
    let s = state();

    s.set_container_scroll("dock.a", 30.0);
    assert_eq!(
        s.container_scroll("dock.b").get_untracked(),
        0.0,
        "two placements of one container scroll independently",
    );
    assert_eq!(
        s.container_scroll("workspaces").get_untracked(),
        0.0,
        "and a third placement elsewhere is untouched",
    );
    assert_eq!(s.container_scroll("dock.a").get_untracked(), 30.0);
}

/// **A placement's cursor announces its moves** — once per real change, naming the placement and
/// the row, and once more when it is cleared. Every writer goes through the one setter, so none has
/// to remember to announce.
#[test]
fn a_cursor_move_is_announced_once_naming_its_placement() {
    let s = state();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_events = seen.clone();
    let _sub = s
        .events()
        .subscribe("container.cursor.changed", move |event| {
            if let ChromeEvent::ContainerCursorChanged { container, key } = event {
                seen_events
                    .borrow_mut()
                    .push((container.clone(), key.clone()));
            }
        });

    s.set_container_cursor("workspaces", Some("pane:4".into()));
    s.set_container_cursor("workspaces", Some("pane:4".into()));
    s.set_container_cursor("workspaces", None);

    assert_eq!(
        seen.borrow().as_slice(),
        [
            ("workspaces".to_string(), Some("pane:4".to_string())),
            ("workspaces".to_string(), None),
        ],
    );
}

/// A cursor belongs to a **placement**, like the scroll offset beside it: place a container
/// twice and each seating points at its own row over the same content.
#[test]
fn each_placement_has_its_own_cursor() {
    let s = state();
    assert_eq!(s.container_cursor("dock.a").get_untracked(), None);

    s.set_container_cursor("dock.a", Some("row-7".into()));
    assert_eq!(
        s.container_cursor("dock.a").get_untracked(),
        Some("row-7".to_string()),
    );
    assert_eq!(
        s.container_cursor("dock.b").get_untracked(),
        None,
        "the other placement of the same container is untouched",
    );

    s.set_container_cursor("dock.a", None);
    assert_eq!(s.container_cursor("dock.a").get_untracked(), None);
}

/// The most recently focused placement is remembered **past the release**, so an action fired
/// from the palette or RPC with nothing focused still has a mount to mean.
#[test]
fn the_last_focused_placement_survives_losing_focus() {
    let s = state();
    assert_eq!(s.last_focused_container(), None);

    s.announce_focused_container(Some("dock.a".into()));
    s.announce_focused_container(Some("dock.b".into()));
    assert_eq!(s.last_focused_container(), Some("dock.b".to_string()));

    s.announce_focused_container(None);
    assert_eq!(
        s.last_focused_container(),
        Some("dock.b".to_string()),
        "clearing focus must not wipe the memory the memory exists to outlive",
    );
}

/// Focus emits once per real change, and never for a repeat — the host announces every frame,
/// so the store is what makes it once.
#[test]
fn focus_emits_only_on_real_change() {
    let s = state();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    let _sub = s.events().subscribe("*", move |event| {
        log.borrow_mut().push(event.name().to_string());
    });

    for _ in 0..3 {
        s.announce_focused_container(Some("workspaces".into()));
    }
    assert_eq!(seen.borrow().as_slice(), ["chrome.container.focus.changed"]);

    s.announce_focused_container(None);
    s.announce_focused_container(None);
    assert_eq!(
        seen.borrow().len(),
        2,
        "letting go is one more change, then quiet again"
    );
}
