//! [`Keyed`] — a child that is **rebuilt only when its key changes**.
//!
//! Something whose content comes from state is often rebuilt every frame just to find out that
//! nothing changed: the tree is made, compared and thrown away. `Keyed` turns that around. The
//! author sums up what the content *is* in a key, says it each frame with
//! [`show`](Keyed::show), and the build closure runs **only when the key differs from the one
//! already shown**. A frame in which nothing changed makes no widget at all.
//!
//! What the content *says* — a label's words — usually changes far more often than what it *is*, so
//! it is not part of the key: [`texts`](Keyed::texts) writes words, by the `key` of the widget that
//! shows each, onto the tree already there. A changed word moves that word and nothing else.
//!
//! It is a handle, like a [`Terminal`]-style widget: [`Clone`] gives another node for the **same**
//! content, so the one a tree holds and the one its owner keeps agree. A node placed anew holds no
//! tree yet, so it asks for the next `show` to build one — a rebuilt parent gets its child back.
//!
//! The tree built arrives in the node's own layout pass, before that pass reads it, so the frame
//! that asked for it also draws it.
//!
//! ```ignore
//! let header = Keyed::new();                 // placed once, with the pane that owns it
//! // …each frame, from whatever knows the facts:
//! header.show(shape_key(&facts), || Some(Box::new(build_header(&facts))));
//! header.texts(vec![("hdr.seg:location".into(), facts.cwd())]);
//! ```

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::builders::LayoutExt;
use crate::component::{Base, Component, PaintCx};
use crate::style::{Direction, Length};

/// What a [`Keyed`] and every node placed for it share.
#[derive(Default)]
struct State {
    /// The key of the content most recently built, or `None` when the node now placed has none.
    key: RefCell<Option<String>>,
    /// Content built and not yet taken by the node.
    pending: RefCell<Option<Box<dyn Component>>>,
    /// Words to write, by the key of the widget that shows each.
    texts: RefCell<Vec<(String, String)>>,
    /// Whether the words changed since the node wrote them.
    texts_dirty: Cell<bool>,
}

/// A child rebuilt only when its key changes. See the [module docs](self).
pub struct Keyed {
    base: Base,
    state: Rc<State>,
}

impl Keyed {
    /// Nothing built yet.
    pub fn new() -> Self {
        let mut base = Base::container();
        // A strip across the room it is given, as tall as what it holds.
        base.style.layout.direction = Direction::Column;
        base.style.layout.width = Length::Percent(1.0);
        base.style.layout.height = Length::Auto;
        Self {
            base,
            state: Rc::new(State::default()),
        }
    }

    /// **Show the content `key` stands for.** `build` makes it, and is called only when `key` is not
    /// the one already shown; `None` from it shows nothing. Returns whether it built.
    pub fn show(
        &self,
        key: impl Into<String>,
        build: impl FnOnce() -> Option<Box<dyn Component>>,
    ) -> bool {
        let key = key.into();
        if self.state.key.borrow().as_deref() == Some(key.as_str()) {
            return false;
        }
        *self.state.pending.borrow_mut() = build();
        *self.state.key.borrow_mut() = Some(key);
        // The words belong to the tree just built, so they are written onto it too.
        self.state.texts_dirty.set(true);
        true
    }

    /// **Write these words** — `(key of the widget, its text)` — onto the content, whenever they
    /// differ from the last ones given. They are never part of the content's identity.
    pub fn texts(&self, texts: Vec<(String, String)>) {
        if *self.state.texts.borrow() != texts {
            *self.state.texts.borrow_mut() = texts;
            self.state.texts_dirty.set(true);
        }
    }
}

impl Default for Keyed {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Keyed {
    /// Another node for the same content. A node placed anew holds nothing, so the next
    /// [`show`](Keyed::show) builds for it.
    fn clone(&self) -> Self {
        *self.state.key.borrow_mut() = None;
        let mut node = Self::new();
        node.state = self.state.clone();
        node
    }
}

impl Component for Keyed {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Take what was built, and write the words, before the layout pass reads this node.
    fn remeasure(&mut self) {
        if let Some(content) = self.state.pending.borrow_mut().take() {
            self.base.children.clear();
            self.base.children.push(content);
            self.base.mark_needs_layout();
        }
        if self.state.texts_dirty.replace(false) {
            for (key, text) in self.state.texts.borrow().iter() {
                crate::hint::set_text_by_key(&*self, key, text);
            }
        }
    }

    fn paint(&self, cx: &mut PaintCx) {
        for child in &self.base.children {
            crate::component::paint_child(child.as_ref(), cx);
        }
    }
}

impl LayoutExt for Keyed {}

#[cfg(test)]
mod tests;
