//! Merging a node's appearance and layout props into the widget's own, resolved against the theme.

use heca_grid_ui::{PropInput, SetProp, Theme};
use heca_view::{PropValue, ViewNode};

/// Overlay a node's properties onto one half of an **already-constructed** widget's style —
/// [`Layout`](heca_grid_ui::style::Layout) or [`Visual`](heca_grid_ui::style::Visual). `None` when the node
/// changes nothing about this half.
///
/// Generic over the half because the two are merged identically: read the live value as a JSON
/// object, overlay only the keys the node actually carries, read it back. It was written for
/// `Layout` alone; pointing it at `Visual` too was all that was needed on this side, which is
/// why the split between the halves was worth keeping rather than undoing.
///
/// On top, not from scratch: widgets set deliberate non-default style in their constructors
/// (`ScrollRegion::new()` zeroes its min sizes and opts into shrinking so a viewport can be smaller
/// than its content). Rebuilding from `Default` would silently break those.
///
/// Total for untrusted input, as the rest of `realize` is: an unknown key is skipped, and a value
/// that does not fit its field is dropped *individually* — one bad property never discards the
/// good ones and never panics.
/// **A property name a description may still be written with.**
///
/// The described vocabulary IS the layout field list — there is no table of allowed names, which
/// is what lets a new layout setting reach plugins with nothing to update (`plugins.md` §8). The
/// cost of that is a *renamed* field: the old spelling stops naming anything and is dropped in
/// silence. Spacing was written as two settings — a px `gap` beside a `gap_spacing` step — and is
/// now one that takes either, so the retired names are mapped onto the surviving one here rather
/// than left to fail quietly.
///
/// This is the whole list, and it should stay short: renaming a layout field is a wire break, and
/// the alternative to this map is telling every plugin author to rewrite their tree.
pub(crate) fn retired_name(key: &str) -> String {
    match key {
        "gap_spacing" => "gap".to_string(),
        "pad_spacing_x" => "padding_x".to_string(),
        "pad_spacing_y" => "padding_y".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn merge_style_half<T>(node: &ViewNode, theme: &Theme, current: T) -> Option<T>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    if node.props.is_empty() {
        return None;
    }
    let Ok(serde_json::Value::Object(current)) = serde_json::to_value(current) else {
        return None;
    };
    // Only keys that name a real field of this half; `current` IS that field list, derived not
    // written. A key that names neither half's field is simply not a style property.
    let incoming: Vec<(String, serde_json::Value)> = node
        .props
        .iter()
        .map(|(key, value)| (retired_name(key), value))
        .filter(|(key, _)| current.contains_key(key.as_str()))
        .filter_map(|(key, value)| prop_to_json(value, theme).map(|v| (key, v)))
        .collect();
    if incoming.is_empty() {
        return None;
    }

    let mut merged = current.clone();
    for (key, value) in &incoming {
        merged.insert(key.clone(), value.clone());
    }
    // Fast path: everything fits. Otherwise fall back to applying one key at a time so a single
    // bad value costs only itself.
    if let Ok(whole) = serde_json::from_value::<T>(serde_json::Value::Object(merged)) {
        return Some(whole);
    }
    let mut acc = current;
    for (key, value) in incoming {
        let mut candidate = acc.clone();
        candidate.insert(key.clone(), value);
        if serde_json::from_value::<T>(serde_json::Value::Object(candidate.clone())).is_ok() {
            acc = candidate;
        }
    }
    serde_json::from_value::<T>(serde_json::Value::Object(acc)).ok()
}

/// A [`PropValue`] as the JSON scalar its field expects — the enums travel as their
/// **names** (`"center"`, `"space_between"`, `"small"`), matching how glyphs and colours already
/// cross the boundary. Colours and glyphs never name a `Layout` field, so they are simply strings
/// here and get filtered out by the field-name check.
pub(crate) fn prop_to_json(value: &PropValue, theme: &Theme) -> Option<serde_json::Value> {
    Some(match value {
        PropValue::Bool(b) => serde_json::Value::Bool(*b),
        PropValue::Int(i) => serde_json::Value::from(*i),
        PropValue::Float(f) => serde_json::Number::from_f64(*f).map(Into::into)?,
        // A colour is a hex literal or a THEME TOKEN NAME. A token is resolved here, against the
        // theme this tree is being built with — the same moment every native widget bakes its
        // colours in. A theme reload drops the retained trees and rebuilds them (`reload_config`
        // clears `chrome_tree` and every pane header), so a token-named override follows the new
        // theme for free, without the colour having to stay unresolved all the way to paint.
        //
        // A hex literal is exactly what it says and does not track the theme. That is the trade a
        // caller makes by writing one, and it is the reason the docs steer toward token names.
        PropValue::Color(c) => serde_json::Value::String(resolve_color(c, theme)?),
        PropValue::Text(t) | PropValue::Glyph(t) => serde_json::Value::String(t.clone()),
        PropValue::Size(s) => serde_json::to_value(s).ok()?,
        PropValue::Variant(v) => serde_json::to_value(v).ok()?,
        PropValue::Align(a) => serde_json::to_value(a).ok()?,
        PropValue::List(items) => serde_json::Value::Array(
            items
                .iter()
                .filter_map(|i| prop_to_json(i, theme))
                .collect(),
        ),
        // A struct-shaped property (`border`, `glow`) — handed to the field's own deserializer as
        // an object. Recursive, so a colour nested inside is a theme token like any other and is
        // resolved here against the same theme: without that a nested `"accent"` would reach serde
        // as the literal word and the whole field would be dropped. A member that cannot be
        // converted is skipped, leaving the rest — the same totality rule as everywhere else, and
        // it means a half-written object degrades to its usable fields rather than vanishing.
        PropValue::Map(fields) => serde_json::Value::Object(
            fields
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), prop_to_json(v, theme)?)))
                .collect(),
        ),
    })
}

