//! **What a widget does in a drag**: whether it can be picked up, what it is, and what it takes.

use crate::component::Component;

/// **What a widget does in a drag** — whether it can be picked up and what it says it is, and
/// whether it takes what is dropped on it.
pub trait DragExt: Component + Sized {
    /// **This widget can be dragged**, and what gets dragged is the identity it already declares
    /// with [`key`](super::IdentityExt::key) — the same one the keyboard cursor and the right-click
    /// target read.
    ///
    /// ```ignore
    /// Row::new().key("pane:7").draggable().drop_target()
    /// ```
    ///
    /// One declaration, and nothing to hand out: it took an opaque id from a host registry before,
    /// beside a closed list of which surfaces were even allowed to drag — so a row named itself
    /// twice and a plugin's row could not be named at all. A widget with no `key` is not a drag
    /// source; there would be nothing to say about what was picked up.
    fn draggable(mut self) -> Self {
        self.base_mut().draggable = true;
        self
    }

    /// **This widget can be dragged, and it says what it is** — an opaque word its component
    /// chose (`"pane"`, `"column"`, `"docker.container"`), so a target can take some things and
    /// refuse others.
    ///
    /// ```ignore
    /// Row::new().key("col:3").draggable_as("column").drop_target().accepts(["column"])
    /// ```
    ///
    /// Say nothing and every target takes it, which is what [`draggable`](Self::draggable) alone
    /// means. The word is never interpreted by the library, and it is not from a list the library
    /// knows — a closed set is one a plugin cannot join.
    ///
    /// Takes an `Option` too ([`IntoName`](super::IntoName)): `None` leaves the widget as it was,
    /// not draggable, so a row that only sometimes can be picked up needs no branch.
    fn draggable_as(mut self, kind: impl super::IntoName) -> Self {
        if let Some(kind) = kind.into_name() {
            let b = self.base_mut();
            b.draggable = true;
            b.drag_kind = Some(kind);
        }
        self
    }

    /// **Carry the small title chip under the pointer instead of a picture of this widget.**
    ///
    /// A widget being dragged shows a picture of itself by default, as the browser does: the widget
    /// painted as it is (a terminal's surface included), in the top layer, at the theme's
    /// `drag_image_alpha` and `drag_image_scale`, with the point you grabbed kept under the
    /// pointer. Ask for the chip when the picture would say less than its title — a source far
    /// larger than any place it can go. Nothing else about the drag changes.
    fn drag_chip(mut self) -> Self {
        self.base_mut().drag_chip = true;
        self
    }

    /// **Pick it up only by its handle** — a widget inside it marked
    /// [`drag_handle`](Self::drag_handle). A press anywhere else in it starts no drag of it, so its
    /// content keeps its own clicks and selection. [`DockFrame`](crate::widgets::DockFrame) does
    /// this with its grip.
    ///
    /// ```ignore
    /// Flex::row().key("card:1").draggable_as("card").drag_by_handle()
    ///     .child(Label::new("≡").drag_handle())
    ///     .child(content)
    /// ```
    fn drag_by_handle(mut self) -> Self {
        self.base_mut().drag_by_handle = true;
        self
    }

    /// **This widget is a handle**: a press on it picks up the nearest enclosing widget that is
    /// [`drag_by_handle`](Self::drag_by_handle). One that is not draggable leaves the handle doing
    /// nothing — it never reaches past to pick up something further out.
    fn drag_handle(mut self) -> Self {
        self.base_mut().drag_handle = true;
        self
    }

    /// **Make it draggable only while `rule` holds** for what is held down when the press moves
    /// far enough to start a drag — `draggable_when(|m| m.meta)` is "Cmd+drag". Without the rule
    /// the press stays whatever the widget's content makes of it (a terminal's selection).
    ///
    /// The rule is asked when a drag would start, never again: a drag in flight does not end
    /// because the key came up.
    ///
    /// **Holding the key is a deliberate request**, so a source picked up by its rule wins over
    /// anything inside it that would start on any press — Cmd+drag on a row inside a dock carries
    /// the dock, not the row.
    fn draggable_when(mut self, rule: impl Fn(crate::event::Modifiers) -> bool + 'static) -> Self {
        self.base_mut().drag_gate = Some(std::rc::Rc::new(rule));
        self
    }

    /// **This widget accepts drops**, identified the same way — by its
    /// [`key`](super::IdentityExt::key). A drag released over its bounds drops onto it, with the side
    /// (before / onto / after) computed from where in its bounds the pointer sits.
    fn drop_target(mut self) -> Self {
        self.base_mut().drop_target = true;
        self
    }

    /// **What this target takes**, by the words a drag source names itself with
    /// ([`draggable_as`](Self::draggable_as)). Anything else is refused *before* it is drawn on, so
    /// a line never appears over something that would then do nothing.
    ///
    /// Declaring nothing takes anything. Implies [`drop_target`](Self::drop_target), because a
    /// widget saying what it takes has said it takes something.
    ///
    /// Takes an `Option` too ([`IntoNames`](super::IntoNames)): `None` leaves the widget as it was,
    /// not a drop target.
    fn accepts(mut self, kinds: impl super::IntoNames) -> Self {
        if let Some(kinds) = kinds.into_names() {
            let b = self.base_mut();
            b.drop_target = true;
            b.accepts = kinds;
            b.accepts_onto = true;
        }
        self
    }

    /// **What this target takes *beside* itself** — for a sibling in an ordered list, where being
    /// dropped *onto* it means nothing.
    ///
    /// A column dropped on a column is a reorder: it can only land before or after, so the target
    /// reads as two halves and the line flips at the midpoint — never a third band that silently
    /// picks one for you. Use [`accepts`](Self::accepts) where the middle is real, as a pane is for
    /// another pane, which it swaps or moves onto.
    ///
    /// Takes an `Option` too, like [`accepts`](Self::accepts): `None` leaves the widget as it was.
    fn accepts_beside(mut self, kinds: impl super::IntoNames) -> Self {
        if let Some(kinds) = kinds.into_names() {
            let b = self.base_mut();
            b.drop_target = true;
            b.accepts = kinds;
            b.accepts_onto = false;
        }
        self
    }
}

impl<T: Component + Sized> DragExt for T {}
