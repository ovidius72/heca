//! **A new surface does not get added to the registry.**
//!
//! A link in AGENTS.md does not stop anyone. `docs/surface-compositor.md` was already marked
//! "required reading before adding any layer, surface, overlay/modal, exposé, or a button on a new
//! surface", in bold, and the notification toast was mounted through the registry anyway — where it
//! received no pointer events at all, so its close button did nothing and hovering it highlighted
//! the pane behind. Nothing failed. Nothing warned.
//!
//! This is what warns. Same shape as `heca-grid-ui/tests/prop_drift.rs`, which stopped a fourteenth
//! hand-written `layout.hidden` after thirteen had already gone in.

mod common;

/// The surfaces that predate the rule, named by the **function** that registers each one. Each is a
/// place a thing on screen lives outside the tree, and each one is meant to move into the tree.
///
/// Named by function rather than by file, so moving a file into another folder neither breaks this
/// list nor quietly excuses whatever else ends up in that file.
///
/// **The toast stack came off this list**: it is placed in the window root with
/// `chrome::place_surface`, so the one walk delivers its pointer events and its × works. It is the
/// worked example for the ones that remain.
///
/// **Do not add to this list.** Adding an entry is the moment to read
/// `docs/surface-compositor.md` § 0 instead — a surface belongs in the tree, and `Overlay` already
/// gives you modal capture, fall-through, nesting, animation and keyboard.
const KNOWN_REGISTRY_SURFACES: &[&str] = &[
    // the command palette
    "open_command_palette",
    // OverlayHost: a modal, a dropdown menu, and a layer described as data
    "open_modal",
    "insert_menu_layer",
    "open_view_layer",
    // the one door a *named* layer is registered through — the exposé, and every layer another
    // crate adds with an extension's `.layer(..)`
    "register_named_layer",
];

/// How a surface gets registered instead of placed.
const REGISTRY_DOORS: &[&str] = &["layers.insert(", "layers.add_named(", "layers.add_view("];

/// The code lines of `src`, numbered from 1, with a method chain joined onto the line it continues.
///
/// rustfmt puts `state` and `.layers` and `.insert(..)` on lines of their own as soon as a call
/// is long enough, so a guard that matched one line at a time went blind the first time a file
/// was formatted. Comments are dropped: a door named in prose is not a door used.
fn code_lines(src: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        match out.last_mut() {
            Some((_, joined)) if trimmed.starts_with('.') => joined.push_str(trimmed),
            _ => out.push((i + 1, line.to_string())),
        }
    }
    out
}

