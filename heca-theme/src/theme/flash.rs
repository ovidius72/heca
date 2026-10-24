use serde::{Deserialize, Serialize};

/// Semantic light pulses, shared by every widget that binds a flash handle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FlashStyle {
    /// Disable visual flashes without changing the events that produce them.
    pub enabled: bool,
    /// One fade, in seconds. Non-positive or non-finite values disable the effect.
    pub duration_s: f32,
    /// Dark interval between pulses, in seconds.
    pub pause_s: f32,
    /// Number of pulses in a finite burst. Zero is treated as one.
    pub pulses: u32,
    /// Maximum redraw frequency during a lit fade. Dark pauses sleep until the next pulse.
    pub frames_per_second: u16,
    /// Border width as a share of the target's resolved font size.
    pub border_width_em: f32,
    /// Peak opacity of the semantic border colour.
    pub border_alpha: u8,
    /// Peak opacity of the soft light inside the target.
    pub fill_alpha: u8,
    /// Glow radius as a share of the target's resolved font size.
    pub glow_radius_em: f32,
    /// Glow intensity, also scaled by the theme's glow settings.
    pub glow_strength: f32,
}

impl Default for FlashStyle {
    fn default() -> Self {
        Self { enabled: true, duration_s: 0.65, pause_s: 0.45, pulses: 3, frames_per_second: 30,
            border_width_em: 0.12, border_alpha: 235, fill_alpha: 18,
            glow_radius_em: 0.65, glow_strength: 0.45 }
    }
}
