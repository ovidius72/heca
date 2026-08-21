//! [`KeyHintGroup`] — **a picker you can declare**, over a subtree you choose.
//!
//! [`KeyHint`](super::KeyHint) makes one region pickable. This opens a picker over *many* of them:
//! while it is open it letters every hint declaration beneath it, takes the keyboard, and runs the
//! one whose letter you type.
//!
//! # Why this is a widget and not a host facility
//!
//! It used to be host-only, and that made the picker a shipped feature nobody outside the app could
//! have. The letters were assigned by a host pass that walked every retained tree on screen, and
//! the "next key picks" state was a host input mode. A plugin could *contribute targets* to heca's
//! picker and could not have one of its own — the exact second-class shape ⭐⭐ RULE ZERO forbids.
//!
//! Nothing about it actually needed the host. A widget can hold focus, and a focused widget gets
//! the keys: [`Overlay`](super::Overlay) and [`ContextMenu`](super::ContextMenu) are already
//! modal this way. So the whole picker is:
//!
//! 1. walk my own descendants for hint declarations,
//! 2. hand each one a letter — [`Base::hint_label`], which the declaring widget **draws itself**,
//! 3. hold focus while open, and on the next typed character fire that declaration.
//!
//! The host keeps exactly one thing, because only it can answer it: **which surfaces are
//! eligible** when the picker's scope is the whole screen (is this card covered by the sidebar? is
//! there a modal layer above it?). `prefix+/` is this widget's behaviour at screen scope with that
//! filter applied; a plugin's is the same behaviour scoped to its own panel.
//!
//! ```
//! use heca_grid_ui::prelude::*;
//! use heca_grid_ui::widgets::{Flex, KeyHint, KeyHintGroup, Label, Row};
//! use heca_grid_ui::reactive::signal;
//!
//! # fn choose(_: u32) {}
//! let open = signal(false);          // the host flips this from an action
//! let picker = KeyHintGroup::new(
//!     Flex::column()
//!         .child(KeyHint::new(Row::new().child(Label::new("one"))).on_hint(|| choose(1)))
//!         .child(KeyHint::new(Row::new().child(Label::new("two"))).on_hint(|| choose(2))),
//! )
//! .open_when(open);
//! ```

use crate::component::{Base, Component};
use crate::event::{Event, Handled, WidgetIntent};
use crate::PaintCx;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::style::{Direction, Length};

/// **The letters a picker hands out when its caller does not say otherwise**, in order.
///
/// **Home row first** — `asdfghjkl`, then the rest of the alphabet, then the same again shifted
/// (F003/P082/T443). The letters are spent in the order they are listed, so the targets a picker
/// finds first get the keys your fingers are already resting on. Antonio, 2026-08-17: *"a way to
/// prefer row keys?"*
///
/// Lower case before capitals because they are one keystroke on every layout; the capitals extend
/// the run for a dense surface without introducing a modifier, which would be a second gesture
/// rather than a longer alphabet.
///
/// **One letter per pick, always, and 52 is the cap.** Past the end of this sequence a target simply
/// gets none. Two-key sequences were raised and refused (Antonio, 2026-08-17: *"typing 2 letters is
/// not an option. always 1. stay with 52."*) — anything needing more than 52 at once is a picker
/// covering too much, and the answer is a smaller picker, never a longer keystroke.
///
/// It is public so a host can hand the same order to its own pickers instead of keeping a second
/// copy of this decision.
pub const DEFAULT_LETTERS: &str =
    "asdfghjklbceimnopqrtuvwxyzASDFGHJKLBCEIMNOPQRTUVWXYZ";

/// A picker over the subtree it wraps: while open, every hint declaration beneath it wears a
/// letter, and typing one runs it.
///
/// A transparent wrapper, like [`KeyHint`](super::KeyHint) and
/// [`FocusScope`](super::FocusScope): layout, paint and pointer events pass straight through, so
/// the subtree behaves exactly as it did unwrapped while the picker is closed.
pub struct KeyHintGroup {
    base: Base,
    /// Host-owned: `true` shows the letters and takes the keyboard.
    open: Signal<bool>,
    /// Last value seen by [`tick`](Component::tick), so opening and closing are edges rather than
    /// something re-derived every frame.
    seen: bool,
    /// The letters this picker hands out, in order — [`DEFAULT_LETTERS`] unless the caller replaced
    /// them with [`letters`](KeyHintGroup::letters).
    letters: Vec<char>,
}

