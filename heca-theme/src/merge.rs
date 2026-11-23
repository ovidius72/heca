//! **The one key-by-key merge** of TOML layers — a user's configuration over heca's defaults, a
//! theme over the theme it inherits. Written once here, the lowest crate that needs it, so every
//! layering means the same thing.

/// **`over` onto `base`**: tables merge per key (recursively); scalars and arrays are replaced by
/// `over`. Arrays are intentionally NOT concatenated, so a user `[[keys.command]]` /
/// `[[keys.mode]]` list replaces the default list wholesale.
pub fn deep_merge(base: toml::Value, over: toml::Value) -> toml::Value {
    match (base, over) {
        (toml::Value::Table(mut b), toml::Value::Table(o)) => {
            for (k, ov) in o {
                let nv = match b.remove(&k) {
                    Some(bv) => deep_merge(bv, ov),
                    None => ov,
                };
                b.insert(k, nv);
            }
            toml::Value::Table(b)
        }
        // Scalars and arrays: `over` wins (arrays are replaced, not merged).
        (_, over) => over,
    }
}
