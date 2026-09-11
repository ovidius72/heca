//! The guard that stops a widget capability from becoming unreachable again.
//!
//! `Input::placeholder` and `ScrollRegion`'s horizontal axis existed for months and could not be
//! set from a description. Nothing failed, because nothing was checking — the declarative surface
//! was a hand-written list somewhere else, and a list nobody is forced to update falls behind.
//!
//! This reads the widget sources and fails when an `impl` block full of builders has no generated
//! surface. It is a lint, not a unit test: it inspects code rather than behaviour, on purpose,
//! because the defect it guards against is *absence* — and absence is invisible to a test that
//! only exercises what exists.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Types whose builders are deliberately not reachable from a description, each with the reason.
///
/// This is an **exceptions** list, so it fails closed: a new widget is required to have a surface
/// unless someone consciously excuses it here and says why. That is the opposite of the list that
/// caused the original defect, which required someone to remember to *add* to it.
const NO_SURFACE_REQUIRED: &[(&str, &str)] = &[
    ("ScrollBar", "host-only: its state is live host signals, which static data cannot drive"),
    ("Flex", "layout-only container; everything it takes comes from Style.layout"),
    ("Surface", "same — a bare styled box, no properties of its own"),
    ("Visibility", "wrapper around a signal"),
    ("ToastStack", "host-owned queue, driven by the notification store"),
    ("CommandPalette", "host-owned overlay, fed by the command registry"),
    ("ContextMenu", "host-owned overlay: it holds a Menu and shows it; the Menu's items are the property surface"),
    ("Menu", "its rows are MenuItem values carrying closures, which static data cannot supply — the items are the property surface"),
    ("KeyHint", "host-owned overlay, targets come from the hint registry"),
    ("Dialog", "host-owned overlay; body/actions arrive as realized subtrees"),
    ("Grid", "track templates are List-valued and handled by the Grid arm"),
    ("Spinner", "no configurable properties"),
    ("ToastSpec", "a value describing a toast, not a widget; the Toast built from it carries the properties"),
    ("ToastAction", "one entry in a ToastSpec's list, not a widget. A DESCRIBED toast declares its actions as children (an ordinary described Button per action), so the description path never meets this type"),
    ("ActiveMarker", "an enum, not a widget"),
    ("GridCell", "a value describing one cell, and everything it carries is a live host signal — a selection light and a hover — which static data cannot supply"),
];

fn widgets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widgets")
}

/// **Every widget source, however it is filed.** A widget family is a folder with one file per
/// member (`overlay/`, `toast/`), and a walk that read only the top level would quietly stop
/// checking a widget the day it was split into one — which is the moment its property surface is
/// most likely to drift. `widgets/mod.rs` is skipped because it only re-exports; a family's own
/// `mod.rs` is that family's widget and is read like any other file.
fn widget_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, top: bool, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).expect("widgets dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, false, out);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if top && name == "mod.rs" {
                continue;
            }
            // Name a family member by its folder, so a failure says `toast/stack.rs`.
            let label = match path.parent().filter(|_| !top).and_then(|p| p.file_name()) {
                Some(folder) => format!("{}/{name}", folder.to_string_lossy()),
                None => name,
            };
            out.push((label, std::fs::read_to_string(&path).expect("read widget source")));
        }
    }
    let mut out = Vec::new();
    walk(&widgets_dir(), true, &mut out);
    out
}

/// `pub fn name(mut self, ..) -> Self` — the builder shape. Accessors taking `&self` are not part
/// of the property surface and are ignored.
fn is_builder(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("pub fn ")
        && (t.contains("(mut self,") || t.contains("(self,"))
        && t.contains("-> Self")
}

