//! **A built-in action as the catalog names it, with the arguments it carries** — what a request
//! for it says when all that arrived was the [`WmAction`] a window sent.

use serde_json::Value;

use crate::chrome::{PropMap, PropValue};
use crate::input::WmAction;

/// The name `action` has in the catalog and the arguments it carries, as the same kind of data a
/// script's line would have given.
pub(crate) fn describe(action: &WmAction) -> (String, PropMap) {
    let (variant, fields) = match serde_json::to_value(action) {
        Ok(Value::String(variant)) => (variant, None),
        Ok(Value::Object(map)) if map.len() == 1 => match map.into_iter().next() {
            Some((variant, fields)) => (variant, Some(fields)),
            None => (String::new(), None),
        },
        _ => (String::new(), None),
    };
    let name = super::builtin_name(action.kind()).map_or(variant, str::to_owned);
    let args = match fields {
        Some(Value::Object(fields)) => fields.into_iter().filter_map(|(key, value)| Some((key, prop(value)?))).collect(),
        Some(other) => prop(other).map(|value| ("value".to_owned(), value)).into_iter().collect(),
        None => PropMap::new(),
    };
    (name, args)
}

fn prop(value: Value) -> Option<PropValue> {
    Some(match value {
        Value::Null => return None,
        Value::Bool(flag) => PropValue::Bool(flag),
        Value::Number(number) => number.as_i64().map_or_else(|| PropValue::Float(number.as_f64().unwrap_or_default()), PropValue::Int),
        Value::String(text) => PropValue::Text(text),
        Value::Array(items) => PropValue::List(items.into_iter().filter_map(prop).collect()),
        Value::Object(fields) => PropValue::Map(fields.into_iter().filter_map(|(key, value)| Some((key, prop(value)?))).collect()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unit_action_has_its_catalog_name_and_no_arguments() {
        let (name, args) = describe(&WmAction::SplitHorizontal);
        assert_eq!(name, "split_horizontal");
        assert!(args.is_empty());
    }

    #[test]
    fn a_parameterized_action_carries_its_fields_as_arguments() {
        let (name, args) = describe(&WmAction::FocusPane { pane_id: heca_core::layout::PaneId(7) });
        assert_eq!(name, crate::actions::builtin_name(WmAction::FocusPane { pane_id: heca_core::layout::PaneId(7) }.kind()).unwrap());
        assert_eq!(args.get("pane_id"), Some(&PropValue::Int(7)));
    }
}
