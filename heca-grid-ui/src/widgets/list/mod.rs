//! [`List`] — **a long list that builds only what is on screen**, with a selection the arrows and
//! `Ctrl+j`/`Ctrl+k` move and Enter or a click chooses. React Native's `FlatList`, Flutter's
//! `ListView.builder`, SwiftUI's `List`.
//!
//! ```
//! use heca_grid_ui::prelude::*;
//! use heca_grid_ui::widgets::{Label, List};
//!
//! let rooms = signal(vec!["lobby".to_string(), "kitchen".to_string()]);
//! let selected = signal(Some(0));
//! let list = List::new(rooms)
//!     .render_item(|room, _index| Label::new(room.clone()))   // any widget
//!     .selected(selected)                                     // the row the keyboard is on
//!     .on_choose(|index| println!("chose {index}"));          // Enter, or a click
//! ```
//!
//! - **`render_item(|item, index| …)`** makes the widget for one item — any widget. It is called only
//!   for the items in view and a few beyond each edge; scrolling builds the ones coming into view and
//!   drops the ones leaving it. An item equal to one already built keeps its widget.
//! - **The list draws the selection and the hover** round what `render_item` returned (a
//!   [`Row`](crate::widgets::Row)), so every item reads the same when selected and `render_item`
//!   draws only the item.
//! - **`selected`** is the selected position, held in a signal: yours, or the list's own. The arrows
//!   and `Ctrl+j`/`Ctrl+k` (`menu_down` / `menu_up` in `[keys.widgets]`) move it and bring it into
//!   view; pointing at a row selects it without scrolling; writing the signal selects that row and
//!   brings it into view.
//! - **`on_choose(|index| …)`** runs when a row is chosen: Enter on the selected one (`activate`), or
//!   a click.
//! - **The height of the items not built is estimated** from the items built so far, so the scroll
//!   bar and the scroll range are those of the whole list.

mod component;
mod slot;
mod window;

use std::cell::Cell;
use std::rc::Rc;

use crate::builders::{EventExt, IntoComponent, LayoutExt, Parent};
use crate::component::Base;
use crate::event::EventKind;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::{Direction, Length, Spacing};
use crate::widgets::{Flex, Row, ScrollRegion};

use slot::{Measured, Slot};

/// What makes the widget for one item.
type RenderItem<T> = Rc<dyn Fn(&T, usize) -> crate::WidgetModel>;

/// **A long list that builds only what is on screen** — see the module docs.
pub struct List<T: Clone + PartialEq + 'static> {
    base: Base,
    items: Signal<Vec<T>>,
    render: Option<RenderItem<T>>,
    selected: Signal<Option<usize>>,
    on_choose: Option<Rc<dyn Fn(usize)>>,
    border_width: Option<f32>,
    /// Whether the inside has been built — on the first layout, once every builder has run.
    built: bool,
    /// The items built, `start..end`.
    window: Signal<(usize, usize)>,
    /// The scroll offset of the region inside.
    scroll: Option<Signal<f32>>,
    /// Heights of the items built, measured by the last layout.
    measured: Measured,
    /// The estimated height of one item.
    item_h: f64,
    /// The room above and below the items built.
    above: Signal<f64>,
    below: Signal<f64>,
    /// How many items it shows before it scrolls, when it says.
    visible: Option<crate::builders::LiveValue<usize>>,
    /// Whether the selection last moved by the keyboard (or by code) rather than the pointer — a row
    /// brings itself into view only then.
    by_keyboard: Signal<bool>,
    /// Set by a row the pointer moved onto, as it selects itself.
    pointed: Rc<Cell<bool>>,
    /// The selection the list last acted on.
    seen: Option<usize>,
}

