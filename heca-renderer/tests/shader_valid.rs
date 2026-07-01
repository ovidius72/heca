//! Offline WGSL validation: parse and validate each shader with `naga` so
//! shader errors are caught deterministically in CI without needing a GPU.

use naga::valid::{Capabilities, ValidationFlags, Validator};

fn validate(name: &str, src: &str) {
    let module = naga::front::wgsl::parse_str(src)
        .unwrap_or_else(|e| panic!("{name}: WGSL parse failed:\n{}", e.emit_to_string(src)));
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{name}: WGSL validation failed: {e:?}"));
}

#[test]
fn grid_shader_is_valid() {
    validate("grid.wgsl", include_str!("../src/grid.wgsl"));
}

#[test]
fn primitive_shader_is_valid() {
    validate("primitive.wgsl", include_str!("../src/primitive.wgsl"));
}

#[test]
fn text_shader_is_valid() {
    validate("text.wgsl", include_str!("../src/text.wgsl"));
}

#[test]
fn image_shader_is_valid() {
    validate("image.wgsl", include_str!("../src/image.wgsl"));
}
