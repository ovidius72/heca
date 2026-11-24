//! **The palette's lists, an open door** — `ext.palette_list("containers")`, the same for heca's own
//! lists, an extension's and a plugin's.
//!
//! ```ignore
//! let demo = heca::extension("demo");
//! demo.palette_list("containers")
//!     .sigil('%')
//!     .placeholder("Search containers")
//!     .items(move || containers().into_iter().map(|c| {
//!         PaletteItem::new(c.name())
//!             .description(c.image())
//!             .on_choose(Intent::new("demo.attach").arg("id", PropValue::Text(c.id())))
//!     }).collect())
//!     .render_item(|item, row| row.trailing(Badge::new("up")));
//!
//! // Adding to a list that exists — heca's sessions — is the same call on its name:
//! pro.palette_list("session").render_item(|item, row| match item.session() {
//!     Some(s) if is_pro(s) => row.trailing(Badge::accent("pro")),
//!     _ => row,
//! });
//! ```
//!
//! - A list is **named**; every call on the same name adds to the same list, from any extension, in
//!   any order. `sigil`, `placeholder` and `items` say what the list is (the last call wins);
//!   `render_item` adds — each one receives the row the one before returned, so additions add up.
//! - An item is a [`PaletteItem`]: a title, a description, an icon, keys, and what choosing it sends —
//!   always an `Intent`, so it passes the one action gate (policy, confirmation) every other act does.
//! - `render_item(|item, row| …)` gets the palette's standard row ([`PaletteRow`]): add something at
//!   the end of its first line ([`trailing`](PaletteRow::trailing)), or return a row of your own
//!   ([`PaletteRow::custom`]).
//! - **A plugin** writes the same calls with descriptions: `trailing_described` /
//!   `custom_described` take a `ViewNode`, which heca realizes into the same widget.

use std::cell::RefCell;
use std::rc::Rc;

use heca_grid_ui::WidgetModel;
use heca_grid_ui::builders::IntoComponent;
use heca_grid_ui::prelude::*;

use super::model::PaletteEntry;
use crate::app::interaction::InteractionIntent;
use crate::entry::service::SessionEntry;

/// **One item of a palette list** — what its row shows and what choosing it sends.
#[derive(Debug, Clone)]
pub struct PaletteItem {
    pub(super) entry: PaletteEntry,
    stands_for: Option<StandsFor>,
}

/// What an item of one of heca's own lists stands for, for an extension adding to its row.
#[derive(Debug, Clone)]
enum StandsFor {
    Session(SessionEntry),
}

impl PaletteItem {
    /// An item titled `title` — the text matched as you type.
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            entry: PaletteEntry {
                id: String::new(),
                rank_id: Some(title.clone()),
                pane: None,
                current: false,
                preselect: false,
                label: title,
                description: String::new(),
                description_path: false,
                detail: None,
                icon: None,
                shortcuts: Vec::new(),
                intent: None,
            },
            stands_for: None,
        }
    }

    /// The muted line under the title.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.entry.description = text.into();
        self
    }

    /// The icon before the title.
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.entry.icon = Some(glyph);
        self
    }

    /// A binding shown at the end of the row, written as in `keybindings.toml` (`"prefix+e"`,
    /// `"ctrl+shift+p"`). Call it once per binding.
    pub fn keys(mut self, binding: impl Into<String>) -> Self {
        self.entry.shortcuts.push(binding.into());
        self
    }

    /// **What choosing it sends** — an `Intent`, dispatched through the one action gate.
    pub fn on_choose(mut self, intent: crate::chrome::Intent) -> Self {
        self.entry.intent = Some(InteractionIntent::View(intent));
        self
    }

    /// The title.
    pub fn title(&self) -> &str {
        &self.entry.label
    }

    /// The session this item stands for, when it is a row of heca's session list.
    pub fn session(&self) -> Option<&SessionEntry> {
        match &self.stands_for {
            Some(StandsFor::Session(entry)) => Some(entry),
            None => None,
        }
    }

    /// An item of one of heca's own lists, from the row heca built.
    pub(super) fn of_entry(entry: PaletteEntry, session: Option<SessionEntry>) -> Self {
        Self {
            entry,
            stands_for: session.map(StandsFor::Session),
        }
    }
}