#[heca_grid_ui_macros::props]
impl KeyHintGroup {
    /// Wrap `child`. Bind the open state with [`open_when`](KeyHintGroup::open_when).
    pub fn new(child: impl Component + 'static) -> Self {
        Self::wrap(Box::new(child))
    }

    /// Wrap an **already-boxed** subtree — what a dynamically built tree is (`realize` output, a
    /// provider's render seam), where the concrete widget type is not known at the call site.
    pub fn new_boxed(child: Box<dyn Component>) -> Self {
        Self::wrap(child)
    }

    fn wrap(child: Box<dyn Component>) -> Self {
        let mut base = Base::new();
        // Hug the child and stay a column, so a stretching parent reaches the subtree unchanged —
        // the same transparency `KeyHint` and `FocusScope` need, for the same reason.
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        base.style.layout.direction = Direction::Column;
        base.children.push(child);
        Self { base, open: signal(false), seen: false, letters: DEFAULT_LETTERS.chars().collect() }
    }

    /// Bind the **host-owned** open signal. Set it from an action — which is how a surface gives
    /// its picker a binding without any widget naming a key.
    #[heca_grid_ui_macros::host_only("bound to a live host signal, which static data cannot drive")]
    pub fn open_when(mut self, open: Signal<bool>) -> Self {
        self.seen = open.get_untracked();
        self.open = open;
        // **Holding focus is the whole of taking the keyboard.** Keys are delivered down the focus
        // owner's chain, so an open picker is on the path and a closed one is not. There is no
        // gate to write and nothing to decline.
        self.base.focused = open;
        self
    }

    /// **The letters this picker hands out, in order.** Defaults to [`DEFAULT_LETTERS`].
    ///
    /// ```
    /// use heca_grid_ui::widgets::{Flex, KeyHintGroup};
    ///
    /// // Home row first, so the nearest targets cost the least reach.
    /// let picker = KeyHintGroup::new(Flex::column()).letters("asdfghjkl".chars());
    /// ```
    ///
    /// **The library owns the mechanism; the caller owns the choice.** Which letters, in what order,
    /// and which to keep free is a decision about *an app*, and it was baked into this widget — so
    /// nobody reusing the library could ask for home-row ordering, a different keyboard layout, or a
    /// letter reserved for something else (F003/P082/T433).
    ///
    /// Still exactly one keystroke per pick: a target past the end of the sequence gets no letter
    /// rather than a longer one. See [`DEFAULT_LETTERS`].
    #[heca_grid_ui_macros::host_only("an app's choice of alphabet, not data a described tree carries")]
    pub fn letters(mut self, letters: impl IntoIterator<Item = char>) -> Self {
        self.letters = letters.into_iter().collect();
        self
    }

    /// Is the picker showing its letters?
    pub fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// The open signal, for a host that drives it directly.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    /// Every hint declaration beneath this group, in document order — **not including itself**, so
    /// a group nested in another group is a target of the outer one only through its children.
    fn targets(&self) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        for (i, child) in self.base.children.iter().enumerate() {
            let mut here = vec![i];
            collect(child.as_ref(), &mut here, &mut out);
        }
        out
    }

    /// Hand out the letters, or take them all back.
    fn set_letters(&self, open: bool) {
        let targets = self.targets();
        if !open {
            for path in &targets {
                if let Some(node) = self.at(path) {
                    node.base().hint_label.set(None);
                }
            }
            return;
        }
        for (path, ch) in targets.iter().zip(self.letters.iter().copied()) {
            if let Some(node) = self.at(path) {
                node.base().hint_label.set(Some(ch.to_string()));
            }
        }
    }

    /// The descendant at `path`, counted from this group's own children.
    fn at(&self, path: &[usize]) -> Option<&dyn Component> {
        let mut node: &dyn Component = self.base.children.get(*path.first()?)?.as_ref();
        for step in &path[1..] {
            node = node.base().children.get(*step)?.as_ref();
        }
        Some(node)
    }
}

fn collect(node: &dyn Component, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    if !node.base().visible.get_untracked() || node.base().style.layout.hidden {
        return;
    }
    if node.base().hint.is_some() {
        out.push(path.clone());
    }
    for (i, child) in node.base().children.iter().enumerate() {
        path.push(i);
        collect(child.as_ref(), path, out);
        path.pop();
    }
}

