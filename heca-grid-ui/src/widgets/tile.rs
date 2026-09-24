//! [`Tile`] — **a thing, said in a line or a few**: a status pip, an icon, a title with an optional
//! quieter suffix, and any number of lines under it.
//!
//! ```text
//!  ● ▣ Neovim (nvim)        ← the head: status · icon · title · suffix
//!    🗀 ~/projects/heca      ← a line
//!    ⎇ main  +2  ~3         ← another line
//! ```
//!
//! It is the shape of a sidebar pane card, a file in a list, a container in a Docker panel — and it
//! knows nothing about any of them. It **arranges**; everything in it is a widget the caller built
//! and keeps, so a signal the caller holds (a title that renames, a pip that changes status, a line
//! that appears when a directory arrives) updates the tile in place, with no rebuild.
//!
//! **It owns only the arrangement, not the selection.** Put it in a [`Row`](super::Row) for the
//! selected pill, the cursor outline, the attention flash and the click:
//!
//! ```ignore
//! Row::new().on_activate(open).child(
//!     Tile::new()
//!         .status(StatusDot::online())
//!         .icon(Icon::new(Glyph::FileCode))
//!         .title(Label::new("Neovim").bold(true))
//!         .suffix(Label::new("(nvim)").font_scale(0.8))
//!         .line(folder_line)
//!         .line(git_line),
//! )
//! ```
//!
//! Why this is a widget: the app's sidebar row and the showcase's demo of it were two hand-written
//! copies of this arrangement, and only one of them got fixed (F003/P082/T491). The showcase could
//! not use the app's, because it cannot reach app code — so the arrangement belongs here, where both
//! can (AGENTS.md § 0b: if it can be built inside grid-ui, it belongs in grid-ui).

use crate::builders::{IntoComponent, LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component};
use crate::style::{Align, Direction, Spacing};
use crate::widgets::Flex;

/// **Why the head is a `Flex` row and not a `Grid`.** Every part of it is optional, and a part that is
/// not there must leave no trace — no track and no gap. A flex row's gap falls only *between the
/// items it lays out*, so a hidden pip costs nothing; a grid keeps an empty track for it and still
/// spends a gap on each side. The head is an inline run, and a run is what a flex row is for.
///
/// Where each head part sits among the head's children. The head is built with every slot present
/// and hidden, so a slot can be filled in any order without splicing children at a surprising
/// position — properties are order-independent (the rule [`Panel`](super::Panel) follows).
const STATUS: usize = 0;
const ICON: usize = 1;
const TEXT: usize = 2;
/// Inside the text run: the title, then the suffix.
const TITLE: usize = 0;
const SUFFIX: usize = 1;
/// The head is the tile's first child; lines follow it.
const HEAD: usize = 0;

/// A status pip, an icon, a title with an optional suffix, and lines under it — see the
/// [module docs](self).
pub struct Tile {
    base: Base,
}

/// An empty, hidden place for a slot nobody has filled. Hidden is `display: none`, so an unfilled
/// slot takes no room and no gap.
fn placeholder() -> Box<dyn Component> {
    let mut empty = Flex::row();
    empty.base_mut().set_hidden(true);
    Box::new(empty)
}

#[heca_grid_ui_macros::props]
impl Tile {
    /// An empty tile. Fill it with [`status`](Tile::status), [`icon`](Tile::icon),
    /// [`title`](Tile::title), [`suffix`](Tile::suffix) and [`line`](Tile::line), in any order.
    pub fn new() -> Self {
        // The title and its suffix are two runs of text on one line, centred on each other. The
        // title gives way first when the tile is narrow — it is the one that cuts with `…`.
        let text = Flex::row()
            .align(Align::Center)
            .gap(Spacing::Sm)
            .flex(1.0)
            .child(placeholder())
            .child(placeholder());
        // The head: pip · icon · text, centred on one line.
        let head = Flex::row()
            .align(Align::Center)
            .gap(Spacing::Sm)
            .child(placeholder())
            .child(placeholder())
            .child(text);

        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        base.style.layout.gap = Spacing::Xs.into();
        base.children.push(Box::new(head));
        Self { base }
    }

    /// **The status pip** at the front — usually a [`StatusDot`](super::StatusDot), which keeps its
    /// own size however narrow the tile gets. Unset, there is no pip and no gap for one.
    #[heca_grid_ui_macros::host_only("a child — a description places it with the `slot` prop")]
    pub fn status(mut self, pip: impl IntoComponent) -> Self {
        self.put_in_head(STATUS, pip.into_component());
        self
    }

    /// **The icon** after the pip — usually an [`Icon`](super::Icon). Unset, none and no gap.
    #[heca_grid_ui_macros::host_only("a child — a description places it with the `slot` prop")]
    pub fn icon(mut self, icon: impl IntoComponent) -> Self {
        self.put_in_head(ICON, icon.into_component());
        self
    }

