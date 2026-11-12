use std::rc::Rc;
use heca_grid_ui::{*, builders::Parent};
use heca_view::{build::{self as view, Parent as _, Style as _}, ViewBadgeVariant, ViewDotStatus, ViewGlyph, ViewSeverity};
use heca_view_realize::{realize, FormBindings, IntentEmitter};

fn same_scene(mut native: impl Component, node: &impl AsRef<heca_view::ViewNode>) {
    let theme = Theme::default();
    let emit: IntentEmitter = Rc::new(|_| {});
    let mut described = realize(node, &theme, &emit, &mut FormBindings::default()).unwrap();
    fn scene(widget: &mut dyn Component, theme: &Theme) -> Vec<DrawCommand> {
        LayoutEngine::new().compute(widget, Size::new(400.0, 180.0));
        let mut scene = Scene::new(); paint_child(widget, &mut PaintCx::new(&mut scene, theme));
        scene.iter().cloned().collect()
    }
    assert_eq!(scene(&mut native, &theme), scene(described.as_mut(), &theme));
}

#[test]
fn caption_constructors_and_properties_use_the_same_composed_widget() {
    for text in ["Build", "", "   ", "A long deployment caption that must fit"] {
        same_scene(Caption::new("Initial").text(text).gap("sm").padding_x("md"),
            &view::Caption::new("Initial").text(text).gap("sm").padding_x("md"));
    }
    let registry = heca_view_realize::WidgetRegistry::builtins().unwrap();
    let definition = registry.get(&heca_view::WidgetId::new("caption")).unwrap();
    assert!(definition.metadata().properties.iter().any(|property| property == "text"));
    assert_eq!(definition.metadata().native_name, "Caption");
}

#[test]
fn badge_variants_and_alert_content_reach_the_same_native_painter() {
    use heca_grid_ui::widgets::{AlertVariant, BadgeVariant};
    for (native, described) in [
        (BadgeVariant::Accent, ViewBadgeVariant::Accent), (BadgeVariant::Neutral, ViewBadgeVariant::Neutral),
        (BadgeVariant::Success, ViewBadgeVariant::Success), (BadgeVariant::Warning, ViewBadgeVariant::Warning),
        (BadgeVariant::Danger, ViewBadgeVariant::Danger), (BadgeVariant::Outline, ViewBadgeVariant::Outline),
    ] { same_scene(Badge::new("State").variant(native), &view::Badge::new("State").variant(described)); }
    for (native, described) in [
        (AlertVariant::Info, ViewSeverity::Info), (AlertVariant::Success, ViewSeverity::Success),
        (AlertVariant::Warning, ViewSeverity::Warning), (AlertVariant::Danger, ViewSeverity::Danger),
    ] { same_scene(Alert::new("Status").variant(native).body("Details"), &view::Alert::new().text("Status").severity(described).body("Details")); }
}

#[test]
fn tags_keep_segments_and_icon_properties_reach_the_native_icon() {
    same_scene(Tag::new("Title").segment(Flex::row().child(Label::new("Detail"))),
        &view::Tag::new("Title").child(view::HStack::new().child(view::Label::new("Detail"))));
    same_scene(Icon::new(Glyph::Terminal).size(12.0), &view::Icon::new().glyph(ViewGlyph::Terminal).size_px(12.0));
}

#[test]
fn all_dot_states_use_the_native_status_signal() {
    for (native, described) in [
        (DotStatus::Online, ViewDotStatus::Online), (DotStatus::Warning, ViewDotStatus::Warning),
        (DotStatus::Error, ViewDotStatus::Error), (DotStatus::Offline, ViewDotStatus::Offline),
    ] { same_scene(StatusDot::new(native), &view::StatusDot::new().status(described)); }
}
