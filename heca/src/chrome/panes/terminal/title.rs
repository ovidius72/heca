//! Optional title composition and its own menu, shared by every terminal placement.

use std::rc::Rc;

use heca_grid_ui::{MenuExt, WidgetModel};
use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
use heca_grid_ui::widgets::{Caption, ContextMenu, Grid, Menu, MenuItem};

use super::{Terminal, shared::Shared, input::TerminalCommand};

pub(super) struct RenamePresentation {
    label: String,
    description: String,
    icon: Option<heca_grid_ui::Glyph>,
}

impl Default for RenamePresentation {
    fn default() -> Self {
        let mut presentation = Self { label: String::new(), description: String::new(), icon: None };
        presentation.update(&crate::actions::ActionCatalog::with_builtins());
        presentation
    }
}

impl RenamePresentation {
    fn update(&mut self, catalog: &crate::actions::ActionCatalog) {
        if let Some(action) = catalog.find("terminal_rename") {
            if self.label != action.label { self.label.clone_from(&action.label); }
            if self.description != action.description { self.description.clone_from(&action.description); }
        }
        self.icon = catalog.icon("terminal_rename");
    }
}

impl Terminal {
    /// Compose this terminal's optional title bar above its surface.
    ///
    /// Configure the terminal first; the returned `Grid` accepts ordinary layout builders.
    /// `false` reserves no header space. An empty or whitespace-only title also collapses it,
    /// and changing the title at runtime shows or hides the same label without rebuilding.
    /// Keep a cloned handle to run, search or rename it.
    /// The title and menu follow the same process in a pane, a dock or an overlay.
    ///
    /// ```
    /// let terminal = heca::Terminal::new("build").title("Build");
    /// let content = terminal.clone().header(true);
    /// ```
    pub fn header(self, visible: bool) -> Grid {
        let header = visible.then(|| WidgetModel::new(Caption::new("").with_text_signal(self.shared.title)));
        let menu = menu(self.shared.clone());
        super::content::layout(header, self).context_menu(menu)
    }

    pub(super) fn refresh_title(&self) {
        let title = self.shown_title().unwrap_or_default();
        if self.shared.title.get_untracked() != title { self.shared.title.set(title); }
    }

    pub(super) fn show_title(&self, title: Option<&str>, catalog: &crate::actions::ActionCatalog) {
        if self.shared.custom_title.borrow().as_deref() != title {
            *self.shared.custom_title.borrow_mut() = title.map(str::to_owned);
        }
        self.shared.rename.borrow_mut().update(catalog);
        self.refresh_title();
    }
}