    /// **The title** — any widget, usually a bold [`Label`](super::Label). Keep its text signal to
    /// rename the tile in place. It is the part that gives way when the tile is narrow, so give a
    /// `Label` an ellipsis (`.truncate(..)`) to cut rather than spill.
    #[heca_grid_ui_macros::host_only("a child — a description places it with the `slot` prop")]
    pub fn title(mut self, title: impl IntoComponent) -> Self {
        self.put_in_text(TITLE, title.into_component());
        self
    }

    /// **A quieter word after the title**, on the same line and centred on it — a program name in
    /// parentheses, a count. Any widget; a `Label` at a smaller `font_scale` reads as secondary. To
    /// show it only sometimes, pass a [`Visibility`](super::Visibility) and flip its signal.
    #[heca_grid_ui_macros::host_only("a child — a description places it with the `slot` prop")]
    pub fn suffix(mut self, suffix: impl IntoComponent) -> Self {
        self.put_in_text(SUFFIX, suffix.into_component());
        self
    }

    /// **A line under the head** — call it again for another; they stack in the order given. Any
    /// widget.
    ///
    /// ⚠️ **Add a line always, and let it hide itself.** A line that has nothing to say yet (a
    /// directory the shell has not reported) should be a [`Visibility`](super::Visibility) whose
    /// signal the caller flips — attached from the start, so it can appear later without a rebuild.
    /// A hidden line takes no room and no gap. Adding the line only once it has something to say is
    /// how a card ended up showing its path or not depending on timing.
    #[heca_grid_ui_macros::host_only("a child — a description places it with the `slot` prop")]
    pub fn line(mut self, line: impl IntoComponent) -> Self {
        self.base.children.push(line.into_component());
        self
    }
}

impl Tile {
    fn head(&mut self) -> &mut Base {
        self.base.children[HEAD].base_mut()
    }

    fn put_in_head(&mut self, at: usize, part: Box<dyn Component>) {
        self.head().children[at] = part;
    }

    fn put_in_text(&mut self, at: usize, part: Box<dyn Component>) {
        self.head().children[TEXT].base_mut().children[at] = part;
    }
}

impl Default for Tile {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Tile {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
}

impl LayoutExt for Tile {}
impl StyleExt for Tile {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::widgets::{Label, StatusDot, Visibility};
    use crate::{LayoutEngine, Size};

    #[test]
    fn filled_slots_sit_in_order_on_one_line_and_lines_stack_under_it() {
        let mut root = Flex::column().width(300.0).height(400.0).child(
            Tile::new()
                .title(Label::new("Neovim"))
                .icon(Label::new("I"))
                .status(StatusDot::online())
                .suffix(Label::new("(nvim)"))
                .line(Label::new("~/projects"))
                .line(Visibility::new(Label::new("main"), false)),
        );
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 400.0));
        let tile = &root.base().children[0];
        let head = &tile.base().children[HEAD];
        // Found by what they say, not by the slot numbers the widget uses — a test reading the
        // same numbers could not catch a part put in the wrong slot.
        fn x_of(node: &dyn Component, text: &str) -> Option<f64> {
            if node.base().children.is_empty() && node.text_summary().as_deref() == Some(text) {
                return Some(node.base().bounds.loc.x);
            }
            node.base()
                .children
                .iter()
                .find_map(|c| x_of(c.as_ref(), text))
        }
        let (icon, title, suffix) = (
            x_of(head.as_ref(), "I").unwrap(),
            x_of(head.as_ref(), "Neovim").unwrap(),
            x_of(head.as_ref(), "(nvim)").unwrap(),
        );
        let pip = head.base().children[0].base().bounds.loc.x;
        // Given out of order, drawn in order: pip, icon, title, suffix.
        assert!(
            pip < icon && icon < title && title < suffix,
            "{pip} {icon} {title} {suffix}"
        );
        // The first line sits below the head; the hidden one takes no room.
        let first = &tile.base().children[1];
        assert!(first.base().bounds.loc.y >= head.base().bounds.loc.y + head.base().bounds.size.h);
        let bottom = tile.base().bounds.loc.y + tile.base().bounds.size.h;
        assert!(
            (bottom - (first.base().bounds.loc.y + first.base().bounds.size.h)).abs() < 1.0,
            "a hidden line adds no height",
        );
    }

    #[test]
    fn an_unfilled_slot_takes_no_room() {
        let mut bare = Flex::column()
            .width(300.0)
            .child(Tile::new().title(Label::new("x")));
        let mut with_icon = Flex::column()
            .width(300.0)
            .child(Tile::new().icon(Label::new("I")).title(Label::new("x")));
        LayoutEngine::new().compute(&mut bare, Size::new(300.0, 400.0));
        LayoutEngine::new().compute(&mut with_icon, Size::new(300.0, 400.0));
        let title_x = |root: &Flex| {
            root.base().children[0].base().children[HEAD]
                .base()
                .children[TEXT]
                .base()
                .bounds
                .loc
                .x
        };
        assert_eq!(
            title_x(&bare),
            0.0,
            "no pip, no icon: the title starts at the edge"
        );
        assert!(title_x(&with_icon) > 0.0, "an icon pushes it along");
    }
}