/// One part of a row an extension added: a widget, or a description heca realizes.
enum Part {
    Widget(WidgetModel),
    Described(heca_view::ViewNode),
}

/// **A palette row, as `render_item` receives and returns it** — the palette's standard row
/// (icon, title with the match drawn, description, keys), with what was added to it.
pub struct PaletteRow {
    trailing: Vec<Part>,
    custom: Option<Part>,
}

impl PaletteRow {
    fn standard() -> Self {
        Self {
            trailing: Vec::new(),
            custom: None,
        }
    }

    /// **Add something at the end of the row's first line**, after the title — a badge, a count,
    /// a status. Several sit side by side, in the order added; the title gives way to them.
    pub fn trailing(mut self, widget: impl IntoComponent) -> Self {
        self.trailing.push(Part::Widget(widget.into_component()));
        self
    }

    /// [`trailing`](Self::trailing) for a plugin: a description, realized into the same widget.
    pub fn trailing_described(mut self, node: heca_view::ViewNode) -> Self {
        self.trailing.push(Part::Described(node));
        self
    }

    /// **A row of your own**, in place of the standard one — the selection, the hover and the
    /// choice stay the palette's; the title is still what is matched.
    pub fn custom(widget: impl IntoComponent) -> Self {
        Self {
            trailing: Vec::new(),
            custom: Some(Part::Widget(widget.into_component())),
        }
    }

    /// [`custom`](Self::custom) for a plugin: a description, realized into the same widget.
    pub fn custom_described(node: heca_view::ViewNode) -> Self {
        Self {
            trailing: Vec::new(),
            custom: Some(Part::Described(node)),
        }
    }
}

/// Turns a description into the widget it describes, with the palette's theme and intent sink.
pub(super) type Realize = Rc<dyn Fn(&heca_view::ViewNode) -> Option<WidgetModel>>;
type Render = Rc<dyn Fn(&PaletteItem, PaletteRow) -> PaletteRow>;
type Items = Rc<dyn Fn() -> Vec<PaletteItem>>;

/// **What a list's items are** — `.items(..)` takes a list, a function giving one, or a signal.
pub trait IntoPaletteItems {
    fn into_items(self) -> Items;
}

impl IntoPaletteItems for Vec<PaletteItem> {
    fn into_items(self) -> Items {
        Rc::new(move || self.clone())
    }
}

impl<F: Fn() -> Vec<PaletteItem> + 'static> IntoPaletteItems for F {
    fn into_items(self) -> Items {
        Rc::new(self)
    }
}

impl IntoPaletteItems for Signal<Vec<PaletteItem>> {
    fn into_items(self) -> Items {
        Rc::new(move || self.get_untracked())
    }
}

/// One declared list.
#[derive(Default, Clone)]
pub(super) struct ListDecl {
    pub(super) name: String,
    pub(super) sigil: Option<char>,
    pub(super) placeholder: Option<String>,
    pub(super) search_descriptions: bool,
    pub(super) items: Option<Items>,
    /// For one of heca's own lists: its rows, from what the window holds when the palette opens —
    /// what an extension's `items` is for its list.
    pub(super) host_rows: Option<HostRows>,
    /// For one of heca's own lists: the action run each time it is entered, for a list whose rows
    /// are asked for (the server's notifications).
    pub(super) on_enter: Option<fn() -> crate::input::WmAction>,
    renders: Vec<Render>,
}

