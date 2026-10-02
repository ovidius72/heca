use super::*;
use crate::color::Color;

#[test]
fn test_default_app_config_uses_grid_tron() {
    let config = Config::default();
    let mut theme = theme::load(&config.settings.theme);
    apply_overrides(&mut theme, &config.settings);

    assert_eq!(config.settings.theme, "grid_tron");
    assert_eq!(theme.name, "Grid Tron");
}

#[test]
fn test_config_default_prefix() {
    let cfg = Config::default();
    assert_eq!(cfg.keys.prefix, "ctrl+b");
}

#[test]
fn test_config_has_default_bindings() {
    let cfg = Config::default();
    assert!(cfg.keys.bindings.contains_key("focus_left"));
    assert!(cfg.keys.bindings.contains_key("split_horizontal"));
    assert!(cfg.keys.bindings.contains_key("zoom_column"));
    assert!(cfg.keys.bindings.contains_key("move_pane_to_column_pick"));
    assert!(cfg.keys.bindings.contains_key("close"));
}

#[test]
fn test_parse_toml_config() {
    let toml = r##"
[settings]
theme = "mocha"
mouse = true

[keys]
prefix = "ctrl+a"
focus_left = ["h", "Left"]
focus_right = "l"

[[keys.command]]
keys = "prefix+Shift+g"
command = "lazygit"
float = true
close_pane = true
keep_on_error = true

[[keys.mode]]
name = "resize"
trigger = "prefix+r"

[[keys.mode.bindings]]
action = "resize_increase"
keys = "="

[program.nvim]
name = "Neovim"
processes = ["v", "nvim", "nv"]
icon = "file_code"
color = "#112233"
"##;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert_eq!(cfg.keys.prefix, "ctrl+a");
    assert_eq!(
        cfg.keys.bindings.get("focus_left").unwrap().keys(),
        vec!["h", "Left"]
    );
    assert_eq!(
        cfg.keys.bindings.get("focus_right").unwrap().keys(),
        vec!["l"]
    );
    assert_eq!(cfg.keys.command.len(), 1);
    assert_eq!(cfg.keys.command[0].kind, "terminal");
    assert!(cfg.keys.command[0].float);
    assert!(cfg.keys.command[0].close_pane);
    assert!(cfg.keys.command[0].keep_on_error);
    assert_eq!(cfg.programs.resolve("nv").name, "Neovim");
    assert_eq!(
        cfg.programs.resolve("nv").icon,
        crate::programs::ProgramIcon::FileCode
    );
    assert_eq!(
        cfg.programs.resolve("nv").color,
        Some(Color::new(17, 34, 51, 255))
    );
    assert_eq!(cfg.keys.mode.len(), 1);
    assert_eq!(cfg.keys.mode[0].name, "resize");
    assert_eq!(cfg.keys.mode[0].bindings.len(), 1);
}

/// Guard: the embedded `config.default.toml` must always parse against the
/// live schema — it is the single source of the non-key defaults, parsed at
/// every startup. A schema/file drift fails here (and would panic the app).
#[test]
fn config_default_toml_parses() {
    let cfg: Config = embedded_base()
        .try_into()
        .expect("config.default.toml + keybindings.default.toml must deserialize into Config");
    assert_eq!(cfg.settings.theme, "grid_tron");
    // The active appearance numeric knobs are all the documented defaults.
    assert_eq!(
        cfg.appearance,
        crate::appearance::AppearanceConfig::default()
    );
    // The program catalog is sourced from the file.
    assert_eq!(cfg.programs.resolve("nvim").name, "Neovim");
    assert_eq!(
        cfg.programs.resolve("nvim").icon,
        crate::programs::ProgramIcon::FileCode
    );
    assert_eq!(
        cfg.programs.resolve("yazi").color,
        Some(Color::new(116, 199, 236, 255))
    );
}

/// Guard: the embedded `keybindings.default.toml` must parse and carry the
/// full default keymap (prefix + ~50 bindings + the three built-in modes).
#[test]
fn keybindings_default_toml_parses() {
    let keys = crate::loader::parse_default_keys();
    assert_eq!(keys.prefix, "ctrl+b");
    // A representative spread of the flat bindings.
    for action in [
        "focus_left",
        "split_horizontal",
        "zoom_column",
        "move_pane_to_column_pick",
        "close",
        "reload_config",
        "paste_clipboard",
    ] {
        assert!(
            keys.bindings.contains_key(action),
            "missing default binding: {action}"
        );
    }
    assert!(
        keys.bindings.len() >= 45,
        "expected the full default keymap"
    );
    // The three built-in modes are present with their bindings.
    for mode_name in ["resize", "selection"] {
        let mode = keys
            .mode
            .iter()
            .find(|m| m.name == mode_name)
            .unwrap_or_else(|| panic!("missing default mode: {mode_name}"));
        assert!(!mode.bindings.is_empty());
    }
}

