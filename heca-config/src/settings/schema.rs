//! Declare each setting once; derive loading and generate file-backed defaults.
//!
//! Add a field to the declaration and its value to config.default.toml. An optional
//! field may stay absent from the file. No callback or field registration is needed.
//! Nested settings declare their table path once in `defaults: [...]`.

// Serde handles missing fields, aliases, duplicate keys and invalid explicit values.
// Default reads individual raw fields so it never deserializes itself recursively.
macro_rules! declare_settings {
    (
        defaults: $tables:tt;
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($(#[$field_meta:meta])* $field_vis:vis $field:ident: $ty:ty),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[serde(default)]
        $vis struct $name {
            $($(#[$field_meta])* $field_vis $field: $ty,)*
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $crate::settings::schema::declare_settings!(
                        @field $tables; $field
                    ),)*
                }
            }
        }
    };
    (@field [$($table:literal),* $(,)?]; $field:ident) => {
        $crate::settings::defaults::field(&[$($table,)* stringify!($field)])
    };
}

pub(super) use declare_settings;
