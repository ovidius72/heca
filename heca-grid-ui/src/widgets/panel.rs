//! [`Panel`] — a titled section container: an optional header label over body content.
//!
//! The difference from [`Card`](super::Card) is presentation, not structure. A `Card` is a *card*:
//! padded 18, framed with the theme border and radius, meant to stand apart from what surrounds it.
//! A `Panel` is a *section of a region* — a plugin's slice of a sidebar, say — so it is quiet by
//! default: no frame of its own, lighter padding, and a header that reads as a heading rather than
//! a card title. Give it a fill or a border through [`StyleExt`] when it should stand out.
//!
//! The title is a real [`Label`](super::Label) child, like every other piece of widget content
//! (`AGENTS.md`: a widget composes its content and paints only its own chrome). It is created up
//! front and **hidden until a title is set**, so `title` can be applied before or after children
//! without inserting into the child list at a surprising position — properties are
//! order-independent, and a widget whose setters depend on ordering is the bug.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component};
use crate::reactive::{Signal, SignalUpdate};
use crate::style::Direction;
use crate::widgets::{Label, Separator};

/// Title size relative to the inherited base font — a heading, a touch larger than body text.
const TITLE_SCALE: f32 = 1.05;
/// Space between the header and the body.
const GAP: f32 = 8.0;
/// How many leading children make up the header: the label and the rule under it. Content is
/// everything after them.
const HEADER_PARTS: usize = 2;
/// Breathing room inside the panel. Lighter than [`Card`](super::Card)'s 18: a panel usually sits
/// inside a region that already has its own padding.
const PADDING: f32 = 10.0;

/// A titled section container.
pub struct Panel {
    base: Base,
    /// The header label's own text signal, kept so [`title`](Panel::title) drives the live label
    /// rather than rebuilding it — and so a host can retitle a mounted panel with no rebuild at all.
    title: Signal<String>,
}

#[heca_grid_ui_macros::props]
impl Panel {
    /// An untitled panel. Add content with `.child(...)`; add a heading with
    /// [`title`](Panel::title).
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        base.style.layout.padding = PADDING;
        base.style.layout.gap = GAP;
        // The header and its rule exist from the start and hide themselves until there is a
        // title, so setting a title never has to splice children in at the front after content
        // has been attached.
        let mut header = Label::new(String::new()).font_scale(TITLE_SCALE).bold(true);
        let title = header.text_signal();
        header.base_mut().style.layout.hidden = true;
        base.children.push(Box::new(header));

        // The rule under the heading is what makes the title read as a header band rather than
        // the first line of the content. It takes the theme's border colour, like every other
        // separator, so it follows a theme change without the panel knowing anything about it.
        let mut rule = Separator::horizontal();
        rule.base_mut().style.layout.hidden = true;
        base.children.push(Box::new(rule));

        Self { base, title }
    }

    /// A panel with a heading. The same as `Panel::new().title(t)`.
    pub fn titled(title: impl Into<String>) -> Self {
        Self::new().title(title)
    }

    /// Set the heading. An empty title hides the header rather than leaving a blank line.
    #[heca_grid_ui_macros::prop]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        // The heading and its rule appear and disappear together: an untitled panel draws neither,
        // rather than a bare rule across the top of its content.
        let hidden = title.is_empty();
        for part in self.base.children.iter_mut().take(HEADER_PARTS) {
            part.base_mut().style.layout.hidden = hidden;
        }
        self.title.set(title);
        self
    }

    /// The heading's text signal — set it to retitle a mounted panel in place.
    pub fn title_signal(&self) -> Signal<String> {
        self.title
    }
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Panel {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
}

impl LayoutExt for Panel {}
impl StyleExt for Panel {}
impl Parent for Panel {}
