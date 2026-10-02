//! **UI code does not write a size, a space, a scale or a delay as a number.**
//!
//! A number at a call site is a look nobody can change: not the theme, not the user's config, not
//! the font size (a `gap(6.0)` stays six pixels when the text doubles, and the spacing around it
//! no longer fits). The library already has the vocabulary — a step of the theme's rhythm
//! (`Spacing`), a size variant (`WidgetSize`), a quick tooltip (`Tooltip::quick`) — and it scales
//! with the font. A literal slipped into `providers/`, `chrome/` and `components/` anyway, and was
//! caught by a person reading, not by anything that could fail.
//!
//! This is what fails. Same shape as `surface_drift.rs`: a rule that lives only in AGENTS.md is
//! found out after it has been broken a dozen times.

use std::path::{Path, PathBuf};

/// The UI code the rule covers: whatever draws, lays out or styles. (`app/` and `handlers/` move
/// state and carry no look.)
const UI_DIRS: &[&str] = &["providers", "chrome", "components"];

/// A builder that takes a size, a space, a scale or a time. A number inside its parentheses is the
/// defect; a step (`Spacing::Sm`), a variant (`WidgetSize::Small`) or a constant passed in is not.
const BUILDERS: &[&str] = &[
    ".gap(",
    ".padding(",
    ".padding_x(",
    ".padding_y(",
    ".padding_xy(",
    ".padding_left(",
    ".padding_right(",
    ".padding_top(",
    ".padding_bottom(",
    ".margin(",
    ".margin_x(",
    ".margin_y(",
    ".margin_xy(",
    ".size(",
    ".font_size(",
    ".font_scale(",
    ".radius(",
    ".delay(",
    ".tooltip_delay(",
    ".width(",
    ".height(",
    ".min_width(",
    ".min_height(",
    ".max_width(",
    ".max_height(",
    "Length::Px(",
    "Space::Px(",
];

fn ui_sources() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = UI_DIRS.iter().map(|d| root.join(d)).collect();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let label = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((label, std::fs::read_to_string(&path).expect("readable")));
            }
        }
    }
    out
}

/// Is this file test code? Tests lay out fixtures in pixels on purpose: they have no theme.
fn is_test_file(label: &str) -> bool {
    let name = label.rsplit('/').next().unwrap_or(label);
    name.contains("test") || label.contains("/tests/")
}

/// Does `after` (the text right after a builder's opening parenthesis) start with a number?
fn starts_with_number(after: &str) -> bool {
    let after = after.trim_start().trim_start_matches('-');
    after.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// Every `file:line` that passes a number to a size builder. A file's tests are the part after its
/// `#[cfg(test)]`, which this stops at.
fn offenders() -> Vec<String> {
    let mut out = Vec::new();
    for (file, src) in ui_sources() {
        if is_test_file(&file) {
            continue;
        }
        for (i, line) in src.lines().enumerate() {
            if line.trim_start().starts_with("#[cfg(test)]") {
                break;
            }
            if line.trim_start().starts_with("//") {
                continue;
            }
            for builder in BUILDERS {
                let mut rest = line;
                while let Some(at) = rest.find(builder) {
                    rest = &rest[at + builder.len()..];
                    if starts_with_number(rest) {
                        out.push(format!("heca/src/{file}:{}  {}", i + 1, line.trim()));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn ui_code_names_its_sizes_instead_of_writing_numbers() {
    let found = offenders();
    assert!(
        found.is_empty(),
        "these pass a number to a size, space, scale or delay builder:\n  {}\n\n\
         Use the library's vocabulary, which scales with the font and can be themed:\n  \
         - a gap or padding: a step — `.gap(Spacing::Sm)`, `.padding(Spacing::Md)` \
           (Hairline, Xs, Sm, Md, Lg)\n  \
         - the size of text and icons: a variant — `.size(WidgetSize::Small)` on the row; an \
           icon with no size of its own takes the font\n  \
         - a tooltip's rest: `Tooltip::quick()`, or the default\n  \
         - a width or height: a share — `Length::Percent`, `.grow(..)`, or let the content decide\n\
         If the library has no step for what you need, add one to heca-grid-ui (and the theme) \
         instead of a number here — see AGENTS.md § 0, rule 2.",
        found.join("\n  "),
    );
}

/// Named numbers that are **not a look** — a limit on an input, say — and so have no theme or
/// config key to move to. Each says why. A look constant does not belong here: it goes to the
/// theme or the config, with today's value as its default.
const NOT_A_LOOK: &[(&str, &str, &str)] = &[(
    "chrome/terminal/input.rs",
    "MAX_UNITS",
    "a clamp on one wheel event's delta, so a bogus device value cannot fling the scrollback",
)];

/// Every `file:line` that gives a UI constant a number — `const GAP: f32 = 6.0;` is the same
/// hard-coded look as `.gap(6.0)`, one step removed and with a name.
fn named_numbers() -> Vec<String> {
    let mut out = Vec::new();
    for (file, src) in ui_sources() {
        if is_test_file(&file) {
            continue;
        }
        for (i, line) in src.lines().enumerate() {
            if line.trim_start().starts_with("#[cfg(test)]") {
                break;
            }
            let t = line.trim_start();
            let Some(rest) = t
                .strip_prefix("const ")
                .or_else(|| t.strip_prefix("pub const "))
                .or_else(|| t.strip_prefix("pub(crate) const "))
                .or_else(|| t.strip_prefix("pub(super) const "))
            else {
                continue;
            };
            let Some((name, ty)) = rest.split_once(':') else {
                continue;
            };
            let numeric =
                ty.trim_start().starts_with("f32 =") || ty.trim_start().starts_with("f64 =");
            if numeric
                && !NOT_A_LOOK
                    .iter()
                    .any(|(f, n, _)| *f == file && *n == name.trim())
            {
                out.push(format!("heca/src/{file}:{}  {}", i + 1, t));
            }
        }
    }
    out
}

#[test]
fn ui_code_has_no_named_look_numbers() {
    let found = named_numbers();
    assert!(
        found.is_empty(),
        "these name a number that is a look:\n  {}\n\nA look is the theme's or the user's to \
         change, not a constant in the app: a spacing step (`Spacing`), a size variant \
         (`WidgetSize`), a theme token (heca-theme `Theme`) or a config key \
         (`heca-config/src/appearance.rs`, with its default in config.default.toml). If it is not a \
         look at all — a limit on an input — add it to NOT_A_LOOK with the reason.",
        found.join("\n  "),
    );
}

/// The allow-list stays honest: an entry for a constant that no longer exists is an excuse left
/// for the next one to reuse the name.
#[test]
fn the_not_a_look_list_names_only_what_still_exists() {
    let sources = ui_sources();
    for (file, name, _) in NOT_A_LOOK {
        let present = sources
            .iter()
            .any(|(f, src)| f == file && src.contains(&format!("const {name}: f32")));
        assert!(
            present,
            "{file} no longer has `const {name}`: remove it from NOT_A_LOOK"
        );
    }
}
