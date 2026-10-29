//! Every widget property is reachable from the SDK.

use super::*;

/// Every widget property is reachable from the typed SDK.
///
/// `heca-view::build` is hand-written — that was the choice, over generating it — so the thing
/// that keeps it honest is this. It walks each widget's generated `PROP_NAMES` and fails when a
/// property has no named setter on that kind's builder, which is how a capability added to the
/// library reaches the authoring layer instead of quietly not existing.
///
/// It **fails closed**: a property must be reachable unless it is named below with a reason.
/// The opposite arrangement — a list you must remember to add to — is what let `placeholder`
/// and the scroll axes go unreachable for months.
///
/// The check reads the SDK's source rather than calling it, because "does a method exist" is
/// not a question a running test can ask. That is the same technique `prop_drift.rs` uses, and
/// for the same reason.
#[test]
fn every_widget_property_is_reachable_from_the_sdk() {
    /// Properties with no setter, each with the reason. Keep it short — an entry here is a
    /// capability an author cannot reach.
    const NOT_IN_SDK: &[(&str, &str, &str)] = &[
        (
            "Button",
            "accent",
            "on Style already — the accent is a style property every kind carries, and it \
             reaches the subtree too, so a per-kind copy would be a second way to say it",
        ),
        (
            "IconButton",
            "accent",
            "on Style already — see Button::accent",
        ),
        (
            "Button",
            "font_size",
            "on Style already — every kind takes font_size, so a per-kind copy would be a \
             second way to say the same thing",
        ),
        ("Input", "font_size", "on Style already"),
        ("Select", "font_size", "on Style already"),
        ("Tabs", "font_size", "on Style already"),
        ("Item", "font_size", "on Style already"),
        ("Label", "font_size", "on Style already"),
        ("Label", "font_scale", "on Style already"),
    ];

    let sdk = sdk_source();

    // Which builder impl a method sits in: the guard is per-kind, so a setter on the wrong
    // builder must not satisfy another's property.
    let block_for = |widget: &str| -> Option<String> {
        let head = format!("\nimpl {widget} {{\n");
        let start = sdk.find(&head)? + head.len();
        let rest = &sdk[start..];
        let end = rest.find("\n}\n").unwrap_or(rest.len());
        Some(rest[..end].to_string())
    };

    let registry = WidgetRegistry::builtins().expect("library definitions");
    let shared_style = sdk.split("pub trait Style: Sized {").nth(1)
        .and_then(|style| style.split("\n}\n").next()).unwrap_or("");
    let mut missing = Vec::new();
    for definition in registry.definitions() {
        let metadata = definition.metadata();
        let sdk_name = metadata.sdk_name;
        assert!(!sdk_name.is_empty(), "{} has no typed SDK identity", metadata.id);
        let block = block_for(sdk_name).unwrap_or_default();
        for prop in &metadata.properties {
            if NOT_IN_SDK.iter().any(|(widget, property, _)| *widget == sdk_name && *property == prop) {
                continue;
            }
            let writes_key = block.contains(&format!(".prop(\"{prop}\""))
                || block.contains(&format!("props.insert(\"{prop}\""))
                || shared_style.contains(&format!("fn {prop}("));
            if !writes_key {
                missing.push(format!("{}::{prop} (builder {sdk_name})", metadata.native_name));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "these widget properties have no setter in heca-view/src/build/, so a description \
         cannot reach them through the SDK: {missing:#?}\n\nAdd a setter, or add the property \
         to NOT_IN_SDK with the reason.",
    );

    // The actual factory definitions supply the checked inventory, including constructor
    // inputs and state adapters. There is no second per-widget check list or exemption list.
    assert!(registry.definitions().count() > 30, "the registration inventory must not be empty");
}