/// The name of the function whose body holds line `line_no` (1-based) of `src`: the nearest
/// `fn name(` above it.
fn enclosing_fn(src: &str, line_no: usize) -> Option<String> {
    src.lines().take(line_no).collect::<Vec<_>>().iter().rev().find_map(|l| {
        let t = l.trim_start();
        let at = t.find("fn ")?;
        let head = &t[..at];
        if !head.is_empty() && !head.ends_with(' ') {
            return None;
        }
        let name: String = t[at + 3..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        (!name.is_empty() && t[at + 3 + name.len()..].starts_with(['(', '<'])).then_some(name)
    })
}

/// The registry's own module — the folder holding `struct LayerRegistry` — where the doors are
/// defined and tested.
fn registry_module() -> String {
    let (file, _) = common::source_defining("pub(crate) struct LayerRegistry");
    file.rsplit_once('/').map(|(dir, _)| format!("{dir}/")).unwrap_or(file)
}

/// Every registration outside the registry's own module, as (file, line, enclosing function).
fn registrations() -> Vec<(String, usize, Option<String>)> {
    let own = registry_module();
    let mut out = Vec::new();
    for (file, src) in common::all_sources() {
        if file.starts_with(&own) {
            continue;
        }
        for (line_no, line) in code_lines(&src) {
            if REGISTRY_DOORS.iter().any(|door| line.contains(door)) {
                out.push((file.clone(), line_no, enclosing_fn(&src, line_no)));
            }
        }
    }
    out
}

/// **A surface is placed in the tree, not registered.**
///
/// The registry is a parallel tree: it stores parent links and re-derives nesting every frame, and
/// the input walk does not read it. Anything mounted through it is painted and dead to the mouse.
///
/// If this fails, you are adding a fifth. Read `docs/surface-compositor.md` § 0 — in particular
/// § 0.4, which shows that the `Overlay` widget already does everything the registry does except
/// reach outside the chrome tree, and § 0.7 if you have concluded you genuinely have no choice.
#[test]
fn a_new_surface_is_placed_in_the_tree_not_registered() {
    let offenders: Vec<String> = registrations()
        .into_iter()
        .filter(|(_, _, func)| !func.as_deref().is_some_and(|f| KNOWN_REGISTRY_SURFACES.contains(&f)))
        .map(|(file, line, func)| format!("heca/src/{file}:{line} (in {})", func.as_deref().unwrap_or("?")))
        .collect();
    assert!(
        offenders.is_empty(),
        "these register a surface instead of placing it in the tree, so nothing will deliver \
         pointer events to it — it will paint correctly and be dead to the mouse, with no error:\
         \n  {}\n\nRead docs/surface-compositor.md § 0. An `Overlay` in the tree already gives you \
         modal capture, fall-through, nesting, animation and keyboard.",
        offenders.join("\n  "),
    );
}

/// **The list above does not grow, and does not go stale.**
///
/// An entry naming a file that no longer registers anything is an excuse left lying around for the
/// next file to reuse the name — the same reasoning `prop_drift.rs` applies to its own allowlist.
#[test]
fn the_known_surface_list_is_exactly_what_still_registers() {
    let found = registrations();
    let stale: Vec<&str> = KNOWN_REGISTRY_SURFACES
        .iter()
        .copied()
        .filter(|known| !found.iter().any(|(_, _, func)| func.as_deref() == Some(*known)))
        .collect();
    assert!(
        stale.is_empty(),
        "these no longer register a surface — remove them from KNOWN_REGISTRY_SURFACES so a future \
         function reusing the name is not excused without anyone deciding that: {stale:?}",
    );
}

/// **A menu row must be wired by the one conversion, whoever contributed it.**
///
/// Rows a component contributes to somebody else's menu were built by hand and wired to
/// `SubmitOverlay`, which resolves the overlay and hands the chosen id to a *completion*. A menu
/// presented from a widget's own declaration has no completion — nobody registers one — so a
/// contributed row opened fine and then did **nothing at all** when chosen.
///
/// Nothing failed. It went unnoticed while contributions were the rare case; the moment a pane's
/// whole menu arrived that way, every entry in it was dead.
///
/// A lint, like its neighbours: what it guards is a *second* way of doing something, and the suite
/// was green with the menu entries doing nothing at all.
#[test]
fn contributed_menu_rows_go_through_the_one_conversion() {
    let (_, src) = common::source_defining("fn merge_contributions(");

    let at = src
        .find("fn merge_contributions(")
        .expect("merge_contributions moved — move this guard with it");
    let body = &src[at..];
    let end = body.find("\n}\n").map(|i| i + 2).unwrap_or(body.len());
    let body = &body[..end];

    assert!(
        body.contains("menu_from_items("),
        "merge_contributions no longer goes through `menu_from_items`.\n\
         That is the one place an entry becomes a row — it wires the row to run the entry's own \
         action, dispatched by name through the central gate. Anything else builds a second kind \
         of row that looks identical and behaves differently.",
    );
    assert!(
        !body.contains("MenuItem::new()"),
        "merge_contributions builds a menu row by hand.\n\
         A row built here is wired by whatever this function remembers to wire, which is how \
         contributed rows ended up carrying an overlay-resolve that lands on a completion nobody \
         registers — they opened, and did nothing at all, and nothing failed.",
    );
}