/// The rows of one of heca's own lists, from what the window holds.
pub(super) type HostRows = fn(&super::modes::RowSources<'_>) -> Vec<PaletteEntry>;

thread_local! {
    static LISTS: RefCell<Vec<ListDecl>> = const { RefCell::new(Vec::new()) };
}

fn with_list(name: &str, change: impl FnOnce(&mut ListDecl)) {
    LISTS.with(|lists| {
        let mut lists = lists.borrow_mut();
        let at = match lists.iter().position(|list| list.name == name) {
            Some(at) => at,
            None => {
                lists.push(ListDecl {
                    name: name.to_string(),
                    ..ListDecl::default()
                });
                lists.len() - 1
            }
        };
        change(&mut lists[at]);
    });
}

/// **Every list declared** — heca's own first, in the order they declare themselves (which makes
/// the first the default list), then the others in the order first named.
pub(super) fn declared() -> Vec<ListDecl> {
    let own = super::modes::declare_builtins();
    let mut lists = LISTS.with(|lists| lists.borrow().clone());
    lists.sort_by_key(|list| {
        own.iter()
            .position(|name| *name == list.name)
            .unwrap_or(own.len())
    });
    lists
}

/// Whether the list named `name` has rows from the window — one of heca's own, declared.
pub(super) fn has_host_rows(name: &str) -> bool {
    LISTS.with(|lists| {
        lists
            .borrow()
            .iter()
            .any(|list| list.name == name && list.host_rows.is_some())
    })
}

/// The list named `name`, if declared.
pub(super) fn find(name: &str) -> Option<ListDecl> {
    declared().into_iter().find(|list| list.name == name)
}

/// **A palette list**, returned by [`Extension::palette_list`](crate::Extension::palette_list).
pub struct PaletteList {
    name: String,
}

impl PaletteList {
    pub(crate) fn new(name: &str) -> Self {
        with_list(name, |_| {});
        Self {
            name: name.to_string(),
        }
    }

    /// The character that opens the palette in this list when typed first (`'%'`).
    pub fn sigil(self, sigil: char) -> Self {
        with_list(&self.name, |list| list.sigil = Some(sigil));
        self
    }

    /// What the empty query field says while this list is shown.
    pub fn placeholder(self, text: impl Into<String>) -> Self {
        let text = text.into();
        with_list(&self.name, |list| list.placeholder = Some(text));
        self
    }

    /// Also match the items' descriptions, not only their titles.
    pub fn search_descriptions(self, on: bool) -> Self {
        with_list(&self.name, |list| list.search_descriptions = on);
        self
    }

    /// **The items** — a list, a function returning one, or a signal holding one; read each time
    /// the palette opens.
    pub fn items(self, items: impl IntoPaletteItems) -> Self {
        let items = items.into_items();
        with_list(&self.name, |list| list.items = Some(items));
        self
    }

    /// heca's own lists: rows from what the window holds when the palette opens.
    pub(super) fn host_rows(self, rows: HostRows) -> Self {
        with_list(&self.name, |list| list.host_rows = Some(rows));
        self
    }

    /// heca's own lists: the action run each time the list is entered.
    pub(super) fn on_enter(self, action: fn() -> crate::input::WmAction) -> Self {
        with_list(&self.name, |list| list.on_enter = Some(action));
        self
    }

    /// **How an item's row looks** — `|item, row| …` gets the item and the row so far (the
    /// standard one, with whatever earlier calls added) and returns the row.
    pub fn render_item(
        self,
        render: impl Fn(&PaletteItem, PaletteRow) -> PaletteRow + 'static,
    ) -> Self {
        let render: Render = Rc::new(render);
        with_list(&self.name, |list| list.renders.push(render));
        self
    }
}

/// **The deprecated door**: [`Extension::palette`](crate::Extension::palette) — a widget or a
/// description at the end of a row. Both now go through [`PaletteList::render_item`].
pub struct PaletteRows {
    mode: String,
}

impl PaletteRows {
    pub(crate) fn new(mode: &str) -> Self {
        Self {
            mode: mode.to_owned(),
        }
    }

    /// Deprecated: `palette_list(mode).render_item(|item, row| row.trailing(..))`.
    #[deprecated(note = "use `palette_list(mode).render_item(|item, row| row.trailing(..))`")]
    pub fn to_raw<C: IntoComponent>(self, produce: impl Fn(&PaletteItem) -> Option<C> + 'static) {
        PaletteList::new(&self.mode).render_item(move |item, row| match produce(item) {
            Some(widget) => row.trailing(widget),
            None => row,
        });
    }

    /// Deprecated: `palette_list(mode).render_item(|item, row| row.trailing_described(..))`.
    #[deprecated(
        note = "use `palette_list(mode).render_item(|item, row| row.trailing_described(..))`"
    )]
    pub fn to_description(
        self,
        produce: impl Fn(&PaletteItem) -> Option<heca_view::ViewNode> + 'static,
    ) {
        PaletteList::new(&self.mode).render_item(move |item, row| match produce(item) {
            Some(node) => row.trailing_described(node),
            None => row,
        });
    }
}

