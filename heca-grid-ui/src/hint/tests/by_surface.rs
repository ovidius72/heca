//! **Whose targets are whose** — the grouping a host with layered surfaces needs, so no host
//! writes it twice.

use crate::builders::Parent as _;
use crate::component::Component;
use crate::hint::*;
use crate::widgets::Flex;
use heca_core::layout::Size;

/// A page with a surface seated above it, each holding one pickable widget — the shape every
/// host with layers has.
fn page_with_a_seated_surface() -> Flex {
    use crate::widgets::Button;

    let mut surface = Flex::column().child(Button::new("in the surface").on_click(|| {}));
    surface.base_mut().key = Some("surface:1".into());
    surface.base_mut().surface = true;

    let mut root = Flex::column()
        .child(Flex::column().child(Button::new("on the page").on_click(|| {})))
        .child(surface);
    crate::LayoutEngine::new().compute(&mut root, Size::new(800.0, 600.0));
    root
}

/// **A widget is collected once, under the surface that owns it**.
///
/// The host used to collect the whole tree and then each seated surface again, so a widget
/// inside one carried two candidates — each drawing its own letter. Grouping is what removes
/// the second walk, and it is here rather than in the host so no future host writes it again.
///
/// The assertion is **where each target lands**, never how many there are in total: a total is
/// satisfied by collecting the right number of the wrong things, which is what the doubled
/// version did.
#[test]
fn a_widget_is_collected_once_under_the_surface_that_owns_it() {
    let root = page_with_a_seated_surface();
    let groups = collect_hints_by_surface(&root);

    let mut seen: Vec<(Vec<usize>, Vec<Vec<usize>>)> = groups
        .iter()
        .map(|g| {
            (
                g.path.clone(),
                g.targets.iter().map(|(p, _)| p.clone()).collect(),
            )
        })
        .collect();
    for (_, t) in &mut seen {
        t.sort();
    }
    assert_eq!(
        seen,
        vec![(vec![1], vec![vec![1, 0]]), (vec![], vec![vec![0, 0]]),],
        "front → back: the surface owning its own widget, then the page owning its own; \
         each target once, addressed from the ROOT",
    );
}

/// **The page is the group with no path**, so a host tells it from a surface structurally
/// rather than by matching a name — and gets the surface's key and box handed to it, instead of
/// walking back down the tree to find them.
#[test]
fn the_page_is_the_group_with_no_path_and_a_surface_carries_its_own_key() {
    let root = page_with_a_seated_surface();
    let groups = collect_hints_by_surface(&root);

    let page = groups.iter().find(|g| g.path.is_empty()).expect("the page");
    assert_eq!(page.key, None, "the page is named by being the page");

    let surface = groups
        .iter()
        .find(|g| !g.path.is_empty())
        .expect("the surface");
    assert_eq!(surface.key.as_deref(), Some("surface:1"));
    assert!(
        surface.bounds.size.w > 0.0,
        "its laid-out box comes back too, so a host never re-walks for it",
    );
}

/// **A target belongs to the DEEPEST surface enclosing it.** A menu inside a dialog groups
/// under the menu, so a host never assumes surfaces are direct children of the root — which is
/// what reading the first step of a path would assume.
#[test]
fn a_nested_surface_owns_its_own_targets() {
    use crate::widgets::Button;

    let mut inner = Flex::column().child(Button::new("in the menu").on_click(|| {}));
    inner.base_mut().key = Some("menu".into());
    inner.base_mut().surface = true;

    let mut outer = Flex::column()
        .child(Button::new("in the dialog").on_click(|| {}))
        .child(inner);
    outer.base_mut().key = Some("dialog".into());
    outer.base_mut().surface = true;

    let mut root = Flex::column().child(outer);
    crate::LayoutEngine::new().compute(&mut root, Size::new(800.0, 600.0));

    let groups = collect_hints_by_surface(&root);
    let owner = |k: &str| {
        groups
            .iter()
            .find(|g| g.key.as_deref() == Some(k))
            .unwrap_or_else(|| panic!("no group for {k}"))
    };
    assert_eq!(
        owner("menu").targets.len(),
        1,
        "the menu owns its own button"
    );
    assert_eq!(
        owner("dialog").targets.len(),
        1,
        "the dialog owns only what is not inside the menu",
    );
    assert_eq!(
        groups.first().map(|g| g.key.as_deref()),
        Some(Some("menu")),
        "front → back: the nested surface is in front of the one holding it",
    );
}

