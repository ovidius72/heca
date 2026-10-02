use super::modes::{ModeKey, ModeKeyPress, mode_key};
use super::*;
use crate::app::conflicts::Conflicts;
use crate::app::registry::{build_component_keymaps, build_modes};
use crate::input::WmAction;
use crate::keymap::BindingIndex;
use crate::keymap::{KeyCombo, KeymapRegistry};
use std::collections::HashMap;

/// The shipped defaults, built the way the app builds them — no config of a test's own, so what
/// these assert is what a user gets.
fn defaults() -> (
    HashMap<String, KeymapRegistry>,
    HashMap<String, KeymapRegistry>,
) {
    let config = heca_config::theme::Config::default();
    let (modes, _) = build_modes(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    let (components, _) =
        build_component_keymaps(&config, &mut Conflicts::default(), &mut BindingIndex::new());
    (modes, components)
}

/// What `written` does in `mode`, with the shipped default keymaps.
fn in_mode(mode: &str, written: &str) -> ModeKey {
    let combo = KeyCombo::parse(written);
    press_in_mode(mode, &combo, written == "Escape", written == "Enter")
}

fn press_in_mode(mode: &str, combo: &KeyCombo, escape: bool, enter: bool) -> ModeKey {
    let keymaps = crate::app::registry::build_keymaps(
        &heca_config::theme::Config::default(),
        &mut Conflicts::default(),
    );
    let key = ModeKeyPress {
        escape,
        enter,
        prefix: false,
        combo,
    };
    mode_key(mode, true, &keymaps.modes, &keymaps.flat, &key)
}

/// **Cmd+V as the keyboard sends it** — built by the same function the app calls on a key
/// press, not parsed from text. A mode used to rebuild the key without Cmd, so Cmd+V reached
/// selection mode as a bare `v` and began a selection instead of pasting (Antonio, 2026-09-29).
#[test]
fn cmd_v_as_pressed_pastes_in_selection_mode() {
    use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
    let combo = crate::app::keyboard::build_event_combo(
        &Key::Character("v".into()),
        &PhysicalKey::Code(KeyCode::KeyV),
        "v",
        ModifiersState::SUPER,
    );
    assert_eq!(
        press_in_mode(SELECTION_MODE, &combo, false, false),
        ModeKey::Own {
            action: crate::keymap::ActionRef::Builtin(WmAction::PasteClipboard),
            stay: true,
        },
    );
}

/// **A key a mode does not bind reaches the normal bindings** (Antonio, 2026-09-29: *"it seems
/// we don't have paste"*) instead of being thrown away — so Cmd+V pastes in a mode that never
/// mentions it, like resize, and a normal key such as `Shift+Home` still scrolls while selecting.
#[test]
fn a_key_a_mode_does_not_bind_reaches_the_normal_bindings() {
    assert_eq!(
        in_mode("resize", "Super+v"),
        ModeKey::Normal(crate::keymap::ActionRef::Builtin(WmAction::PasteClipboard)),
    );
    assert!(
        matches!(in_mode(SELECTION_MODE, "Shift+Home"), ModeKey::Normal(_)),
        "selection mode does not bind Shift+Home; the normal bindings do",
    );
}

/// The mode's own keys still come first, and a key nobody binds never reaches the program.
#[test]
fn a_modes_own_key_wins_and_an_unbound_one_is_swallowed() {
    assert_eq!(
        in_mode(SELECTION_MODE, "y"),
        ModeKey::Own {
            action: crate::keymap::ActionRef::Builtin(WmAction::CopySelection),
            stay: true,
        },
    );
    for key in ["p", "Super+v", "Ctrl+Shift+v"] {
        assert_eq!(
            in_mode(SELECTION_MODE, key),
            ModeKey::Own {
                action: crate::keymap::ActionRef::Builtin(WmAction::PasteClipboard),
                stay: true,
            },
            "{key}: y copies, p (or the usual paste key) pastes — one entry, three keys",
        );
    }
    assert_eq!(in_mode(SELECTION_MODE, "x"), ModeKey::Swallow);
    assert_eq!(in_mode(SELECTION_MODE, "Escape"), ModeKey::Cancel);
    assert_eq!(in_mode("resize", "Enter"), ModeKey::Confirm);
}

fn escape(surface: &FocusedSurface) -> Option<crate::keymap::ActionRef> {
    let (modes, components) = defaults();
    surface_action(surface, &modes, &components, &KeyCombo::parse("Escape"))
}

fn dock(mount: &str, kind: &str) -> FocusedSurface {
    FocusedSurface::Dock {
        mount: mount.to_string(),
        kind: Some(kind.to_string()),
    }
}

/// **THE REGRESSION** (F003/P082/T428). Antonio, driving 2026-08-13: entering a sidebar pane
/// list with `prefix+/` or `prefix+e`, *"I'm not able to exit without selecting anything. I
/// should be able to do Esc and go back to normal in the scrolling area."*
///
/// The keymap and the floor were both correct the whole time; a **global** `Escape` bound to
/// `close_overlay` resolved first and consumed the key, closing nothing because no overlay was
/// up. This asserts the rule that replaced it, at the level the bug lived: with a dock holding
/// the keyboard and no layer in front, `Escape` reaches `unfocus_dock`.
#[test]
fn escape_releases_a_focused_dock_back_to_the_panes() {
    assert_eq!(
        escape(&dock("workspaces", "workspaces")),
        Some(crate::keymap::ActionRef::Builtin(WmAction::UnfocusDock)),
    );
}

/// A layer in front closes itself — the same press, one surface further forward.
#[test]
fn escape_closes_the_layer_in_front() {
    let surface = FocusedSurface::Layer {
        name: Some("heca.expose".to_string()),
    };
    assert_eq!(
        escape(&surface),
        Some(crate::keymap::ActionRef::Builtin(WmAction::CloseOverlay {
            overlay: None
        })),
    );
}

/// **And in the scrolling area it is nobody's.** `None` here is what sends the key on to the
/// global map and then to the pane, so `Escape` still means what it means inside vim. This is
/// the assertion that fails if anyone gives the panes a floor "for symmetry".
#[test]
fn escape_in_the_scrolling_area_belongs_to_the_pane() {
    assert_eq!(escape(&FocusedSurface::Panes), None);
}

/// Nearest declaration wins: what a surface declares for itself beats the floor its kind gets,
/// and beats the global map by resolving here at all. The exposé's `x` is the live case.
#[test]
fn a_surfaces_own_keys_come_before_its_floor() {
    let (modes, components) = defaults();
    let surface = FocusedSurface::Layer {
        name: Some("heca.expose".to_string()),
    };
    assert!(
        surface_action(&surface, &modes, &components, &KeyCombo::parse("x")).is_some(),
        "the map's own delete key resolves in its own layer",
    );
    // A key neither the map nor the `layer` floor claims. (Not `q` — that is the floor's, and
    // key matching is case-insensitive on the name, so `Q` is the same key.)
    assert_eq!(
        surface_action(&surface, &modes, &components, &KeyCombo::parse("w")),
        None,
        "and a key it does not claim falls through to the global map",
    );
}

/// **A way out of an overlay is declared once, for every layer** — in the `layer` floor, not in
/// each surface's own entry and not in the global map.
///
/// The global map is the fallback for what nothing in front claimed, so a key there is taken
/// from the program in the pane whether or not an overlay is up. `close_overlay` shipped as a
/// global `q`, and `:q` in vim stopped at the colon — in every terminal, always (Antonio,
/// driving, 2026-08-20). Declared here it exists only while a layer holds the keyboard, which
/// is the same thing tmux's key tables do.
#[test]
fn closing_an_overlay_is_a_layer_key_never_a_global_one() {
    let (modes, components) = defaults();
    let layer = FocusedSurface::Layer {
        name: Some("heca.expose".to_string()),
    };
    for key in ["q", "Ctrl+q"] {
        assert_eq!(
            surface_action(&layer, &modes, &components, &KeyCombo::parse(key)),
            Some(crate::keymap::ActionRef::Builtin(WmAction::CloseOverlay {
                overlay: None
            })),
            "{key} closes the overlay in front of you",
        );
        // …and in the scrolling area nobody claims it, which is what sends it to the program.
        assert_eq!(
            surface_action(
                &FocusedSurface::Panes,
                &modes,
                &components,
                &KeyCombo::parse(key)
            ),
            None,
            "{key} belongs to the pane when no overlay is up",
        );
    }
}

/// **The sidebar's own keys are claimed before anything reaches the tree.** A key a dock's
/// keymap resolves is the host's, so a focused terminal or field inside the dock never sees
/// `j`/`k`/`Enter` and the cursor keeps moving. Only what comes back `None` here is handed to
/// whatever inside the dock holds focus.
#[test]
fn the_sidebars_cursor_keys_are_claimed_before_the_tree_is_offered_a_key() {
    let (modes, components) = defaults();
    let sidebar = dock("workspaces", "workspaces");
    assert!(
        sidebar.delivers_to_tree(),
        "a dock offers the tree what it leaves over"
    );
    for key in ["j", "k", "Enter"] {
        assert!(
            surface_action(&sidebar, &modes, &components, &KeyCombo::parse(key)).is_some(),
            "{key} stays with the sidebar",
        );
    }
}

/// A dock is consulted at two names, **placement before component**, so narrowing one seating
/// of a provider does not have to restate the rest (F003/P086/T362).
#[test]
fn a_dock_is_consulted_at_its_placement_then_its_kind() {
    let (modes, components) = defaults();
    let combo = KeyCombo::parse("x");
    let by_kind = surface_action(
        &dock("nowhere-in-particular", "workspaces"),
        &modes,
        &components,
        &combo,
    );
    assert!(
        by_kind.is_some(),
        "an unknown mount still resolves through the provider's kind",
    );
}
