//! Vocabulary coverage — the guard that keeps the mapper from rotting.

use heca_view::WidgetKind;

use super::*;

// ── Vocabulary coverage (the guard that keeps this from rotting) ──

/// **Every widget in the library can be described, or somebody said why not**.
///
/// The test below asks the vocabulary is *complete* — every kind realizes. This asks the other
/// direction, which is the one that actually failed: at one point **thirteen** library
/// widgets had no declarative form, and nothing anywhere said so. A plugin author found out by
/// not finding one. The census that discovered them was a person reading two crates by hand,
/// and a census run once is a census that is wrong a month later.
///
/// So the list is derived from the library's own exports, and every widget must be one of:
///
/// - a [`WidgetKind`] of the same name — it can be described;
/// - a **declaration** on any node rather than a thing you place (a tooltip, a menu, a hint
///   letter, visibility) — the wrapper survives for regions that are not widgets, but nobody
///   needs to name it to get the behaviour;
/// - **host-only by design**, with the reason.
///
/// Anything else is a widget a description cannot ask for, and this says so by name.
#[test]
fn every_library_widget_is_describable_or_deliberately_not() {
    /// Widgets with no `WidgetKind`, each with why. An entry is a deliberate decision, not a
    /// place to park an omission — read the three categories in the doc above before adding one.
    const NO_KIND: &[(&str, &str)] = &[
        // ── Declarations: the behaviour is on every node, so nothing needs naming ──
        (
            "Tooltip",
            "a declaration on any node; the wrapper is for regions",
        ),
        (
            "ContextMenu",
            "a declaration on any node, never a thing you place",
        ),
        ("KeyHint", "a declaration on any node: hint + placement"),
        (
            "Visibility",
            "two declarations on any node: visible, hidden",
        ),
        // ── Host-only by design ──
        (
            "ChromeRegion",
            "host-only: a region of the window, which the host owns",
        ),
        ("Pane", "host-only: the app's own concept, not a primitive"),
        ("PaneFrame", "host-only: the frame of the above"),
        (
            "FocusScope",
            "host-only: focus containment the host arranges",
        ),
        (
            "ScrollBar",
            "host-only: live host signals; realize refuses it",
        ),
        ("ToastStack", "host-only: it holds a queue the host drives"),
        // ── Not widgets: parts, values and helpers that happen to be exported ──
        (
            "GridCell",
            "not a widget: a CardGrid cell, built by realize",
        ),
        ("Menu", "not a widget: the data a menu declaration carries"),
        ("MenuItem", "not a widget: one entry of the above"),
        ("MenuEntry", "not a widget: one entry of the above"),
        ("MenuAnchor", "not a widget: where a menu opens"),
        (
            "EdgeLine",
            "not a widget: where a landing slot's line runs inside its box",
        ),
        ("Command", "not a widget: one entry of a command palette"),
        ("Tip", "not a widget: what a tooltip declaration carries"),
        ("Flex", "the arrangement: VStack/HStack are its names"),
        ("Container", "not a widget: a helper for building a `Flex`"),
        ("ScrollRegion", "described as `Scroll`"),
        ("ProgressBar", "described as `Progress`"),
        // ── Native code composes these; a description has no use for them ──
        (
            "Keyed",
            "a list that keeps its rows by name across rebuilds; a description already reconciles by key",
        ),
        // ── Opened through the host API, not placed in a tree ──
        //
        // `docs/chrome-and-ui.md` § 3.5 is the agreed plugin contract: a plugin opens these
        // with `app.overlay.openModal(..)` / `openDropdown(..)` and fires actions with
        // `app.actions.dispatch(..)`. They are not kinds a plugin constructs — it would then
        // own the scrim, the open state and the dismissal, and get the chosen button back
        // without the form data beside it. `Dialog` stays a widget **native** code composes.
        ("Dialog", "opened via app.overlay.openModal (§3.5)"),
        ("CommandPalette", "opened via the host API (§3.5)"),
    ];

    let exports = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../heca-grid-ui/src/widgets/mod.rs"),
    )
    .expect("the widget exports are where they are expected");

    // Every type name a `pub use` line publishes. Types, not values: a widget is a type, and
    // the lower-case items on these lines are constructors and constants.
    let published: std::collections::BTreeSet<String> = exports
        .lines()
        .filter(|l| l.trim_start().starts_with("pub use"))
        .flat_map(|l| {
            l.split(|c: char| !c.is_alphanumeric() && c != '_')
                .skip(1)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|n| n.chars().next().is_some_and(char::is_uppercase))
        .collect();
    assert!(
        published.len() > 30,
        "this guard has stopped reading the library's exports, which is worse than the gap it \
         exists for — it found only {}",
        published.len(),
    );

    // The value types (`ButtonVariant`, `Glyph`, `Display`, …) are vocabularies a widget reads,
    // not widgets. They are told apart by being named in a kind's properties rather than by a
    // list here, which would be one more thing to keep in step.
    let kinds: std::collections::BTreeSet<String> =
        WidgetKind::ALL.iter().map(|k| format!("{k:?}")).collect();

    let unreachable: Vec<&String> = published
        .iter()
        .filter(|n| !kinds.contains(*n))
        .filter(|n| !NO_KIND.iter().any(|(w, _)| w == *n))
        .filter(|n| !VALUE_TYPES.contains(&n.as_str()))
        .collect();
    assert!(
        unreachable.is_empty(),
        "these library widgets have no `WidgetKind` and no recorded reason, so a plugin can \
         neither name them nor find out why: {unreachable:#?}\n\nAdd a kind, or add the name \
         to NO_KIND with which of the three categories it is in.",
    );
}

