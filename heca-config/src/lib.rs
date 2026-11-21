pub mod appearance;
pub mod color;
pub mod confirm;
pub mod docks;
pub mod extension_defaults;
pub mod font;
pub mod keys;
pub mod loader;
pub mod programs;
pub mod settings;
pub mod sound;
pub mod state_file;
pub mod theme;
pub mod trust;

pub use extension_defaults::add_extension_defaults;
pub use loader::ConfigError;