/// Every `impl <Type> {` block that contains at least one builder, and whether it carries a
/// generated surface.
fn impl_blocks_with_builders(src: &str) -> Vec<(String, bool)> {
    let lines: Vec<&str> = src.lines().collect();
    let mut found = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("impl ") else { continue };
        let Some(ty) = rest.strip_suffix(" {") else { continue };
        // Skip trait impls (`impl Trait for Type`) and generics — only inherent blocks matter.
        if ty.contains(" for ") || ty.contains('<') || !ty.chars().all(|c| c.is_alphanumeric()) {
            continue;
        }
        let annotated = i > 0 && lines[i - 1].contains("heca_grid_ui_macros::props");
        let has_builder = lines[i..]
            .iter()
            .take_while(|l| !l.starts_with('}'))
            .any(|l| is_builder(l));
        if has_builder {
            found.push((ty.to_string(), annotated));
        }
    }
    found
}

/// Every widget whose builders a description could plausibly want has a generated property
/// surface — or an explicit, reasoned exception.
///
/// When this fails, the fix is almost always one line: put `#[heca_grid_ui_macros::props]` on the
/// impl block and `#[heca_grid_ui_macros::prop]` on the builders that carry data. Add to
/// `NO_SURFACE_REQUIRED` only when the widget genuinely cannot be driven by static data, and say
/// why — the reason is the point.
#[test]
fn every_builder_bearing_widget_has_a_generated_property_surface() {
    let excused: BTreeSet<&str> = NO_SURFACE_REQUIRED.iter().map(|(t, _)| *t).collect();
    let mut missing = Vec::new();

    for (file, src) in widget_sources() {
        for (ty, annotated) in impl_blocks_with_builders(&src) {
            if !annotated && !excused.contains(ty.as_str()) {
                missing.push(format!("  {ty} ({file})"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "these widgets have builder methods but no generated property surface, so a description \
         cannot reach them — exactly how Input::placeholder and ScrollRegion's second axis went \
         missing.\n\n{}\n\nAdd #[heca_grid_ui_macros::props] to the impl block and \
         #[heca_grid_ui_macros::prop] to the data-carrying builders, or add an entry to \
         NO_SURFACE_REQUIRED in this file with the reason it cannot be driven by static data.",
        missing.join("\n"),
    );
}

/// The exceptions list stays honest: every name in it must still exist. A stale entry would
/// silently excuse a *different* widget later if the name were reused.
#[test]
fn every_exception_still_names_a_real_type() {
    let sources: String = widget_sources().into_iter().map(|(_, src)| src).collect();
    let stale: Vec<&str> = NO_SURFACE_REQUIRED
        .iter()
        .map(|(t, _)| *t)
        .filter(|t| {
            !sources.contains(&format!("pub struct {t}")) && !sources.contains(&format!("pub enum {t}"))
        })
        .collect();
    assert!(
        stale.is_empty(),
        "NO_SURFACE_REQUIRED names types that no longer exist: {stale:?} — remove them, or a \
         future type reusing the name would be excused without anyone deciding that.",
    );
}


/// **Nothing writes `style.layout.hidden` by hand.**
///
/// `hidden` is the engine's `display: none`, so flipping it moves every sibling — it is a *layout*
/// change, and the pass that re-places them has to be asked for. `Base::set_hidden` does both, and
/// only asks when the value actually changed (so a widget syncing itself inside `remeasure` cannot
/// request a pass every pass).
///
/// Written by hand it is right half the time and silent the other half: a dock's rows ended up
/// painted on top of each other and a sidebar row stayed collapsed with its content already
/// arrived, both because the value was correct and nobody had moved anything. Thirteen call sites
/// carried that copy; this is what stops a fourteenth.
#[test]
fn nothing_sets_hidden_without_asking_for_the_layout_it_needs() {
    let mut offenders = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            // `style.rs` defines the field; `component.rs` holds the one setter allowed to write it.
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if name == "style.rs" || name == "component.rs" {
                continue;
            }
            let src = std::fs::read_to_string(&path).expect("utf-8");
            for (i, line) in src.lines().enumerate() {
                if line.contains("layout.hidden =") {
                    offenders.push(format!("{}:{}  {}", path.display(), i + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these write `layout.hidden` directly, so they change the layout without asking for the \
         pass that re-places everything after them. Use `Base::set_hidden(bool)`:\n  {}",
        offenders.join("\n  "),
    );
}
