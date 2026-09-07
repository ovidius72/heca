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

use std::path::{Path, PathBuf};

/// The surfaces that predate the rule. Each one is a place a thing on screen lives outside the
/// tree, and each is removed by P097(F003).
///
/// **The toast stack came off this list** (F003/P097/T494): it is placed in the window root with
/// `chrome::place_surface`, so the one walk delivers its pointer events and its × works. It is the
/// worked example for the three that remain.
///
/// **Do not add to this list.** Adding an entry is the moment to read
/// `docs/surface-compositor.md` § 0 instead — a surface belongs in the tree, and `Overlay` already
/// gives you modal capture, fall-through, nesting, animation and keyboard.
const KNOWN_REGISTRY_SURFACES: &[&str] = &[
    // the command palette
    "chrome/palette.rs",
    // OverlayHost: open_modal and open_dropdown
    "chrome/overlay.rs",
    // the exposé
    "chrome/expose/mod.rs",
];

/// How a surface gets registered instead of placed.
const REGISTRY_DOORS: &[&str] = &["layers.insert(", "layers.add_named(", "layers.add_view("];

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `.rs` under `heca/src`, labelled by its path relative to `src` so a failure names
/// something you can open.
fn sources() -> Vec<(String, String)> {
    let root = src_root();
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "rs") {
                let label = path
                    .strip_prefix(&root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let src = std::fs::read_to_string(&path).expect("readable");
                out.push((label, src));
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
    let mut offenders = Vec::new();
    for (file, src) in sources() {
        // The registry's own module is where these doors are defined and tested.
        if file.starts_with("chrome/layers/") {
            continue;
        }
        if KNOWN_REGISTRY_SURFACES.contains(&file.as_str()) {
            continue;
        }
        for (i, line) in src.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if REGISTRY_DOORS.iter().any(|door| line.contains(door)) {
                offenders.push(format!("heca/src/{file}:{}", i + 1));
            }
        }
    }
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
    let files: Vec<(String, String)> = sources();
    let mut stale = Vec::new();
    for known in KNOWN_REGISTRY_SURFACES {
        let still_registers = files.iter().any(|(file, src)| {
            file == known
                && src
                    .lines()
                    .filter(|l| !l.trim_start().starts_with("//"))
                    .any(|l| REGISTRY_DOORS.iter().any(|door| l.contains(door)))
        });
        if !still_registers {
            stale.push(*known);
        }
    }
    assert!(
        stale.is_empty(),
        "these no longer register a surface — remove them from KNOWN_REGISTRY_SURFACES so a future \
         file reusing the path is not excused without anyone deciding that: {stale:?}",
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
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chrome/overlay.rs"),
    )
    .expect("read the overlay host");

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
