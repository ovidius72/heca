//! **What extensions and plugins add to the palette's rows** — `pro.palette("session").to_raw(..)`.
//!
//! An extension registers, for a palette mode, a function from the row's item to something drawn at
//! the end of the row: a widget (`to_raw`, native code) or a description (`to_description`, what a
//! plugin can send across its boundary). Both end in the same place — a description is realized
//! into a widget — so the row cannot tell which door it came through. Several contributions to one
//! row sit side by side, in the order they were registered. The function runs each time a row is
//! built; a widget that follows a signal of its own stays current while the palette is open.

use std::cell::RefCell;
use std::rc::Rc;

use heca_grid_ui::WidgetModel;
use heca_grid_ui::prelude::*;

use crate::entry::service::SessionEntry;

/// **What a palette row stands for**, handed to what an extension adds to it.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PaletteItem {
    /// A row of the session list (`#`, mode `"session"`).
    Session(SessionEntry),
}

impl PaletteItem {
    /// The session, when the row is one.
    pub fn session(&self) -> Option<&SessionEntry> {
        match self {
            Self::Session(entry) => Some(entry),
        }
    }

    /// The mode the row belongs to, as [`palette`](crate::Extension::palette) names it.
    pub fn mode(&self) -> &'static str {
        match self {
            Self::Session(_) => "session",
        }
    }
}

type Raw = Rc<dyn Fn(&PaletteItem) -> Option<WidgetModel>>;
/// Turns a description into the widget it describes, with the palette's theme and intent sink.
pub(super) type Realize = Rc<dyn Fn(&heca_view::ViewNode) -> Option<WidgetModel>>;
/// Makes what sits at the end of one row.
pub(super) type MakeTrailing = Rc<dyn Fn() -> WidgetModel>;
type Described = Rc<dyn Fn(&PaletteItem) -> Option<heca_view::ViewNode>>;

/// One registered contribution: a widget or a description, for one mode.
#[derive(Clone)]
enum Produce {
    Raw(Raw),
    Description(Described),
}

thread_local! {
    static CONTRIBUTIONS: RefCell<Vec<(String, Produce)>> = const { RefCell::new(Vec::new()) };
}

/// **A palette mode an extension adds to**, returned by
/// [`Extension::palette`](crate::Extension::palette).
pub struct PaletteRows {
    mode: String,
}

impl PaletteRows {
    pub(crate) fn new(mode: &str) -> Self {
        Self {
            mode: mode.to_owned(),
        }
    }

    /// **Native code: a widget at the end of each row** — or nothing (`None`) for a row it has
    /// nothing to say about.
    ///
    /// ```ignore
    /// pro.palette("session").to_raw(move |item| {
    ///     let session = item.session()?;
    ///     rooms.with(|r| r.is_pro(session.key())).then(|| Badge::accent("pro"))
    /// });
    /// ```
    pub fn to_raw<C: heca_grid_ui::builders::IntoComponent>(
        self,
        produce: impl Fn(&PaletteItem) -> Option<C> + 'static,
    ) {
        let produce: Raw = Rc::new(move |item| {
            produce(item).map(heca_grid_ui::builders::IntoComponent::into_component)
        });
        add(self.mode, Produce::Raw(produce));
    }

    /// **A plugin: a description at the end of each row** — data that crosses the plugin boundary;
    /// heca realizes it into the same widget `to_raw` would give.
    pub fn to_description(
        self,
        produce: impl Fn(&PaletteItem) -> Option<heca_view::ViewNode> + 'static,
    ) {
        add(self.mode, Produce::Description(Rc::new(produce)));
    }
}

fn add(mode: String, produce: Produce) {
    CONTRIBUTIONS.with(|all| all.borrow_mut().push((mode, produce)));
}

/// **What sits at the end of `item`'s row**, as a way to make it — `None` when nothing is registered
/// for its mode. `realize` turns a description into a widget with the palette's own theme and
/// intent sink.
pub(super) fn trailing_for(item: PaletteItem, realize: Realize) -> Option<MakeTrailing> {
    let produce: Vec<Produce> = CONTRIBUTIONS.with(|all| {
        all.borrow()
            .iter()
            .filter(|(mode, _)| mode == item.mode())
            .map(|(_, produce)| produce.clone())
            .collect()
    });
    if produce.is_empty() {
        return None;
    }
    Some(Rc::new(move || {
        let parts: Vec<WidgetModel> = produce
            .iter()
            .filter_map(|produce| match produce {
                Produce::Raw(make) => make(&item),
                Produce::Description(describe) => describe(&item).and_then(|node| realize(&node)),
            })
            .collect();
        heca_grid_ui::widgets::Flex::row()
            .gap(heca_grid_ui::style::Spacing::Xs)
            .align("center")
            .child(parts)
            .into()
    }))
}

#[cfg(test)]
pub(super) fn clear_for_tests() {
    CONTRIBUTIONS.with(|all| all.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::service::{SessionKey, SessionState};

    fn session() -> PaletteItem {
        PaletteItem::Session(SessionEntry::new(
            SessionKey::generate(),
            "work".into(),
            None,
            SessionState::Active,
        ))
    }

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

    /// **Both doors give the row the same thing**: a widget from `to_raw` and a description from
    /// `to_description` sit side by side, in the order registered; a mode with nothing registered
    /// adds nothing, and a contribution that says `None` is left out.
    #[test]
    fn a_widget_and_a_description_reach_the_row_alike() {
        clear_for_tests();
        let item = session();
        assert!(
            trailing_for(item.clone(), realize()).is_none(),
            "nothing registered: nothing added"
        );
        PaletteRows::new("session").to_raw(|item| {
            item.session()
                .map(|s| heca_grid_ui::Label::new(format!("raw {}", s.name())))
        });
        PaletteRows::new("session").to_description(|item| {
            item.session().map(|s| {
                heca_view::ViewNode::new(heca_view::WidgetId::Label)
                    .text(format!("described {}", s.name()))
            })
        });
        PaletteRows::new("session").to_raw(|_| None::<heca_grid_ui::Label>);
        PaletteRows::new("pane").to_raw(|_| Some(heca_grid_ui::Label::new("another mode")));
        let make = trailing_for(item, realize()).expect("something is registered for sessions");
        assert_eq!(texts(&make()), ["raw work", "described work"]);
        assert_eq!(
            texts(&make()),
            ["raw work", "described work"],
            "made again, the same"
        );
        clear_for_tests();
    }
}