#[test]
fn deep_merge_merges_tables_per_key_and_replaces_arrays() {
    let base: toml::Value = toml::from_str(
        r#"
[settings]
theme = "grid_tron"
mouse = true

[keys]
focus_left = "prefix+h"
focus_right = "prefix+l"
list = [1, 2, 3]
"#,
    )
    .unwrap();
    let over: toml::Value = toml::from_str(
        r#"
[settings]
mouse = false

[keys]
focus_left = "prefix+a"
list = [9]
"#,
    )
    .unwrap();

    let merged = deep_merge(base, over);
    let settings = merged.get("settings").unwrap();
    // Table values merge per-key: `theme` kept, `mouse` overridden.
    assert_eq!(settings.get("theme").unwrap().as_str(), Some("grid_tron"));
    assert_eq!(settings.get("mouse").unwrap().as_bool(), Some(false));
    let keys = merged.get("keys").unwrap();
    assert_eq!(keys.get("focus_left").unwrap().as_str(), Some("prefix+a"));
    assert_eq!(keys.get("focus_right").unwrap().as_str(), Some("prefix+l"));
    // Arrays are replaced wholesale, not concatenated.
    assert_eq!(keys.get("list").unwrap().as_array().unwrap().len(), 1);
}

#[test]
fn partial_user_config_overlays_on_file_defaults() {
    // User sets a single appearance knob; everything else stays default.
    let user: toml::Value = toml::from_str("[appearance]\ntransparency = 30\n").unwrap();
    let merged = deep_merge(embedded_base(), user);
    let cfg: Config = merged.try_into().unwrap();
    assert_eq!(cfg.appearance.transparency, 30);
    // Untouched values keep their file defaults.
    assert_eq!(cfg.appearance.blur, 0);
    assert_eq!(cfg.settings.theme, "grid_tron");
    assert_eq!(cfg.keys.prefix, "ctrl+b");
    assert!(cfg.keys.bindings.contains_key("focus_left"));
}

#[test]
fn partial_user_keys_overlay_keeps_other_bindings() {
    // User rebinds a single action; every other default binding survives.
    let user: toml::Value = toml::from_str("[keys]\nclose = \"prefix+Shift+x\"\n").unwrap();
    let merged = deep_merge(embedded_base(), user);
    let cfg: Config = merged.try_into().unwrap();
    assert_eq!(
        cfg.keys.bindings.get("close").unwrap().keys(),
        vec!["prefix+Shift+x"]
    );
    assert!(cfg.keys.bindings.contains_key("focus_left"));
    assert!(cfg.keys.bindings.contains_key("split_horizontal"));
}

#[test]
fn config_default_matches_embedded_files() {
    // `Config::default()` is exactly the parsed embedded files.
    let cfg = Config::default();
    assert_eq!(cfg.settings.theme, "grid_tron");
    assert_eq!(cfg.keys.prefix, "ctrl+b");
    assert!(cfg.keys.bindings.len() >= 45);
    assert_eq!(cfg.programs.resolve("zsh").name, "zsh");
}

#[test]
fn terminal_color_overrides_apply_via_apply_overrides() {
    let mut theme = theme::load("mocha");
    let settings: SettingsConfig = toml::from_str(
        r##"
terminal-background = "#112233"
terminal-foreground = "#ddeeff"
"##,
    )
    .expect("settings should parse");

    apply_overrides(&mut theme, &settings);

    assert_eq!(theme.terminal_background, Some(Color::new(17, 34, 51, 255)));
    assert_eq!(
        theme.terminal_foreground,
        Some(Color::new(221, 238, 255, 255))
    );
}

#[test]
fn terminal_color_values_survive_when_settings_do_not_override_them() {
    let mut theme = theme::load("mocha");
    let original_bg = theme.terminal_background;
    apply_overrides(&mut theme, &SettingsConfig::default());
    assert_eq!(theme.terminal_background, original_bg);
}

#[test]
fn font_config_parses_from_full_config() {
    let toml = r##"
[font.family.ui]
normal = "Iosevka"

[font.family.terminal]
normal = "Iosevka Term"
italic = "Iosevka Term Italic"

[font.size]
ui = 18.0
terminal = 16.0
"##;
    let cfg: Config = toml::from_str(toml).expect("config should parse");
    assert_eq!(cfg.font.family.ui_normal(), "Iosevka");
    assert_eq!(cfg.font.family.terminal_normal(), "Iosevka Term");
    assert_eq!(
        cfg.font.family.terminal.italic.as_deref(),
        Some("Iosevka Term Italic")
    );
    assert_eq!(cfg.font.size.ui, 18.0);
    assert_eq!(cfg.font.size.terminal, 16.0);
    cfg.font.validate().unwrap();
}

#[test]
fn font_config_defaults_when_section_absent() {
    let cfg: Config = toml::from_str("").unwrap();
    assert_eq!(cfg.font.family.ui_normal(), "Geist Mono");
    assert_eq!(cfg.font.family.terminal_normal(), "Maple Mono Normal NF");
    assert_eq!(cfg.font.size.ui, 15.0);
    assert_eq!(cfg.font.size.terminal, 14.0);
}

