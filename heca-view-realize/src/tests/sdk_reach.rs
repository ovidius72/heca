//! Every widget property is reachable from the SDK.

use heca_grid_ui::{
    Alert, Badge, BadgeButton, Button, Checkbox, Choice, DockFrame, Gauge, Icon, IconButton, Input,
    Item, ItemGroup, KeyHintGroup, Label, LandingSlot, MarkerGroup, NfIcon, Overlay, Panel,
    ProgressBar, RailCell, Row as GridRow, ScrollRegion, Select, Separator, Splitter, Tabs, Tag,
    Toast, Toggle,
};
use heca_view::WidgetKind;

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

    let mut missing: Vec<String> = Vec::new();
    let mut check = |widget: &str, props: &[&str], sdk_name: &str| {
        let block = block_for(sdk_name).unwrap_or_default();
        for prop in props {
            let excused = NOT_IN_SDK
                .iter()
                .any(|(w, p, _)| *w == sdk_name && p == prop);
            if excused {
                continue;
            }
            // A setter reaches the property if it writes that key, whatever the method is
            // called: `Button::glowing` sets "glow", because `glow` on Style means the halo.
            let writes_key = block.contains(&format!("self.prop(\"{prop}\""))
                || block.contains(&format!("props.insert(\"{prop}\""));
            if !writes_key {
                missing.push(format!("{widget}::{prop} (builder {sdk_name})"));
            }
        }
    };

    check("Alert", <Alert as SetProp>::PROP_NAMES, "Alert");
    check("Badge", <Badge as SetProp>::PROP_NAMES, "Badge");
    check(
        "BadgeButton",
        <BadgeButton as SetProp>::PROP_NAMES,
        "BadgeButton",
    );
    check("Button", <Button as SetProp>::PROP_NAMES, "Button");
    check("Checkbox", <Checkbox as SetProp>::PROP_NAMES, "Checkbox");
    check("Choice", <Choice as SetProp>::PROP_NAMES, "Choice");
    check("DockFrame", <DockFrame as SetProp>::PROP_NAMES, "DockFrame");
    check("Gauge", <Gauge as SetProp>::PROP_NAMES, "Gauge");
    check("Icon", <Icon as SetProp>::PROP_NAMES, "Icon");
    check(
        "IconButton",
        <IconButton as SetProp>::PROP_NAMES,
        "IconButton",
    );
    check("Input", <Input as SetProp>::PROP_NAMES, "Input");
    check("Item", <Item as SetProp>::PROP_NAMES, "Item");
    check("ItemGroup", <ItemGroup as SetProp>::PROP_NAMES, "ItemGroup");
    check(
        "KeyHintGroup",
        <KeyHintGroup as SetProp>::PROP_NAMES,
        "KeyHintGroup",
    );
    check("Label", <Label as SetProp>::PROP_NAMES, "Label");
    check(
        "MarkerGroup",
        <MarkerGroup as SetProp>::PROP_NAMES,
        "MarkerGroup",
    );
    check("Overlay", <Overlay as SetProp>::PROP_NAMES, "Overlay");
    check("Panel", <Panel as SetProp>::PROP_NAMES, "Panel");
    check("RailCell", <RailCell as SetProp>::PROP_NAMES, "RailCell");
    check("Row", <GridRow as SetProp>::PROP_NAMES, "Row");
    check(
        "ScrollRegion",
        <ScrollRegion as SetProp>::PROP_NAMES,
        "Scroll",
    );
    check("Select", <Select as SetProp>::PROP_NAMES, "Select");
    check("Separator", <Separator as SetProp>::PROP_NAMES, "Separator");
    check("Splitter", <Splitter as SetProp>::PROP_NAMES, "Splitter");
    check(
        "LandingSlot",
        <LandingSlot as SetProp>::PROP_NAMES,
        "LandingSlot",
    );
    check("Tabs", <Tabs as SetProp>::PROP_NAMES, "Tabs");
    check("Tag", <Tag as SetProp>::PROP_NAMES, "Tag");
    check("Toast", <Toast as SetProp>::PROP_NAMES, "Toast");
    check("Toggle", <Toggle as SetProp>::PROP_NAMES, "Toggle");
    check(
        "ProgressBar",
        <ProgressBar as SetProp>::PROP_NAMES,
        "Progress",
    );
    check("NfIcon", <NfIcon as SetProp>::PROP_NAMES, "NfIcon");
    check(
        "ButtonGroup",
        <heca_grid_ui::widgets::ButtonGroup as SetProp>::PROP_NAMES,
        "ButtonGroup",
    );

    assert!(
        missing.is_empty(),
        "these widget properties have no setter in heca-view/src/build/, so a description \
         cannot reach them through the SDK: {missing:#?}\n\nAdd a setter, or add the property \
         to NOT_IN_SDK with the reason.",
    );

    // ── And the list above cannot silently fall behind the vocabulary ────────────────────
    //
    // Everything above is a **hand-written** call per widget, which is the shape this file's
    // own doc calls the defect: a list you must remember to add to. It failed exactly that way
    // — `Progress` was added to `WidgetKind` with a property, and every guard stayed green
    // because nobody had written its line, so the check silently covered 33 of 35 kinds.
    //
    // It cannot be derived (a kind does not name its Rust type), so instead the omission is
    // made loud: every kind must be checked above, or excused here by name with a reason.
    const NO_PROPERTIES_TO_CHECK: &[(&str, &str)] = &[
        ("VStack", "a plain box: `Flex` has only Style properties"),
        ("HStack", "the same box, laid out the other way"),
        ("Grid", "tracks are read by hand, not via the surface"),
        ("CardGrid", "cells are built by the realizer, not props"),
        ("Card", "a titled surface; its title is the `text` sugar"),
        ("Surface", "a plain painted box"),
        ("Spinner", "no properties; animates off the clock"),
        ("StatusDot", "its state picks the constructor"),
        ("ScrollBar", "host-only; `realize` refuses it"),
        (
            "Tile",
            "it only arranges: every part is a child placed by its `slot`",
        ),
    ];

    // This file's own text, read relative to the file itself, so moving it cannot lose it.
    let own_source = include_str!("sdk_reach.rs");
    // The third argument of each `check(..)` is the kind's SDK name — the one that has to line
    // up with `WidgetKind`.
    // Whitespace-insensitive on purpose: written to match `PROP_NAMES, "` it read nothing the
    // moment rustfmt wrapped one of these calls across lines, and a guard that quietly stops
    // reading is worse than the gap it exists for — which is why the size assertion below is
    // not decoration.
    let checked: std::collections::BTreeSet<&str> = own_source
        .match_indices("PROP_NAMES,")
        .filter_map(|(i, m)| {
            let rest = own_source[i + m.len()..].trim_start();
            let rest = rest.strip_prefix('"')?;
            rest.find('"').map(|e| &rest[..e])
        })
        .collect();
    assert!(
        checked.len() > 20,
        "this guard has stopped reading the check list, which is worse than the gap it exists \
         for — it found only {} entries",
        checked.len(),
    );

    let unchecked: Vec<String> = WidgetKind::ALL
        .iter()
        .map(|k| format!("{k:?}"))
        .filter(|name| !checked.contains(name.as_str()))
        .filter(|name| !NO_PROPERTIES_TO_CHECK.iter().any(|(n, _)| n == name))
        .collect();
    assert!(
        unchecked.is_empty(),
        "these kinds are in the vocabulary but no `check(..)` line above covers them, so their \
         properties could be unreachable from the SDK and nothing would say so: {unchecked:#?}\
         \n\nAdd a `check(..)` line, or name the kind in NO_PROPERTIES_TO_CHECK with a reason.",
    );
}
