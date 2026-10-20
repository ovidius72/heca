use super::super::*;

/// The bars, the drag label, the new-column opening and the bell/search highlights have a home
/// in config instead of a constant in the app — and today's values are what they default to.
#[test]
fn the_look_values_that_were_app_constants_default_to_what_they_were() {
    let cfg = AppearanceConfig::default();
    assert_eq!(cfg.effective_top_bar_height(), 32.0);
    assert_eq!(cfg.effective_bottom_bar_height(), 24.0);
    // A big pane: a quarter of the shorter side, capped at 72. A small one: floored at 24.
    assert_eq!(cfg.effective_drag_label_size(1000.0), 72.0);
    assert_eq!(cfg.effective_drag_label_size(200.0), 50.0);
    assert_eq!(cfg.effective_drag_label_size(40.0), 24.0);
    assert_eq!(cfg.terminal.bell_flash_alpha, 56);
    assert_eq!(cfg.terminal.search_match_alpha, 64);
    assert_eq!(cfg.terminal.search_current_match_alpha, 150);
}

#[test]
fn a_configured_look_value_wins_and_is_kept_sane() {
    let cfg: AppearanceConfig = toml::from_str(
        "top_bar_height = 40.0\nbottom_bar_height = 1000.0\n[pane]\n\
         drag_label_min_size = 10.0\ndrag_label_max_size = 5.0\n\
         [terminal]\nbell_flash_alpha = 200\n",
    )
    .expect("parses");
    assert_eq!(cfg.effective_top_bar_height(), 40.0);
    assert_eq!(
        cfg.effective_bottom_bar_height(),
        96.0,
        "clamped, not trusted"
    );
    // A max below the min cannot invert the range.
    assert_eq!(cfg.effective_drag_label_size(1000.0), 10.0);
    assert_eq!(cfg.terminal.bell_flash_alpha, 200);
}

#[test]
fn defaults_are_all_off() {
    let cfg = AppearanceConfig::default();
    assert_eq!(cfg.transparency, 0);
    assert_eq!(cfg.blur, 0);
    assert_eq!(cfg.terminal.transparency, 0);
    assert_eq!(cfg.terminal.floating_transparency, 0);
    assert_eq!(cfg.terminal.floating_blur, 0);
    assert_eq!(cfg.terminal.show_scrollbar, ScrollbarVisibility::WhenNeeded);
    assert!(cfg.terminal.show_scrolled_up_badge);
    assert_eq!(cfg.vibrancy, Vibrancy::None);
    assert_eq!(cfg.background_blur, 0);
    assert_eq!(cfg.background_transparency, 0);
    assert_eq!(cfg.background_gradient_top, None);
    assert_eq!(cfg.background_gradient_bottom, None);
    assert_eq!(cfg.glow_size, None);
    assert_eq!(cfg.intensity, None);
    assert!(!cfg.is_transparent());
    assert!((cfg.opacity() - 1.0).abs() < f32::EPSILON);
    assert!((cfg.blur_radius()).abs() < f32::EPSILON);
    assert!((cfg.terminal_opacity() - 1.0).abs() < f32::EPSILON);
    assert!((cfg.terminal_floating_opacity() - 1.0).abs() < f32::EPSILON);
    assert!((cfg.terminal_floating_blur_radius()).abs() < f32::EPSILON);
    // z=0 defaults to opaque (background_transparency = 0).
    assert!((cfg.background_alpha() - 1.0).abs() < f32::EPSILON);
    assert!((cfg.background_blur_radius()).abs() < f32::EPSILON);
    assert_eq!(cfg.os_vibrancy(), None);
}

#[test]
fn pane_info_bar_defaults_to_location_and_app_with_split_close() {
    let cfg = AppearanceConfig::default();
    assert_eq!(cfg.pane.title_segments, vec!["location", "app_name"]);
    // Move-left/right are omitted by default (mouse drag already moves panes).
    assert_eq!(cfg.pane.title_actions, vec!["split", "close"]);
    assert!(cfg.pane_info_bar_visible());
}

#[test]
fn pane_info_segments_and_actions_parse_snake_case() {
    let cfg: AppearanceConfig = toml::from_str(
        "[pane]\ntitle_segments = [\"location\", \"app_name\", \"git_branch\", \"git_status\"]\ntitle_actions = [\"split\", \"close\"]",
    )
    .unwrap();
    assert_eq!(
        cfg.pane.title_segments,
        vec!["location", "app_name", "git_branch", "git_status"]
    );
    assert_eq!(cfg.pane.title_actions, vec!["split", "close"]);
}

/// **A button is a name, so a name heca does not ship still parses** — that is what lets a
/// program built on heca put its own button in the list.
#[test]
fn a_pane_button_can_be_any_name() {
    let cfg: AppearanceConfig =
        toml::from_str("[pane]\ntitle_actions = [\"split\", \"demo.show_notes\"]").unwrap();
    assert_eq!(cfg.pane.title_actions, vec!["split", "demo.show_notes"]);
}

#[test]
fn the_expose_show_cwd_key_parses_and_defaults_off() {
    assert_eq!(AppearanceConfig::default().expose.show_cwd, None);
    let cfg: AppearanceConfig = toml::from_str("[expose]\nshow_cwd = true").unwrap();
    assert_eq!(cfg.expose.show_cwd, Some(true));
}

#[test]
fn empty_pane_info_bar_is_not_visible() {
    let cfg: AppearanceConfig =
        toml::from_str("[pane]\ntitle_segments = []\ntitle_actions = []").unwrap();
    assert!(!cfg.pane_info_bar_visible());
}