/// What a row of `list` is made with, once `render_item` has had its say — `None` when nothing was
/// added to the standard row.
pub(super) struct Rendered {
    pub(super) trailing: Option<Rc<dyn Fn() -> WidgetModel>>,
    pub(super) custom: Option<Rc<dyn Fn() -> WidgetModel>>,
}

/// **Run `list`'s `render_item`s on `item`**, as ways to make the row's parts — made again each time
/// the row is built, since a widget can be shown once.
pub(super) fn rendered(list: &str, item: PaletteItem, realize: Realize) -> Rendered {
    let renders: Vec<Render> = LISTS.with(|lists| {
        lists
            .borrow()
            .iter()
            .find(|decl| decl.name == list)
            .map(|decl| decl.renders.clone())
            .unwrap_or_default()
    });
    if renders.is_empty() {
        return Rendered {
            trailing: None,
            custom: None,
        };
    }
    let item = Rc::new(item);
    let run = {
        let (item, realize) = (item.clone(), realize.clone());
        Rc::new(move || {
            let row = renders
                .iter()
                .fold(PaletteRow::standard(), |row, render| render(&item, row));
            let make = |part: Part| match part {
                Part::Widget(widget) => Some(widget),
                Part::Described(node) => realize(&node),
            };
            let trailing: Vec<WidgetModel> = row.trailing.into_iter().filter_map(make).collect();
            (trailing, row.custom.and_then(make))
        })
    };
    // Rendered once now to learn which parts there are; each part re-runs it when its row is built.
    let (trailing, custom) = run();
    Rendered {
        trailing: (!trailing.is_empty()).then(|| {
            let run = run.clone();
            Rc::new(move || {
                heca_grid_ui::widgets::Flex::row()
                    .gap(heca_grid_ui::style::Spacing::Xs)
                    .align("center")
                    .child(run().0)
                    .into()
            }) as Rc<dyn Fn() -> WidgetModel>
        }),
        custom: custom.is_some().then(|| {
            Rc::new(move || {
                run()
                    .1
                    .unwrap_or_else(|| heca_grid_ui::widgets::Flex::row().into())
            }) as Rc<dyn Fn() -> WidgetModel>
        }),
    }
}