/// **The line a plugin author writes** (⭐⭐ RULE ZERO).
///
/// A surface says what it obscures with one builder each, on its own widget — no registry, no
/// id, no host-private type — and a host reads both back off the tree it already walks. While
/// these lived as fields on a host registry entry, a plugin could declare them only by reaching
/// a host call, so its overlay was a second-class version of one of heca's own.
#[test]
fn a_surface_declares_what_it_obscures_with_one_line_on_the_widget() {
    use crate::builders::PresenceExt as _;
    use crate::widgets::Button;

    let mut panel = Flex::column()
        .child(Button::new("in my panel").on_click(|| {}))
        .lock(true);
    // Taking the keyboard is NOT written: an overlay holds focus while it is open, and that is
    // how it takes keys. A bare `Flex` has no such story, so this test says so explicitly —
    // which is the one case the override exists for.
    panel.base().focus(false);
    panel.base_mut().key = Some("myplugin.panel".into());
    panel.base_mut().surface = true;

    let mut root = Flex::column().child(panel);
    crate::LayoutEngine::new().compute(&mut root, Size::new(800.0, 600.0));

    let group = collect_hints_by_surface(&root)
        .into_iter()
        .find(|g| g.key.as_deref() == Some("myplugin.panel"))
        .expect("the plugin's panel owns its own targets");
    assert!(
        group.lock,
        "what the widget declared is what the host reads back, with nothing registered",
    );
    assert!(
        group.holds_keyboard,
        "and taking the keyboard is READ from the tree — the surface holds focus, so it is \
         the active context without anyone declaring it",
    );
}

/// The counterpart, so the rule above cannot be satisfied by declaring everything: a surface
/// that says nothing obscures nothing. That is the toast stack — it spans the window to
/// position its cards in a corner, and suppresses no letters anywhere.
#[test]
fn a_surface_that_declares_nothing_obscures_nothing() {
    use crate::widgets::Button;

    let mut ambient = Flex::column().child(Button::new("a toast").on_click(|| {}));
    ambient.base_mut().key = Some("heca.notifications".into());
    ambient.base_mut().surface = true;

    let mut root = Flex::column().child(ambient);
    crate::LayoutEngine::new().compute(&mut root, Size::new(800.0, 600.0));

    let group = collect_hints_by_surface(&root)
        .into_iter()
        .find(|g| !g.path.is_empty())
        .expect("the stack is a surface like any other");
    assert!(
        !group.lock && !group.holds_keyboard,
        "spanning the window is how it POSITIONS its cards, not a claim on the screen — and \
         holding no focus is how it says it is not taking the keyboard",
    );
}

/// **A named node's words can be rewritten without rebuilding anything**.
///
/// This is what lets a host change what a retained tree *says* while keeping the tree: its
/// identity, its layout and every widget in it survive, because the text is a signal.
///
/// The case it exists for: a pane header shows the foreground program and the branch. Both
/// change while you work, and while they were part of the header's identity every change threw
/// the bar away and built a new one — and a fresh widget paints nothing until the layout walk
/// gives it a box, so the buttons blinked out and back.
#[test]
fn a_named_nodes_words_are_rewritten_in_place() {
    use crate::builders::IdentityExt as _;
    use crate::widgets::Label;

    use crate::reactive::SignalGet as _;

    let app = Label::new("zsh").key("hdr.seg:AppName");
    let branch = Label::new("main").key("hdr.seg:GitBranch");
    // The signals, taken before the labels move into the tree — what the widgets will say.
    let (app_text, branch_text) = (app.text_signal(), branch.text_signal());
    let root = Flex::column().child(app).child(branch);

    assert!(set_text_by_key(&root, "hdr.seg:AppName", "cargo"));
    assert_eq!(
        app_text.get_untracked(),
        "cargo",
        "the named node says the new words"
    );
    assert_eq!(
        branch_text.get_untracked(),
        "main",
        "and nothing else was touched",
    );
    assert!(
        !set_text_by_key(&root, "hdr.seg:Nothing", "x"),
        "a name nothing answers to writes nothing, and says so",
    );
}