#[test]
fn test_parse_toml_rejects_invalid_command_kind() {
    let toml = r#"
[[keys.command]]
keys = "prefix+g"
command = "lazygit"
kind = "terminl"
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert!(validate_config(&cfg).is_err());
}

fn toml_value(text: &str) -> toml::Value {
    toml::from_str(text).unwrap()
}

/// **A project file that sets one colour and one key changes only those two**, and everything
/// else stays the user's. (The two values are what the phase's "done when" names.)
#[test]
fn a_project_file_changes_only_the_keys_it_sets() {
    let user = deep_merge(
        embedded_base(),
        toml_value("[settings]\nmouse = false\n[keys]\nfocus_left = \"prefix+a\""),
    );
    let without = layered(user.clone(), None).expect("user config");
    let project = toml_value(
        "[appearance.sidebar]\nborder_color = \"#ff8800\"\n[keys]\nfocus_right = \"prefix+r\"",
    );
    let with = layered(user, Some((PathBuf::from(".heca/config.toml"), project)))
        .expect("user config with the project on top");

    assert_ne!(
        with.appearance.sidebar.border_color,
        without.appearance.sidebar.border_color
    );
    assert_eq!(
        with.settings.mouse, without.settings.mouse,
        "the user's own value stays"
    );
    let mut expected = with.clone();
    expected.appearance.sidebar.border_color = without.appearance.sidebar.border_color;
    expected.keys = without.keys.clone();
    assert_eq!(
        expected, without,
        "nothing else differs: only the colour and the binding"
    );
}

/// **No project file: exactly today's behaviour.**
#[test]
fn without_a_project_file_nothing_changes() {
    let user = embedded_base();
    assert_eq!(
        layered(user.clone(), None).unwrap(),
        finish(user, &config_dir().join("config.toml")).unwrap()
    );
}

/// **A project file the schema rejects is left out and the user's configuration stands.**
#[test]
fn a_project_file_with_a_wrong_shape_keeps_the_users_config() {
    let user = embedded_base();
    let broken = toml_value("[settings]\nmouse = \"not a bool\"");
    let got = layered(
        user.clone(),
        Some((PathBuf::from(".heca/config.toml"), broken)),
    )
    .expect("the user's configuration stands");
    assert_eq!(got, layered(user, None).unwrap());
}

/// **The nearest folder with a `.heca/` wins**, searching from where heca was started upward.
#[test]
fn the_nearest_project_folder_wins() {
    let root = std::env::temp_dir().join(format!("heca-project-{}", std::process::id()));
    let inner = root.join("a").join("b");
    std::fs::create_dir_all(inner.join(".heca")).unwrap();
    std::fs::create_dir_all(root.join(".heca")).unwrap();
    let deep = inner.join("src");
    std::fs::create_dir_all(&deep).unwrap();

    assert_eq!(
        project_config_path(&deep),
        Some(inner.join(".heca").join("config.toml")),
        "the nearest parent that has .heca/"
    );
    assert_eq!(
        project_config_path(&root.join("a")),
        Some(root.join(".heca").join("config.toml"))
    );
    assert_eq!(
        project_config_path(&std::env::temp_dir().join("no-such-heca-folder")),
        None
    );
    let _ = std::fs::remove_dir_all(&root);
}

fn project_folder(name: &str, content: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("heca-trust-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".heca")).unwrap();
    std::fs::write(dir.join(".heca").join("config.toml"), content).unwrap();
    dir
}

/// **An untrusted project file is ignored; a trusted one is applied; an edited one is untrusted
/// again.** The trust question is injected so the test never touches the user's real list.
#[test]
fn a_project_file_applies_only_while_it_is_trusted_as_it_stands() {
    let dir = project_folder("gate", "[settings]\nmouse = false\n");
    let canon = |p: &Path| p.canonicalize().unwrap();

    let untrusted = read_project_with(&dir, |_, _| false).unwrap().unwrap();
    assert!(!untrusted.trusted);
    assert!(
        trusted_layer(Some(untrusted)).is_none(),
        "untrusted: ignored"
    );

    let hash_now = crate::trust::content_hash("[settings]\nmouse = false\n");
    let trusted = read_project_with(&dir, |d, h| canon(d) == canon(&dir) && h == hash_now)
        .unwrap()
        .unwrap();
    assert!(trusted.trusted);
    assert!(trusted_layer(Some(trusted)).is_some(), "trusted: applied");

    // The file is edited: the hash no longer matches what was trusted.
    std::fs::write(
        dir.join(".heca").join("config.toml"),
        "[settings]\nmouse = true\n",
    )
    .unwrap();
    let edited = read_project_with(&dir, |_, h| h == hash_now)
        .unwrap()
        .unwrap();
    assert!(!edited.trusted, "edited: asked about again");
    let _ = std::fs::remove_dir_all(&dir);
}
