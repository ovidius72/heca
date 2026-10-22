//! `[appearance.terminal] padding*` — how the terminal's grid is held in.

use super::super::*;

#[test]
fn padding_defaults_to_no_change() {
    let cfg = AppearanceConfig::default();
    assert_eq!(
        cfg.terminal.effective_terminal_padding(),
        TerminalPadding {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        }
    );
}

#[test]
fn a_unified_padding_reaches_every_side() {
    let cfg: AppearanceConfig =
        toml::from_str("[terminal]\npadding = 6.0\n").expect("terminal padding toml should parse");
    let p = cfg.terminal.effective_terminal_padding();
    assert_eq!(
        (p.top, p.bottom, p.left, p.right),
        (6.0, 6.0, 6.0, 6.0),
        "`padding` holds the grid in from all four sides"
    );
}

#[test]
fn a_side_overrides_the_unified_value() {
    let cfg: AppearanceConfig =
        toml::from_str("[terminal]\npadding = 6.0\npadding_top = 12.0\npadding_left = 0.0\n")
            .expect("terminal side-padding toml should parse");
    let p = cfg.terminal.effective_terminal_padding();
    assert_eq!(
        (p.top, p.bottom, p.left, p.right),
        (12.0, 6.0, 0.0, 6.0),
        "a written side decides that side alone"
    );
}

#[test]
fn negative_padding_is_refused() {
    let cfg: AppearanceConfig =
        toml::from_str("[terminal]\npadding = -3.0\npadding_right = -1.5\n")
            .expect("negative terminal padding toml should parse");
    assert_eq!(
        cfg.terminal.effective_terminal_padding(),
        TerminalPadding {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        },
        "clamped at 0 — the grid cannot crop into the frame"
    );
}