#[cfg(test)]
pub(super) fn clear_for_tests() {
    LISTS.with(|lists| lists.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::service::{SessionKey, SessionState};
    use heca_grid_ui::widgets::{Command, CommandPalette};
    use heca_grid_ui::{Event, LayoutEngine, Size, WidgetIntent};

    fn realize() -> Realize {
        let theme = heca_grid_ui::Theme::default();
        let sink: crate::chrome::IntentEmitter = Rc::new(|_| {});
        Rc::new(move |node| {
            crate::chrome::realize(node, &theme, &sink, &mut Default::default()).ok()
        })
    }

    fn texts(model: &WidgetModel) -> Vec<String> {
        model
            .base()
            .children
            .iter()
            .filter_map(|child| child.text_summary())
            .collect()
    }

    fn session_item() -> PaletteItem {
        let session = SessionEntry::new(
            SessionKey::generate(),
            "work".into(),
            None,
            SessionState::Active,
        );
        PaletteItem::of_entry(PaletteItem::new("work").entry, Some(session))
    }

    /// **Every `render_item` on a list adds to the row the one before returned**, a widget and a
    /// description alike, in the order registered — and the deprecated door goes the same way.
    #[test]
    #[allow(deprecated)]
    fn render_items_add_up_in_order_and_the_old_door_goes_through_them() {
        clear_for_tests();
        assert!(
            rendered("session", session_item(), realize())
                .trailing
                .is_none(),
            "nothing added"
        );
        PaletteList::new("session").render_item(|item, row| match item.session() {
            Some(s) => row.trailing(heca_grid_ui::Label::new(format!("raw {}", s.name()))),
            None => row,
        });
        PaletteRows::new("session").to_description(|item| {
            item.session().map(|s| {
                heca_view::ViewNode::new(heca_view::WidgetId::Label)
                    .text(format!("described {}", s.name()))
            })
        });
        PaletteRows::new("session").to_raw(|_| None::<heca_grid_ui::Label>);
        PaletteList::new("pane")
            .render_item(|_, row| row.trailing(heca_grid_ui::Label::new("elsewhere")));
        let make = rendered("session", session_item(), realize())
            .trailing
            .expect("something was added to session rows");
        assert_eq!(texts(&make()), ["raw work", "described work"]);
        assert_eq!(
            texts(&make()),
            ["raw work", "described work"],
            "made again, the same"
        );
        clear_for_tests();
    }

    /// **An extension's list is a mode of the palette** — its sigil opens it, its items are the rows,
    /// its placeholder is what the empty field says while it is shown, and choosing a row is the
    /// item's own `on_choose`.
    #[test]
    fn an_extensions_list_is_a_palette_mode_with_its_own_placeholder() {
        clear_for_tests();
        PaletteList::new("demo.rooms")
            .sigil('%')
            .placeholder("Join a room")
            .items(vec![
                PaletteItem::new("lobby").on_choose(
                    crate::chrome::Intent::new("demo.join")
                        .arg("room", crate::chrome::PropValue::Text("lobby".into())),
                ),
                PaletteItem::new("kitchen"),
            ]);
        let chosen: Rc<RefCell<Vec<String>>> = Default::default();
        let command = {
            let chosen = chosen.clone();
            move |row: &PaletteEntry, _: &str| {
                let (chosen, id) = (chosen.clone(), row.id.clone());
                Command::new(row.label.clone(), move || {
                    chosen.borrow_mut().push(id.clone())
                })
            }
        };
        let session = heca_core::layout::testing::Windowed::new(Size::new(800.0, 600.0), 1.0);
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let owners = Default::default();
        let shortcuts = crate::chrome::ActionShortcuts::default();
        let programs = heca_config::programs::ProgramsConfig::default();
        let panes = crate::chrome::SharedChromeState::new(280.0, 260.0).panes;
        let sources = super::super::modes::RowSources {
            sessions: &[],
            follow: &[],
            notifications: &[],
            catalog: &catalog,
            owners: &owners,
            shortcuts: &shortcuts,
            layout: session.l(),
            programs: &programs,
            panes: &panes,
            add_process_name: false,
            last_focused: None,
            last_visited: None,
        };
        let (palette, rows) =
            super::super::modes::populate(CommandPalette::new(), &sources, &command, |_| {
                Box::new(|| {})
            });
        let rows: Vec<_> = rows
            .into_iter()
            .filter(|r| r.id.starts_with("demo.rooms#"))
            .collect();
        assert_eq!(
            rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
            ["lobby", "kitchen"]
        );
        assert!(
            matches!(&rows[0].intent, Some(InteractionIntent::View(intent)) if intent.action == "demo.join"),
            "choosing it sends its own intent"
        );
        let mut palette = super::super::with_placeholders(palette).default_open(true);
        let viewport = Size::new(1200.0, 800.0);
        for letter in "%kit".chars() {
            heca_grid_ui::dispatch(&mut palette, &Event::TextInput(letter.to_string()));
            LayoutEngine::new().compute(&mut palette, viewport);
        }
        heca_grid_ui::dispatch(&mut palette, &Event::Widget(WidgetIntent::Activate));
        assert_eq!(
            chosen.borrow().as_slice(),
            ["demo.rooms#1"],
            "the sigil opened the list and found kitchen"
        );
        clear_for_tests();
    }

    /// **heca's lists keep their order, whoever names them first** — an extension adding to the
    /// session rows before the palette ever opened does not make sessions the default list; heca's
    /// own come first, in their order, and an extension's list after them.
    #[test]
    fn heca_lists_lead_in_their_order_whoever_named_one_first() {
        clear_for_tests();
        PaletteList::new("demo.rooms").sigil('%').items(Vec::new());
        PaletteList::new("session").render_item(|_, row| row);
        let names: Vec<String> = declared().into_iter().map(|list| list.name).collect();
        assert_eq!(
            names,
            [
                "command",
                "pane",
                "workspace",
                "session",
                "follow",
                "notification",
                "demo.rooms"
            ]
        );
        assert!(
            find("session").is_some_and(|list| list.sigil == Some('#') && list.host_rows.is_some()),
            "adding to heca's list kept what heca declared"
        );
        clear_for_tests();
    }
}
