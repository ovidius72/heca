//! An action from its name and its arguments as text — the one constructor a binding, a menu
//! entry, a plugin and RPC all reach.

use super::WmAction;

mod chrome;
mod layout;
mod naming;
mod terminal;
#[cfg(test)]
mod tests;

// ── Parameterized action builders ──

pub(super) fn get_u64(args: &std::collections::HashMap<String, String>, key: &str) -> Option<u64> {
    args.get(key)?.parse().ok()
}
pub(super) fn get_usize(args: &std::collections::HashMap<String, String>, key: &str) -> Option<usize> {
    args.get(key)?.parse().ok()
}
pub(super) fn get_f64(args: &std::collections::HashMap<String, String>, key: &str) -> Option<f64> {
    args.get(key)?.parse().ok()
}
pub(super) fn get_string(args: &std::collections::HashMap<String, String>, key: &str) -> Option<String> {
    args.get(key).cloned()
}
pub(super) fn get_enum<T: std::str::FromStr>(
    args: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<T> {
    args.get(key)?.parse().ok()
}
/// An **optional** vocabulary argument: absent means the default, and a value that does not parse
/// is a mistake worth failing on rather than silently becoming the default.
pub(super) fn get_enum_or_default<T: std::str::FromStr + Default>(
    args: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<T> {
    match args.get(key) {
        Some(raw) => raw.parse().ok(),
        None => Some(T::default()),
    }
}

/// **The one door from a name and its arguments to a [`WmAction`]** — what a `config.toml` binding, a
/// menu entry's `Intent`, the palette, a plugin, RPC and the catalog all call.
///
/// The arguments are read first ([`build_action`]); a name that says everything on its own falls
/// back to [`action_from_name`](super::action_from_name). `None` when neither builds it — an unknown
/// name, or a required argument missing or malformed. The two builders are private to `input` so no
/// caller can combine them in its own order again.
pub fn resolve_action(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    build_action(name, args).or_else(|| super::action_from_name(name))
}

/// Build a `WmAction` from a name and its arguments as text — the constructor a `config.toml`
/// binding, a menu entry's `Intent`, a plugin and RPC all reach.
///
/// `None` when the name is unknown, or when a required argument is missing or does not parse.
///
/// **Each arm's arguments are declared** in that action's
/// [`ActionDescriptor::args`](crate::actions::ActionDescriptor::args), which is what lets a caller
/// discover them (`describe-action`) and what lets
/// [`check_args`](crate::args::check_args) say *which* argument was wrong instead of the whole
/// call quietly evaporating. The list that used to sit here in a doc comment named eleven of the
/// thirty-five and had not been updated in a long time; the declarations replaced it, and
/// `every_declared_argument_is_read_by_the_action` keeps them and these arms in step.
pub(super) fn build_action(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    layout::build_layout(name, args)
        .or_else(|| naming::build_naming(name, args))
        .or_else(|| terminal::build_terminal(name, args))
        .or_else(|| chrome::build_chrome(name, args))
}