pub(super) fn menu(shared: Rc<Shared>) -> impl Fn(heca_grid_ui::Point) -> ContextMenu {
    move |_| {
        let presentation = shared.rename.borrow();
        let target = shared.clone();
        let mut rename = MenuItem::new().label(&presentation.label).enabled(shared.id.get().is_some()).on_click(move || {
            if let Some(id) = target.id.get()
                && let Some(seams) = target.intents.borrow().as_ref()
            {
                (seams.command)(id, TerminalCommand::Rename);
            }
        });
        if let Some(icon) = presentation.icon { rename = rename.icon(icon); }
        ContextMenu::new("terminal").child(Menu::new(shared.title.get_untracked(), &presentation.description).child(rename))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::{Component, LayoutEngine, LayoutExt, Size, WidgetSize};
    use heca_grid_ui::widgets::Label;
    use crate::chrome::panes::terminal::{content::{BODY_AREA, HEADER_AREA}, testing, TerminalId};

    #[test]
    fn optional_header_owns_its_space_and_never_changes_the_terminal_identity() {
        let terminal = Terminal::new("optional_header_geometry").title("Logs");
        terminal.attach(TerminalId(71));
        for (visible, width, height) in [(false, 240.0, 180.0), (true, 240.0, 180.0), (true, 95.0, 100.0)] {
            let mut root = testing::laid_out_in(WidgetModel::new(terminal.clone().header(visible)), width, height);
            let body = heca_grid_ui::area(root.as_ref(), BODY_AREA).unwrap().base().bounds;
            assert!(body.loc.y >= 0.0 && body.loc.y + body.size.h <= height + 0.01);
            match heca_grid_ui::area(root.as_ref(), HEADER_AREA) {
                Some(header) => {
                    assert!(visible);
                    let bounds = header.base().bounds;
                    assert!(bounds.size.h > 0.0 && bounds.loc.y + bounds.size.h <= body.loc.y + 0.01);
                }
                None => { assert!(!visible); assert_eq!(body.size.h, height); }
            }
            terminal.show_title(Some("Renamed logs"), &crate::actions::ActionCatalog::with_builtins());
            LayoutEngine::new().compute(root.as_mut(), Size::new(width, height));
            assert_eq!(terminal.id(), Some(TerminalId(71)));
        }
    }

    #[test]
    fn user_title_wins_over_declaration_updates_until_explicitly_cleared() {
        let terminal = Terminal::new("shared_title_updates").title("Build");
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let mut root = terminal.clone().header(true);
        terminal.show_title(Some("Tests"), &catalog);
        let _same = terminal.clone().title("Configured build");
        assert_eq!(terminal.shown_title().as_deref(), Some("Tests"));
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 180.0));
        let theme = heca_grid_ui::Theme::default();
        let mut scene = heca_grid_ui::Scene::new();
        heca_grid_ui::paint_child(&root, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        assert!(scene.iter().any(|command| matches!(command, heca_grid_ui::DrawCommand::Text(text) if text.text == "Tests")));
        terminal.show_title(None, &catalog);
        assert_eq!(terminal.shown_title().as_deref(), Some("Configured build"));
    }

    #[test]
    fn a_titled_terminal_is_pickable_without_declarations_in_its_host() {
        use heca_grid_ui::Parent;
        for visible in [false, true] {
            let terminal = Terminal::new(if visible { "pick_titled" } else { "pick_untitled" });
            let mut root = testing::laid_out_in(WidgetModel::new(
                heca_grid_ui::widgets::Flex::column()
                    .child(Label::new("Notes"))
                    .child(terminal.clone().header(visible).grow(1.0)),
            ), 300.0, 200.0);
            let mut scene = heca_grid_ui::Scene::new();
            let theme = heca_grid_ui::Theme::default();
            heca_grid_ui::paint_child(root.as_ref(), &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
            let target = heca_grid_ui::collect_hints(root.as_ref()).into_iter().find(|(path, _)| {
                let mut node = root.as_ref();
                for step in path { node = node.base().children[*step].as_ref(); }
                node.base().focus_id() == terminal.shared.node.get()
            });
            let (path, _) = target.expect("the terminal must have a pick letter without caller wiring");
            assert!(heca_grid_ui::fire_hint(root.as_mut(), &path));
            assert_eq!(heca_grid_ui::keyboard_holder(root.as_ref()), Some(terminal.shared.node.get()));
        }
    }

    #[test]
    fn the_name_is_a_small_caption_aligned_with_its_upper_edge() {
        let terminal = Terminal::new("plain_title").title("Build");
        let root = testing::laid_out_in(WidgetModel::new(terminal.header(true)), 300.0, 200.0);
        let header = heca_grid_ui::area(root.as_ref(), HEADER_AREA).unwrap();
        let theme = heca_grid_ui::Theme::default();
        let mut scene = heca_grid_ui::Scene::new();
        heca_grid_ui::paint_child(header, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        let text = scene.iter().find_map(|command| match command {
            heca_grid_ui::DrawCommand::Text(text) if text.text == "Build" => Some(text),
            _ => None,
        }).unwrap();
        assert_eq!(text.align, heca_grid_ui::scene::TextAlign::Start);
        assert_eq!(text.size, theme.font_size * WidgetSize::Small.font_scale());
        assert_eq!(text.color, theme.colors.muted);
        let edge = scene.iter().find_map(|command| match command {
            heca_grid_ui::DrawCommand::Rect(rect) if rect.fill == theme.colors.border => Some(rect),
            _ => None,
        }).expect("the caption interrupts one upper-edge line, not a field or a second frame");
        assert!(edge.rect.loc.x >= text.rect.loc.x + text.rect.size.w);
        assert!((edge.rect.loc.y + edge.rect.size.h / 2.0 - (text.rect.loc.y + text.rect.size.h / 2.0)).abs() < 0.01,
            "edge={:?}, text={:?}", edge.rect, text.rect);
        assert!(edge.rect.loc.x + edge.rect.size.w > header.base().bounds.loc.x + header.base().bounds.size.w * 0.9,
            "the upper edge must span the available width, not stop after the name: {:?}", edge.rect);
        assert!(!scene.iter().any(|command| matches!(command,
            heca_grid_ui::DrawCommand::Brackets(_) | heca_grid_ui::DrawCommand::Line(_)
        )));
    }

    #[test]
    fn an_empty_title_collapses_and_reappears_without_rebuilding_the_terminal() {
        let terminal = Terminal::new("empty_title_geometry").title("");
        terminal.attach(TerminalId(72));
        let mut root = testing::laid_out_in(WidgetModel::new(terminal.clone().header(true)), 300.0, 200.0);
        for title in ["", "Build", "  ", "Tests"] {
            let _same = terminal.clone().title(title);
            LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
            let body = heca_grid_ui::area(root.as_ref(), BODY_AREA).unwrap().base().bounds;
            if title.trim().is_empty() {
                assert_eq!(body.loc.y, 0.0, "an empty title reserves no line");
                assert_eq!(body.size.h, 200.0);
            } else {
                assert!(body.loc.y > 0.0);
                assert!(body.size.h < 200.0);
            }
            assert_eq!(terminal.id(), Some(TerminalId(72)));
        }
    }

    #[test]
    fn caption_renames_stay_inside_narrow_layouts_without_moving_the_terminal() {
        for font in [12.0, 20.0, 30.0] {
            for width in [90.0, 240.0, 560.0] {
                let terminal = Terminal::new("caption_resize").title("Build");
                terminal.attach(TerminalId(73));
                let mut root = terminal.clone().header(true);
                let mut layout = LayoutEngine::new().base_font(font);
                layout.compute(&mut root, Size::new(width, 200.0));
                let before = heca_grid_ui::area(&root, BODY_AREA).unwrap().base().bounds;
                for title in ["Build", "A very long terminal name that must not overflow its container", "Logs"] {
                    let _same = terminal.clone().title(title);
                    layout.compute(&mut root, Size::new(width, 200.0));
                    let header = heca_grid_ui::area(&root, HEADER_AREA).unwrap();
                    let mut scene = heca_grid_ui::Scene::new();
                    let theme = heca_grid_ui::Theme { font_size: font, ..Default::default() };
                    heca_grid_ui::paint_child(header, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
                    for command in scene.iter() {
                        let rect = match command {
                            heca_grid_ui::DrawCommand::Text(text) => text.rect,
                            heca_grid_ui::DrawCommand::Rect(edge) => edge.rect,
                            _ => continue,
                        };
                        assert!(rect.loc.x >= 0.0 && rect.loc.x + rect.size.w <= width + 0.01,
                            "caption escaped at font={font}, width={width}: {rect:?}");
                    }
                    assert_eq!(heca_grid_ui::area(&root, BODY_AREA).unwrap().base().bounds, before);
                    assert_eq!(terminal.id(), Some(TerminalId(73)));
                }
            }
        }
    }

    #[test]
    fn rename_menu_uses_the_catalog_and_the_owning_terminal_not_keyboard_focus() {
        use std::cell::RefCell;
        let terminal = Terminal::new("rename_menu_target");
        terminal.attach(TerminalId(88));
        let sent = Rc::new(RefCell::new(Vec::new()));
        let seen = sent.clone();
        let (mut seams, _) = testing::recording();
        seams.command = Box::new(move |id, command| seen.borrow_mut().push((id, command)));
        terminal.bind(seams);
        let menu = menu(terminal.shared.clone())(heca_grid_ui::Point::new(0.0, 0.0));
        let expected = crate::actions::ActionCatalog::with_builtins().label("terminal_rename").unwrap().to_owned();
        assert_eq!(menu.menu().item_labels(), vec![Some(expected)]);
        menu.menu().clone().into_items()[0].activate();
        assert_eq!(*sent.borrow(), vec![(TerminalId(88), TerminalCommand::Rename)]);
        assert!(Terminal::unbound().base().context_menu.is_none(), "pane-owned terminals retain the enclosing pane menu");
    }
}
