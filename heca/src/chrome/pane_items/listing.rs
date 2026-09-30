//! **How a user's list of names becomes the items that show** — written once, for every kind of pane
//! item (buttons, chips, and the row lines after them).
//!
//! The rules are the user's, and they are the same for each kind:
//!
//! 1. The list is the list — by name, in order. A name nothing provides is skipped and reported,
//!    with the nearest real one offered.
//! 2. **What another crate adds is a default.** While the list is still heca's own, what was added
//!    shows after it; once the user has written their own list, only their list counts.
//! 3. An added item the user's own list leaves out is said **once ever**, so it can still be found
//!    without being repeated on every start.
//!
//! A kind supplies its items and says what they are called; it writes none of the above.

use crate::chrome::warn_author;

/// An item a user can list by name.
pub(crate) trait Named {
    /// What config lists: `split`, `git_branch`, `pro.show_notes`.
    fn name(&self) -> &str;
    /// Added by another crate rather than shipped with heca.
    fn is_added(&self) -> bool;
}

impl<T: Named> Named for std::rc::Rc<T> {
    fn name(&self) -> &str {
        (**self).name()
    }
    fn is_added(&self) -> bool {
        (**self).is_added()
    }
}

/// **The items to show, in order** — rules 1 and 2. A name nothing provides is skipped here and
/// said by [`report`].
pub(crate) fn shown<'a, T: Named>(
    defs: &'a [T],
    config: &[String],
    default: &[String],
) -> Vec<&'a T> {
    let mut out: Vec<&T> = config
        .iter()
        .filter_map(|n| defs.iter().find(|d| d.name() == n))
        .collect();
    if config == default {
        out.extend(defs.iter().filter(|d| d.is_added()));
    }
    out
}

/// **Say what the user's list gets wrong or misses** — rules 1 and 3.
///
/// `what` names the kind ("pane button"), `key` the config key that lists it (`title_actions`).
pub(crate) fn report<T: Named>(
    what: &str,
    key: &str,
    defs: &[T],
    config: &[String],
    default: &[String],
) {
    for name in config
        .iter()
        .filter(|n| defs.iter().all(|d| d.name() != n.as_str()))
    {
        let near = crate::args::nearest_of(name, defs.iter().map(|d| d.name()));
        warn_author(format!(
            "[heca] {what} '{name}' in [appearance.pane] {key} is not provided by heca or by a \
             program built on it, so it is skipped{}",
            near.map(|n| format!(" — did you mean '{n}'?"))
                .unwrap_or_default()
        ));
    }
    if config != default {
        for def in defs.iter().filter(|d| d.is_added()) {
            let listed = config.iter().any(|n| n == def.name());
            if !listed && crate::announced::first_time(&format!("{what}:{}", def.name())) {
                warn_author(format!(
                    "[heca] {what} '{}' is available; add it to {key} in [appearance.pane] to \
                     show it",
                    def.name()
                ));
            }
        }
    }
}