impl Component for KeyHintGroup {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn tick(&mut self, dt: f32) -> bool {
        let open = self.open.get_untracked();
        if open != self.seen {
            self.seen = open;
            // The letters are set on the **edge**, not re-derived every frame: a picker that
            // reassigned each tick would renumber its targets under the user's fingers whenever
            // anything below it changed.
            self.set_letters(open);
            self.base.mark_needs_paint();
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// **Capture, not bubble.** The letter has to be claimed before the subtree sees it: the
    /// regions a picker covers commonly answer typed characters themselves (the exposé's cards take
    /// `x`, `r` and `d`), and while the picker is open those letters belong to it.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if !self.open.get_untracked() {
            return Handled::No;
        }
        match ev {
            // Typed text, not a key: `a` and `A` are the same *key* and differ only as text, which
            // is what lets the alphabet run past twenty-six without a modifier.
            Event::TextInput(typed) => {
                let picked = self
                    .targets()
                    .into_iter()
                    .find(|path| {
                        self.at(path)
                            .and_then(|n| n.base().hint_label.get_untracked())
                            .is_some_and(|l| l == *typed)
                    });
                // Closed before firing, and the letters come down with it: running a pick may tear
                // the tree down, and a keycap must not outlive the picker that put it up.
                self.open.set(false);
                self.seen = false;
                self.set_letters(false);
                if let Some(path) = picked
                    && let Some(node) = self.at(&path)
                    && let Some(hint) = &node.base().hint
                {
                    hint.run();
                }
                Handled::Yes
            }
            // Dismissal is the shared vocabulary's, so the key is whatever `[keys.widgets] dismiss`
            // says — this widget carries no key of its own.
            Event::Widget(WidgetIntent::Dismiss) => {
                self.open.set(false);
                self.seen = false;
                self.set_letters(false);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        for child in &self.base.children {
            crate::component::paint_child(child.as_ref(), cx);
        }
    }

}

impl crate::builders::LayoutExt for KeyHintGroup {}
impl crate::builders::StyleExt for KeyHintGroup {}
impl crate::builders::Parent for KeyHintGroup {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{ComponentExt as _, Parent as _};
    use crate::widgets::{Flex, KeyHint, Label, Row};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Picks = Rc<RefCell<Vec<u32>>>;

    fn group(open: Signal<bool>) -> (KeyHintGroup, Picks) {
        let picks: Picks = Rc::new(RefCell::new(Vec::new()));
        let row = |id: u32| {
            let picks = picks.clone();
            KeyHint::new(Row::new().child(Label::new(format!("row {id}"))))
                .on_hint(move || picks.borrow_mut().push(id))
        };
        let g = KeyHintGroup::new(Flex::column().child(row(1)).child(row(2)).child(row(3)))
            .open_when(open);
        (g, picks)
    }

    fn label_of(g: &KeyHintGroup, nth: usize) -> Option<String> {
        let paths = g.targets();
        g.at(&paths[nth])?.base().hint_label.get_untracked()
    }

    /// **Opening letters its own descendants, in document order** — and nothing else on screen is
    /// involved, which is the whole difference from the host picker.
    #[test]
    fn opening_letters_the_subtree_in_order() {
        let open = signal(false);
        let (mut g, _) = group(open);
        assert_eq!(label_of(&g, 0), None, "closed, nothing wears a letter");

        open.set(true);
        g.tick(0.0);

        // The first three of `DEFAULT_LETTERS`, which is home row first (F003/P082/T443).
        assert_eq!(label_of(&g, 0).as_deref(), Some("a"));
        assert_eq!(label_of(&g, 1).as_deref(), Some("s"));
        assert_eq!(label_of(&g, 2).as_deref(), Some("d"));
    }

    /// **The caller's alphabet, in the caller's order** (F003/P082/T433). Which letters to hand out
    /// is a decision about an app — home row first, a different layout, a letter kept free — and it
    /// was baked into this widget, so nobody reusing the library could make it.
    #[test]
    fn the_caller_chooses_the_letters_and_their_order() {
        let open = signal(false);
        let (g, _) = group(open);
        let mut g = g.letters("jkl".chars());

        open.set(true);
        g.tick(0.0);

        assert_eq!(label_of(&g, 0).as_deref(), Some("j"));
        assert_eq!(label_of(&g, 1).as_deref(), Some("k"));
        assert_eq!(label_of(&g, 2).as_deref(), Some("l"));
    }

