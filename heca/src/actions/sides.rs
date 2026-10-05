//! **Which side each kind of action runs on**, built once from the built-in descriptors.
//!
//! The registry routes by [`WmActionKind`] — the same key it finds a handler by — so deciding where
//! an action runs is one lookup in a table, not a search of the descriptors by name for every
//! action that fires.

use std::collections::HashMap;

use super::{Side, builtins};
use crate::args::{ArgSpec, sample_args};
use crate::input::WmActionKind;

/// `kind → side`, for every built-in.
pub(crate) struct Sides(HashMap<WmActionKind, Side>);

impl Sides {
    /// Read the side of every built-in. Each descriptor is turned into its kind the way
    /// [`ActionCatalog::with_builtins`](super::ActionCatalog::with_builtins) turns it into a policy:
    /// from its name and its declared arguments. Two descriptors of one kind must agree, since the
    /// registry cannot route one handler two ways.
    pub(crate) fn from_builtins() -> Self {
        let mut table: HashMap<WmActionKind, Side> = HashMap::new();
        for d in builtins() {
            let args: Vec<ArgSpec> = d.args.iter().map(ArgSpec::from_descriptor).collect();
            let kind = crate::input::resolve_action(d.name, &sample_args(&args))
                .unwrap_or_else(|| {
                    unreachable!("built-in action {:?} builds from neither name nor arguments", d.name)
                })
                .kind();
            if let Some(earlier) = table.insert(kind, d.side) {
                assert_eq!(
                    earlier, d.side,
                    "built-in {:?}: another action of the same kind is on the other side",
                    d.name
                );
            }
        }
        Self(table)
    }

    /// The side of `kind`. `None` for a kind no built-in descriptor names.
    pub(crate) fn of(&self, kind: WmActionKind) -> Option<Side> {
        self.0.get(&kind).copied()
    }
}