impl<T: Clone + PartialEq + 'static> List<T> {
    /// A list of `items`. Give it [`render_item`](Self::render_item) to show them.
    pub fn new(items: Signal<Vec<T>>) -> Self {
        let mut base = Base::container();
        base.style.layout.direction = Direction::Column;
        // It gives way in a short box — below its content's height, which is what lets the region
        // inside scroll instead of the list running off the box.
        base.style.layout.flex_shrink = Some(1.0);
        base.style.layout.min_height = Some(Length::Px(0.0));
        Self {
            base,
            items,
            render: None,
            selected: signal(None),
            on_choose: None,
            border_width: None,
            built: false,
            window: signal((0, 0)),
            scroll: None,
            measured: Rc::new(Cell::new((0.0, 0))),
            item_h: 0.0,
            above: signal(0.0),
            below: signal(0.0),
            visible: None,
            by_keyboard: signal(false),
            pointed: Rc::new(Cell::new(false)),
            seen: None,
        }
    }

    /// **The widget for one item** — `|item, index| …`, any widget. Called only for the items in
    /// view and a few beyond each edge.
    pub fn render_item<C: IntoComponent>(
        mut self,
        render: impl Fn(&T, usize) -> C + 'static,
    ) -> Self {
        self.render = Some(Rc::new(move |item, index| {
            render(item, index).into_component()
        }));
        self
    }

    /// **The selected position**, in your signal. Unset, the list keeps its own.
    pub fn selected(mut self, selected: Signal<Option<usize>>) -> Self {
        self.selected = selected;
        self
    }

    /// **What choosing a row does** — Enter on the selected row, or a click. Gets its position.
    pub fn on_choose(mut self, choose: impl Fn(usize) + 'static) -> Self {
        self.on_choose = Some(Rc::new(choose));
        self
    }

    /// **The width of the selected row's outline** — the same builder
    /// [`Row::border_width`](crate::widgets::Row::border_width) is. Unset, the theme's ring.
    pub fn border_width(mut self, width: f32) -> Self {
        self.border_width = Some(width);
        self
    }

    /// **How many items it shows before it scrolls** — its height is that many of its items,
    /// measured, so a list of rows of any height shows exactly that many. A count, or a value that
    /// changes (a `Signal<usize>`, a function reading signals). Unset, it is as tall as its
    /// container lets it be.
    pub fn visible_items(mut self, count: impl crate::builders::IntoCountValue) -> Self {
        let count = count.into_count_value();
        if count.changes() {
            let watched = count.clone();
            crate::reactive::create_effect(move |_| {
                watched.now();
                crate::component::request_frame();
            });
        }
        self.visible = Some(count);
        self
    }

    /// The selected position's signal — read it, or write it to select a row.
    pub fn selected_signal(&self) -> Signal<Option<usize>> {
        self.selected
    }

    /// Build the inside: a region scrolling the room above, the items in the window, and the room
    /// below. Once, on the first layout, when every builder has run.
    fn build(&mut self) {
        self.built = true;
        let Some(render) = self.render.clone() else {
            return;
        };
        let (items, window) = (self.items, self.window);
        let read = move || {
            let (start, end) = window.get();
            let items = items.get();
            let end = end.min(items.len());
            (start.min(end)..end)
                .map(|i| (i, items[i].clone()))
                .collect::<Vec<_>>()
        };
        let make = {
            let (selected, by_keyboard, pointed) =
                (self.selected, self.by_keyboard, self.pointed.clone());
            let (choose, measured, border) = (
                self.on_choose.clone(),
                self.measured.clone(),
                self.border_width,
            );
            move |(index, item): &(usize, T)| {
                let index = *index;
                let pointed = pointed.clone();
                let choose = choose.clone();
                let mut row = Row::new()
                    .padding_x(Spacing::Md)
                    .padding_y(Spacing::Sm)
                    .nav_selected(move || selected.get() == Some(index))
                    .reveal_when(by_keyboard)
                    .on(EventKind::PointerEnter, move |_| {
                        if selected.get_untracked() != Some(index) {
                            pointed.set(true);
                            selected.set(Some(index));
                        }
                    })
                    .on_click(move |_| {
                        if let Some(choose) = &choose {
                            choose(index);
                        }
                    })
                    .child(render(item, index));
                if let Some(width) = border {
                    row = row.border_width(width);
                }
                Slot::new(row, measured.clone())
            }
        };
        // The room of the items not built, above and below — a box as tall as they would be.
        let room = |height: Signal<f64>| {
            Flex::column()
                .shrink(0.0)
                .height(move || Length::Px(height.get() as f32))
        };
        let content = Flex::column()
            .width(Length::FULL)
            .child(room(self.above))
            .child(crate::builders::BoundChildren::new(read, make))
            .child(room(self.below));
        let region = ScrollRegion::new()
            .width(Length::FULL)
            .shrink(1.0)
            .min_height(0.0)
            .child(content);
        self.scroll = Some(region.scroll_offset());
        self.base.children = vec![crate::WidgetModel::new(region)];
    }
}

impl<T: Clone + PartialEq + 'static> LayoutExt for List<T> {}

/// **What a description may set on a list** — written out because the list is generic over its
/// items, which the generating macro does not take; the same two lists it would produce.
impl<T: Clone + PartialEq + 'static> crate::prop::SetProp for List<T> {
    const PROP_NAMES: &'static [&'static str] = &["border_width"];
    const HOST_ONLY_BUILDERS: &'static [&'static str] = &[
        // A closure; a description's items are its children.
        "render_item",
        // A live signal; a description says `selected` as a position, read by realize.
        "selected",
        // A closure; a description binds the `activate` event.
        "on_choose",
    ];

    fn set_prop(self, key: &str, value: &crate::prop::PropInput) -> Self {
        match (key, value.as_f32()) {
            ("border_width", Some(width)) => self.border_width(width),
            _ => self,
        }
    }

    fn accepts_prop(key: &str, value: &crate::prop::PropInput) -> bool {
        key == "border_width" && value.as_f32().is_some()
    }
}
