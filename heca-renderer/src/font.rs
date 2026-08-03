//! Embedded terminal font defaults.

/// Family name of the embedded terminal monospace font.
pub const DEFAULT_TERMINAL_FAMILY: &str = "Maple Mono Normal NF";

/// Family of the embedded **Nerd Font** — the face
/// [`FontRole::NerdFont`](heca_grid_ui::scene::FontRole::NerdFont) runs are shaped with
/// ([`NfIcon`](heca_grid_ui::widgets::NfIcon), keycaps).
///
/// It is the same file as the default terminal face, which is a real Nerd Font, so the app ships one
/// copy rather than two. Naming it separately is what keeps that an implementation detail: chrome
/// asks for "the Nerd Font", never for "the terminal font", and the two can diverge without a single
/// keycap changing. Crucially it is addressed **by family name**, and the embedded faces are loaded
/// unconditionally at renderer init — so a user configuring their own terminal font cannot take the
/// glyphs out from under the UI.
pub const NERD_FONT_FAMILY: &str = DEFAULT_TERMINAL_FAMILY;

/// Embedded bytes of the default terminal font regular face.
pub const DEFAULT_TERMINAL_BYTES: &[u8] =
    include_bytes!("../assets/MapleMonoNormal-NF-Regular.ttf");

/// Embedded bytes of the default terminal font bold face.
pub const DEFAULT_TERMINAL_BOLD_BYTES: &[u8] =
    include_bytes!("../assets/MapleMonoNormal-NF-Bold.ttf");
