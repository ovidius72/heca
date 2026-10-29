//! A growing list of composed entries. ScrollRegion owns the viewport and following the bottom.

use std::{cell::RefCell, rc::Rc};
use crate::{Base, Component, Label, LayoutExt, PaintCx, ScrollRegion};
use crate::builders::{IntoComponent};

/// A producer for a mounted message list. Retain it before moving the list into a tree.
#[derive(Clone)]
pub struct MessageFeed(Rc<RefCell<Vec<crate::WidgetModel>>>);

impl MessageFeed {
    /// Append arbitrary content. Existing entries and their local state are preserved.
    pub fn append(&self, entry: impl IntoComponent) {
        self.0.borrow_mut().push(entry.into_component());
        crate::component::request_frame();
    }
    /// Append plain text, wrapped by Label at the available width.
    pub fn message(&self, text: impl Into<String>) {
        self.append(Label::new(text).wrap(true));
    }
}

/// A scrollable list of entries, initially following its bottom. Scrolling up pauses following.
/// Give the list a bounded height; each entry keeps its natural height.
///
/// ```
/// use heca_grid_ui::{MessageList, LayoutExt};
/// let list = MessageList::new().height(200.0).message("First entry");
/// let feed = list.feed();
/// feed.message("Arrived later");
/// ```
///
/// [`message`](Self::message) appends wrapped Label text; [`entry`](Self::entry) and
/// [`MessageFeed::append`] accept arbitrary composed content with its own events. Existing entries
/// keep their widget state. [`visible_lines`](Self::visible_lines) optionally sizes the viewport
/// from the font; an explicit layout height wins. Root gap defaults to `sm` and applies between
/// entries. New arrivals request a frame and layout automatically. The feed is UI-thread local.
///
/// Descriptions use ordinary children as entries, or the `message` text prop for one initial entry
/// (before the children). There are no list-specific events. Controls in entries retain theirs.
///
/// ```ignore
/// use heca_view::{ViewNode, WidgetId, PropValue};
/// let messages = ViewNode::new(WidgetId::MessageList)
///     .prop("visible_lines", PropValue::Int(6))
///     .child(ViewNode::new(WidgetId::Label).text("First entry"));
/// ```
pub struct MessageList {
    base: Base,
    feed: MessageFeed,
    visible_lines: Option<usize>,
}

#[heca_grid_ui_macros::props]
impl MessageList {
    /// Empty list. Its scrolling child fills whatever viewport the parent gives it.
    pub fn new() -> Self {
        let mut base = Base::container();
        base.style.layout.gap = "sm".into();
        base.children.push(crate::WidgetModel::new(ScrollRegion::new().follow_bottom(true)
            .width("100%").height("100%")));
        Self { base, feed: MessageFeed(Rc::default()), visible_lines: None }
    }
    /// Append a wrapped text entry (also the `message` property for a described list).
    #[heca_grid_ui_macros::prop]
    pub fn message(self, text: impl Into<String>) -> Self {
        self.feed.message(text);
        self
    }
    /// Append arbitrary composed content. Descriptions use ordinary children.
    #[heca_grid_ui_macros::host_only("composed content crosses as children")]
    pub fn entry(self, entry: impl IntoComponent) -> Self {
        self.feed.append(entry);
        self
    }
    /// Bound the viewport to this many text lines (at least one), instead of its parent height.
    /// Unset by default. This limits the viewport, not the size or number of entries.
    #[heca_grid_ui_macros::prop]
    pub fn visible_lines(mut self, lines: usize) -> Self { self.visible_lines = Some(lines.max(1)); self }

    /// A cloneable producer; appending schedules a frame and keeps existing entry widgets alive.
    pub fn feed(&self) -> MessageFeed { self.feed.clone() }
}

impl Default for MessageList { fn default() -> Self { Self::new() } }
impl Component for MessageList {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn taffy_style(&self) -> taffy::Style {
        let mut layout = self.base.style.layout;
        if let Some(lines) = self.visible_lines && layout.height == crate::Length::Auto {
            layout.height = crate::Length::Px(self.base.font * crate::font::MONO_LINE_RATIO * lines as f32);
        }
        layout.to_taffy(self.base.font)
    }
    fn remeasure(&mut self) {
        self.base.children[0].base_mut().style.layout.gap = self.base.style.layout.gap;
        self.append_pending();
    }
    fn tick(&mut self, dt: f32) -> bool {
        self.append_pending();
        let mut moving = self.base.tick_presence(dt);
        for child in &mut self.base.children { moving |= child.tick(dt); }
        moving
    }
    fn paint(&self, cx: &mut PaintCx) {
        cx.paint_base(&self.base);
        for child in &self.base.children { crate::component::paint_child(child.as_ref(), cx); }
    }
}
impl MessageList {
    fn append_pending(&mut self) {
        let entries: Vec<_> = self.feed.0.borrow_mut().drain(..).collect();
        if !entries.is_empty() {
            self.base.children[0].base_mut().children.extend(entries);
            self.base.mark_needs_layout();
        }
    }
}
impl LayoutExt for MessageList {}