/// Vocabularies a widget reads — enums and value types, not widgets. Listed once because two
/// guards ask the same question of the same exports.
const VALUE_TYPES: &[&str] = &[
    "AlertVariant",
    "BadgeVariant",
    "ButtonVariant",
    "LabelSide",
    "RegionMode",
    "Glyph",
    "NfGlyph",
    "ActiveMarker",
    "Ellipsis",
    "Orientation",
    "DotStatus",
    "RevealAlign",
    "ScrollAxes",
    "ScrollInfo",
    "ToastAction",
    "ToastPosition",
    "ToastSeverity",
    "ToastSpec",
    "Display",
    "TooltipSide",
    "HintPlacement",
    "KeyCap",
    "KeycapVariant",
    "HintStyle",
    "DEFAULT_LETTERS",
    // The dialog panel recipe — spacing steps every dialog-shaped surface reads, so a
    // described one is laid out identically to a native one.
    "DIALOG_PAD",
    "DIALOG_GAP",
    "DIALOG_BTN_GAP",
    "NamedAnimation",
];

/// **Every `WidgetKind` realizes to a live widget** — one that either holds the children it was
/// given or paints something. The fallback (an empty `Flex`) does neither, so a kind with no arm
/// fails here loudly instead of rendering nothing and being noticed months later by a plugin
/// author.
///
/// The single exception is `ScrollBar`, which is **host-only by design**: its state is live host
/// signals (`content_extent` / `viewport_extent` / `offset`), and static serializable data
/// fundamentally cannot drive a signal — a declarative one would render a dead control. The test
/// asserts it realizes to *nothing*, so that decision is pinned rather than merely documented.
#[test]
fn every_widget_kind_realizes_to_a_live_widget_except_the_host_only_ones() {
    use heca_core::layout::Size;
    use heca_grid_ui::{LayoutEngine, PaintCx, Scene, Theme};

    let theme = Theme::default();
    for &kind in WidgetKind::ALL {
        let mut widget = realize(
            &sample_node(kind),
            &Theme::default(),
            &noop_emitter(),
            &mut FormBindings::default(),
        );
        LayoutEngine::new().compute(widget.as_mut(), Size::new(400.0, 200.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            widget.paint(&mut cx);
        }
        let alive = !widget.base().children.is_empty() || !scene.is_empty();

        if kind == WidgetKind::ScrollBar {
            assert!(
                !alive,
                "ScrollBar is host-only: it must NOT realize (its state is a live host signal — \
                 a plugin uses Scroll instead)",
            );
        } else {
            assert!(
                alive,
                "{kind:?} realized to nothing — it needs a `realize` arm (or, if its state is a \
                 live host signal, to be documented as host-only like ScrollBar)",
            );
        }
    }
}
