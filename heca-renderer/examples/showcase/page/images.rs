//! Native and described images use the same loaders, sizing and scene commands.
use super::*;
use heca_grid_ui::{Image, ImageFit, Svg};
use heca_view::{build, ViewImageFit};
use heca_view::build::Style as _;

pub(super) fn build(theme: &Theme) -> Flex {
    let svg = include_str!("../../../assets/showcase/image.svg");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/showcase").to_string_lossy().into_owned();
    let described = build::Image::load("image.svg").asset_root(root.clone())
        .fit(ViewImageFit::Cover).width_pct(1.0).aspect_ratio(1.0)
        .alt("Described SVG, cover");
    let emit: heca_view_realize::IntentEmitter = Rc::new(|_| {});
    let described = heca_view_realize::realize(&described, theme, &emit, &mut Default::default())
        .expect("the compiled image fixture must match the library registry");
    Flex::column().gap("sm")
        .child(Label::new("Image — SVG / PNG / JPEG · shared loader and layout").bold(true))
        .child(Grid::new().template_column("1fr 1fr 1fr 1fr").gap("md")
            .child(Flex::column().gap("sm").child(Label::new("Inline SVG · contain"))
                .child(Svg::new(svg).width("100%").aspect_ratio(1.0).fit(ImageFit::Contain).alt("Inline vector")))
            .child(Flex::column().gap("sm").child(Label::new("PNG · cover"))
                .child(Image::load("image.png").asset_root(root.clone()).width("100%").aspect_ratio(1.0).fit(ImageFit::Cover).alt("PNG cover")))
            .child(Flex::column().gap("sm").child(Label::new("JPEG · stretch"))
                .child(Image::load("image.jpg").asset_root(root).width("100%").aspect_ratio(1.0).fit(ImageFit::Stretch).alt("JPEG stretch")))
            .child(Flex::column().gap("sm").child(Label::new("Described SVG · cover")).child(described)))
}