/// A colour property as the hex string `Color`'s own deserializer accepts.
///
/// `#rrggbb` / `#rrggbbaa` / `#rgb` pass straight through — `Color::from_str` owns those spellings
/// and this does not second-guess it. Anything else is read as a **theme token name** and looked up
/// among the theme's own colour fields.
///
/// The lookup is derived, not written: `heca_theme::Theme` is `Serialize` and its colours
/// serialize as hex strings, so the accepted vocabulary *is* the theme's field list. Add a colour
/// token to the theme and a description can name it the same day, with no table here to update —
/// the same arrangement `merge_style_half` uses for the style fields themselves.
///
/// `None` for a name the theme does not have, which drops that one property and leaves its
/// neighbours alone (`merge_style_half` is total for untrusted input).
pub(crate) fn resolve_color(spec: &str, theme: &Theme) -> Option<String> {
    if spec.starts_with('#') {
        return Some(spec.to_string());
    }
    let serde_json::Value::Object(tokens) = serde_json::to_value(&theme.colors).ok()? else {
        return None;
    };
    match tokens.get(spec) {
        // A colour token. Non-colour fields (`name`, the numeric geometry) either are not strings
        // or do not parse as a colour, so they cannot be named by accident.
        Some(serde_json::Value::String(hex)) if hex.starts_with('#') => Some(hex.clone()),
        _ => None,
    }
}

/// Feed a node's properties to a widget through its **generated** surface.
///
/// This is the whole point of the arrangement: the app names no property here. Which keys a widget
/// accepts is decided by that widget's own `#[prop]` builders, so a capability added in the library
/// is reachable from a description the same day, and one that is forgotten fails the drift guard
/// rather than going quietly missing.
///
/// Call it **after** children are attached — properties are order-independent on that condition,
/// which is what lets a builder that clamps against its children (`Select::selected`) see them.
pub(crate) fn with_props<W: SetProp>(widget: W, node: &ViewNode, theme: &Theme) -> W {
    widget.apply_props(|key| node.props.get(key).and_then(|v| prop_to_input(v, theme)))
}

/// A [`PropValue`] as the library's neutral scalar. The library never sees the app's model; this
/// is the one conversion at the boundary.
///
/// Enums cross as their **names**, which is how glyphs and colours already travel, so the widget's
/// own variants are the accepted vocabulary and there is no table of strings on either side.
pub(crate) fn prop_to_input(value: &PropValue, theme: &Theme) -> Option<PropInput> {
    Some(match value {
        PropValue::Bool(b) => PropInput::Bool(*b),
        PropValue::Int(i) => PropInput::Number(*i as f64),
        PropValue::Float(f) => PropInput::Number(*f),
        // A colour token is resolved to hex HERE, on the host side, before it reaches the widget:
        // the library has no notion of a theme token, and `Color::from_str` only knows hex.
        PropValue::Color(c) => PropInput::Text(resolve_color(c, theme)?),
        PropValue::Text(t) | PropValue::Glyph(t) => PropInput::Text(t.clone()),
        PropValue::Size(s) => PropInput::Text(prop_enum_name(s)?),
        PropValue::Variant(v) => PropInput::Text(prop_enum_name(v)?),
        PropValue::Align(a) => PropInput::Text(prop_enum_name(a)?),
        // Neither shape fits a widget builder: `PropInput` is a scalar channel (Bool | Number |
        // Text) because no `#[prop]` builder takes a list or a struct. A `Map` is for the
        // struct-shaped fields of `Layout`/`Visual`, which reach the widget through
        // `merge_style_half` and serde instead — see `prop_to_json`. If a widget builder ever does
        // take one, `PropInput` grows a variant then, deliberately, and not before.
        PropValue::List(_) | PropValue::Map(_) => return None,
    })
}

/// The snake_case name serde already gives these enums — reused rather than re-spelled, so the two
/// paths (the layout merge and the widget surface) cannot disagree about what `"space_between"` is.
pub(crate) fn prop_enum_name<T: serde::Serialize>(value: &T) -> Option<String> {
    match serde_json::to_value(value).ok()? {
        serde_json::Value::String(s) => Some(s),
        _ => None,
    }
}
