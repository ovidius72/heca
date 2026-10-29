use crate::builders::{HintExt, Parent, PlaceExt, PresenceExt};
use crate::component::{Base, Component, Event, Handled};
use crate::focus::{focus_on_press, holder_path};
use crate::widgets::{Flex, Grid};

/// A widget the keyboard can land on — and, when `clickable`, one you can act on as well.
struct Control {
    base: Base,
}

impl Control {
    fn new() -> Self {
        let mut base = Base::new();
        base.focusable = true;
        Self { base }
    }

    fn clickable(mut self) -> Self {
        self.base.activatable = true;
        self
    }
}

impl Component for Control {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn on_event(&mut self, _: &Event) -> Handled {
        Handled::No
    }
}

/// A frame with a header and a content part, its content holding `inside` one level down — so
/// what takes the keyboard is not the part itself.
fn frame(header: impl Component + 'static, inside: impl Component + 'static) -> Grid {
    Grid::new()
        .template_row("auto 1fr")
        .template_area(["header", "content"])
        .child([
            crate::WidgetModel::new(Flex::row().area("header").child(header)),
            crate::WidgetModel::new(Flex::column().area("content").child(Flex::row().child(inside))),
        ])
}

/// **Pressing the frame gives the keyboard to what its content holds**, however deep it sits.
#[test]
fn a_press_on_the_frame_gives_the_keyboard_to_its_part() {
    let mut root = frame(Flex::row(), Control::new()).keyboard_to("content");
    focus_on_press(&mut root, &[]);
    assert_eq!(holder_path(&root), Some(vec![1, 0, 0]));
}

/// A press on a part of the frame that takes nothing — its header — lands in the content too.
#[test]
fn a_press_on_a_bare_part_of_the_frame_lands_in_the_content() {
    let mut root = frame(Flex::row(), Control::new()).keyboard_to("content");
    focus_on_press(&mut root, &[0]);
    assert_eq!(holder_path(&root), Some(vec![1, 0, 0]));
}

/// **Something that takes the keyboard itself keeps it** — a field in the header is not overruled
/// by the frame around it.
#[test]
fn a_press_on_something_that_takes_the_keyboard_keeps_it() {
    let mut root = frame(Control::new(), Control::new()).keyboard_to("content");
    focus_on_press(&mut root, &[0, 0]);
    assert_eq!(holder_path(&root), Some(vec![0, 0]));
}

/// A content part with nothing that takes the keyboard — a picture — changes nothing.
#[test]
fn a_part_with_nothing_to_take_it_changes_nothing() {
    let mut root = frame(Flex::row(), Flex::row()).keyboard_to("content");
    focus_on_press(&mut root, &[]);
    assert_eq!(holder_path(&root), None);
}

/// Targets in `root`, by path.
fn lettered(root: &dyn Component) -> Vec<Vec<usize>> {
    crate::hint::collect_hints(root)
        .into_iter()
        .map(|(path, _)| path)
        .collect()
}

/// **What the frame hands its keyboard to wears no letter of its own** — the frame's reaches it.
/// Without the hand-off it is lettered as anything that takes the keyboard is.
#[test]
fn what_the_frame_hands_the_keyboard_to_wears_no_letter() {
    let handing = frame(Flex::row(), Control::new())
        .keyboard_to("content")
        .on_hint(|| {});
    assert_eq!(lettered(&handing), vec![Vec::<usize>::new()]);

    let not_handing = frame(Flex::row(), Control::new()).on_hint(|| {});
    assert_eq!(lettered(&not_handing), vec![vec![], vec![1, 0, 0]]);
}

/// **Even one you can click** — the parent's letter is how you reach it.
#[test]
fn a_handed_to_widget_you_can_click_wears_no_letter_either() {
    let root = frame(Flex::row(), Control::new().clickable())
        .keyboard_to("content")
        .on_hint(|| {});
    assert_eq!(lettered(&root), vec![Vec::<usize>::new()]);
}

/// **A pick it declared itself stays** — its author said a pick there means something else.
#[test]
fn a_handed_to_widget_with_its_own_pick_keeps_its_letter() {
    let root = frame(Flex::row(), Control::new().on_hint(|| {}))
        .keyboard_to("content")
        .on_hint(|| {});
    assert_eq!(lettered(&root), vec![vec![], vec![1, 0, 0]]);
}