    /// **One keystroke per pick, and a target past the end simply gets none** — never a second
    /// letter to type. Antonio, 2026-08-17: *"typing 2 letters is not an option. always 1."*
    #[test]
    fn running_out_of_letters_gives_none_rather_than_a_longer_one() {
        let open = signal(false);
        let (g, _) = group(open);
        let mut g = g.letters("x".chars()); // one letter, three targets

        open.set(true);
        g.tick(0.0);

        assert_eq!(label_of(&g, 0).as_deref(), Some("x"));
        assert_eq!(label_of(&g, 1), None, "no letter rather than a two-key sequence");
        assert_eq!(label_of(&g, 2), None);
    }

    /// The default is 52 single keystrokes — the cap, stated once so a change to it is deliberate.
    /// **Home row first**, and every letter used exactly once (F003/P082/T443).
    #[test]
    fn the_default_alphabet_is_fifty_two_single_keystrokes_home_row_first() {
        assert_eq!(DEFAULT_LETTERS.chars().count(), 52);
        assert!(DEFAULT_LETTERS.chars().all(|c| c.is_ascii_alphabetic()));

        assert!(
            DEFAULT_LETTERS.starts_with("asdfghjkl"),
            "the keys your fingers rest on are spent first"
        );

        // No letter handed out twice, and none missing — a duplicate would give two targets the
        // same key, and an omission would waste one of the 52.
        let mut seen: Vec<char> = DEFAULT_LETTERS.chars().collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 52, "every letter appears exactly once");
    }

    /// Typing a letter runs **that** declaration and closes the picker.
    #[test]
    fn typing_a_letter_runs_that_declaration() {
        let open = signal(false);
        let (mut g, picks) = group(open);
        open.set(true);
        g.tick(0.0);

        // `s` is the second letter of `DEFAULT_LETTERS` — home row first (F003/P082/T443).
        let handled = g.on_event_capture(&Event::TextInput("s".to_string()));

        assert_eq!(handled, Handled::Yes, "the picker claims the letter");
        assert_eq!(*picks.borrow(), vec![2], "the second row's own declaration ran");
        assert!(!g.is_open(), "and the picker closed behind it");
        assert_eq!(label_of(&g, 0), None, "with every letter taken back");
    }

    /// **The letters are claimed in capture**, before the subtree sees them. The regions a picker
    /// covers commonly answer typed characters themselves — the exposé's cards take `x`, `r`, `d` —
    /// and while it is open those letters are the picker's.
    #[test]
    fn a_letter_never_reaches_the_subtree_while_the_picker_is_open() {
        let open = signal(false);
        let (mut g, picks) = group(open);
        open.set(true);
        g.tick(0.0);

        assert_eq!(
            g.on_event_capture(&Event::TextInput("a".to_string())),
            Handled::Yes,
        );
        assert_eq!(*picks.borrow(), vec![1]);
    }

    /// A key nobody was given still closes it — the picker is modal while open, so a stray letter
    /// does not leave the letters up and the keyboard captured.
    #[test]
    fn an_unassigned_letter_closes_without_picking() {
        let open = signal(false);
        let (mut g, picks) = group(open);
        open.set(true);
        g.tick(0.0);

        g.on_event_capture(&Event::TextInput("z".to_string()));

        assert!(picks.borrow().is_empty(), "nothing ran");
        assert!(!g.is_open(), "and it is closed");
    }

    /// Dismissal comes from the shared `[keys.widgets]` vocabulary, so this widget names no key.
    #[test]
    fn dismiss_puts_the_letters_away() {
        let open = signal(false);
        let (mut g, picks) = group(open);
        open.set(true);
        g.tick(0.0);

        g.on_event_capture(&Event::Widget(WidgetIntent::Dismiss));

        assert!(picks.borrow().is_empty());
        assert!(!g.is_open());
        assert_eq!(label_of(&g, 0), None);
    }

    /// **Closed, it is not there.** A picker that only lets its letters through while open is the
    /// difference between a wrapper and a mode: the subtree must behave exactly as it did unwrapped
    /// the rest of the time.
    #[test]
    fn a_closed_picker_claims_nothing() {
        let open = signal(false);
        let (mut g, _) = group(open);
        assert_eq!(
            g.on_event_capture(&Event::TextInput("a".to_string())),
            Handled::No,
        );
    }
}
