use super::*;

pub(super) fn build(cx: &DemoCx<'_>) -> Box<dyn Component> {
    let theme = cx.theme;


    Box::new({
            let selected = signal(0usize);
            let states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
            let select = move |i: usize, item: Item| -> Item {
                states.borrow_mut().push(item.state());
                let states = states.clone();
                item.on_activate(move || {
                    selected.set(i);
                    for (j, s) in states.borrow().iter().enumerate() {
                        s.set(j == i);
                    }
                })
            };
            Pane::new()
                .width(320.0)
                .gap(2.0)
                .background(theme.colors.surface)
                .child(select(
                    0,
                    Item::new("DASHBOARD")
                        .leading(StatusDot::online())
                        .active(true)
                        .marker(ActiveMarker::Bar),
                ))
                .child(select(
                    1,
                    Item::new("VIEW PROFILE")
                        .marker(ActiveMarker::Bar)
                        .trailing(Tag::new("CMD P")),
                ))
                .child(select(
                    2,
                    Item::new("SETTINGS")
                        .marker(ActiveMarker::Bar)
                        .trailing(Tag::new("CMD ,")),
                ))
                .child(
                    Item::new("SYSTEM")
                        .muted(true)
                        .trailing(Label::new(">").color(theme.colors.muted)),
                )
        })
}
