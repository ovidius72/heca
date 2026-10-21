//! Editable text is a Label; selection and caret are its chrome. ScrollRegion moves all of it.

use super::*;
use crate::{Label, Grid, Parent};
use crate::builders::PlaceExt;
use std::{cell::RefCell, ops::Range, rc::Rc};

pub(super) struct View {
    pub text: Signal<String>,
    pub cursor: usize,
    pub selection: Option<(usize, usize)>,
    pub focused: bool,
    pub caret: bool,
    pub multiline: bool,
    pub placeholder: String,
    pub placeholder_on_focus: bool,
    pub width: f64,
    pub font: f32,
    pub caret_width: f32,
    pub offset_x: Signal<f32>,
    pub offset_y: Signal<f32>,
}
impl View {
    pub fn new(text: Signal<String>) -> Self {
        Self { text, cursor: 0, selection: None, focused: false, caret: false, multiline: false,
            placeholder: String::new(), placeholder_on_focus: false, width: 0.0,
            offset_x: signal(0.0), offset_y: signal(0.0),
            font: crate::Theme::default().font_size, caret_width: heca_theme::InputStyle::default().caret_width }
    }
    pub fn rows(&self) -> Vec<Range<usize>> {
        let cell = (self.font * MONO_ADVANCE_RATIO) as f64;
        let cells = if self.multiline {
            crate::font::mono_cells((self.width - self.caret_width as f64).max(0.0), cell).max(1)
        } else { usize::MAX };
        crate::font::preserved_ranges(&self.text.get_untracked(), cells)
    }
    pub fn position(&self, index: usize) -> (usize, usize) {
        let rows = self.rows();
        let row = rows.iter().rposition(|r| index >= r.start).unwrap_or(0);
        (row, index.saturating_sub(rows[row].start).min(rows[row].len()))
    }
    pub fn index_at(&self, point: Point, origin: Point, round: bool) -> usize {
        let rows = self.rows();
        let line = (self.font * MONO_LINE_RATIO) as f64;
        let cell = (self.font * MONO_ADVANCE_RATIO) as f64;
        let row = (((point.y - origin.y + self.offset_y.get_untracked() as f64).max(0.0) / line.max(f64::MIN_POSITIVE)) as usize)
            .min(rows.len().saturating_sub(1));
        let column = (point.x - origin.x + self.offset_x.get_untracked() as f64).max(0.0) / cell.max(f64::MIN_POSITIVE);
        let column = if round { column.round() } else { column.floor() } as usize;
        rows[row].start + column.min(rows[row].len())
    }
}

pub(super) struct Content {
    base: Base,
    view: Rc<RefCell<View>>,
    wrap: Signal<bool>,
}
impl Content {
    pub fn new(view: Rc<RefCell<View>>) -> Self {
        let label = Label::new("").with_text_signal(view.borrow().text).wrap(true).preserve_whitespace(true).truncate(crate::Ellipsis::None);
        let wrap = label.wrap_signal();
        let mut base = Base::container();
        let placeholder = Label::new("");
        let grid = Grid::new().template_row("auto").template_column("1fr")
            .child(label.row(1).column(1)).child(placeholder.row(1).column(1));
        base.children.push(Box::new(grid));
        base.children.push(Box::new(Caret { base: Base::new(), view: view.clone() }));
        Self { base, view, wrap }
    }
}
impl Component for Content {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn remeasure(&mut self) {
        let view = self.view.borrow();
        let text = view.text.get_untracked();
        let placeholder = text.is_empty() && (!view.focused || view.placeholder_on_focus);
        self.base.children[0].base().children[1].set_text(view.placeholder.clone());
        self.base.style.layout.min_width = Some(Length::Percent(1.0));
        self.base.style.layout.width = if view.multiline { Length::Percent(1.0) } else { Length::Auto };
        self.base.style.layout.padding_right = Some(crate::style::Space::Px(view.caret_width));
        self.base.style.visual.content_tone = placeholder.then_some(crate::Tone::Muted);
        if self.wrap.get_untracked() != view.multiline { self.wrap.set(view.multiline); }
        for label in &mut self.base.children[0].base_mut().children {
            // Explicit font sizes are already resolved; applying the size variant again
            // makes the glyphs disagree with the caret and selection geometry.
            label.base_mut().style.visual.font_size = view.font;
        }

    }
    fn on_layout(&mut self) {
        let mut view = self.view.borrow_mut();
        view.width = self.base.bounds.size.w;
        let (row, column) = view.position(view.cursor);
        let cell = (view.font * MONO_ADVANCE_RATIO) as f64;
        let line = (view.font * MONO_LINE_RATIO) as f64;
        self.base.children[1].base_mut().bounds = Rectangle::new(
            Point::new(self.base.bounds.loc.x + column as f64 * cell,
                self.base.bounds.loc.y + row as f64 * line),
            Size::new(view.caret_width as f64, line));
    }
    fn paint(&self, cx: &mut PaintCx) {
        let view = self.view.borrow();
        if let Some((start, end)) = view.selection {
            let line = (view.font * MONO_LINE_RATIO) as f64;
            let cell = (view.font * MONO_ADVANCE_RATIO) as f64;
            for (index, row) in view.rows().iter().enumerate() {
                let a = start.max(row.start);
                let b = end.min(row.end);
                if a < b {
                    cx.rect(Rectangle::new(Point::new(self.base.bounds.loc.x + (a - row.start) as f64 * cell,
                        self.base.bounds.loc.y + index as f64 * line), Size::new((b - a) as f64 * cell, line)),
                        cx.accent().with_alpha(cx.theme().colors.interaction.selection), None, 0.0, None);
                }
            }
        }
        let placeholder = view.text.get_untracked().is_empty() && (!view.focused || view.placeholder_on_focus);
        self.base.children[0].base().children[0].base().visible.set(!placeholder);
        self.base.children[0].base().children[1].base().visible.set(placeholder);
        drop(view);
        for child in &self.base.children { crate::component::paint_child(child.as_ref(), cx); }
    }
}

struct Caret { base: Base, view: Rc<RefCell<View>> }
impl Component for Caret {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn wants_visible(&self) -> bool { self.view.borrow().focused }
    fn paint(&self, cx: &mut PaintCx) {
        let view = self.view.borrow();
        if view.focused && view.caret && view.selection.is_none() {
            cx.rect(self.base.bounds, cx.accent(), None, 0.0, None);
        }
    }
}
