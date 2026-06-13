//! Embedded terminal font defaults.

/// Family name of the embedded terminal monospace font.
pub const DEFAULT_TERMINAL_FAMILY: &str = "Maple Mono Normal NF";

/// Embedded bytes of the default terminal font regular face.
pub const DEFAULT_TERMINAL_BYTES: &[u8] =
    include_bytes!("../assets/MapleMonoNormal-NF-Regular.ttf");

/// Embedded bytes of the default terminal font bold face.
pub const DEFAULT_TERMINAL_BOLD_BYTES: &[u8] =
    include_bytes!("../assets/MapleMonoNormal-NF-Bold.ttf");
