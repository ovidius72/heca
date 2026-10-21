use super::*;

pub(super) fn build(_cx: &DemoCx<'_>) -> Box<dyn Component> {

    let report = |a: Action| println!("[showcase] {} -> {:?}", a.name, a.data);
    Box::new(Tabs::empty()
                .tab(
                    Choice::new("files")
                        .child(Icon::new(Glyph::FolderOpen))
                        .child(Label::new("FILES")),
                )
                .tab(
                    Choice::new("issues")
                        .child(Label::new("ISSUES"))
                        .child(Badge::danger("3")),
                )
                .tab(Choice::labeled("log", "LOG"))
                .selected(1)
                .on_change(report),)
}
