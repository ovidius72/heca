//! **A theme colour by its name** — what a widget writes when it wants a colour that follows the
//! theme (`Surface::new().background(Fill::theme("surface"))`) and what a description's token
//! (`"fill": "surface"`) resolves through.

use super::Theme;
use crate::Color;

impl Theme {
    /// **The colour this theme gives `name`**, the name being the colour's own field (`"surface"`,
    /// `"accent"`, `"border"`, …). `None` for a name that is not a colour of the theme, and for an
    /// optional colour this theme leaves unset.
    ///
    /// Read at paint, so it is a plain match rather than a walk over the serialized theme; the test
    /// `every_colour_field_answers_to_its_own_name` fails the day a colour is added to the theme
    /// and not here.
    pub fn color_named(&self, name: &str) -> Option<Color> {
        match name {
            "background" => Some(self.background),
            "surface" => Some(self.surface),
            "foreground" => Some(self.foreground),
            "muted" => Some(self.muted),
            "border" => Some(self.border),
            "accent" => Some(self.accent),
            "glow" => Some(self.glow),
            "selected_background" => self.selected_background,
            "previous_background" => self.previous_background,
            "workspace_active_background" => self.workspace_active_background,
            "workspace_previous_background" => self.workspace_previous_background,
            "card_previous_background" => self.card_previous_background,
            "danger" => Some(self.danger),
            "success" => Some(self.success),
            "warning" => Some(self.warning),
            "left_sidebar_background" => self.left_sidebar_background,
            "right_sidebar_background" => self.right_sidebar_background,
            "top_bottom_pane_background" => self.top_bottom_pane_background,
            "background_gradient_top" => self.background_gradient_top,
            "background_gradient_bottom" => self.background_gradient_bottom,
            "focus_ring" => self.focus_ring,
            "hint_color" => self.hint_color,
            "drag_edge_color" => self.drag_edge_color,
            "drag_edge_target_color" => self.drag_edge_target_color,
            "float_background" => Some(self.float_background),
            "float_accent" => Some(self.float_accent),
            "float_focus" => Some(self.float_focus),
            "drag_ghost_bg" => Some(self.drag_ghost_bg),
            "drag_ghost_fg" => Some(self.drag_ghost_fg),
            "drag_source_bg" => Some(self.drag_source_bg),
            "drag_source_border" => Some(self.drag_source_border),
            "drop_target_bg" => Some(self.drop_target_bg),
            "drop_target_border" => Some(self.drop_target_border),
            "drop_insertion" => Some(self.drop_insertion),
            "terminal_foreground" => self.terminal_foreground,
            "terminal_background" => self.terminal_background,
            "terminal_cursor_foreground" => self.terminal_cursor_foreground,
            "terminal_cursor_background" => self.terminal_cursor_background,
            "terminal_cursor_border" => self.terminal_cursor_border,
            "terminal_selection_foreground" => self.terminal_selection_foreground,
            "terminal_selection_background" => self.terminal_selection_background,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every colour of the theme answers to its own field name**, with the colour the theme
    /// holds — found by walking the serialized theme, so a colour added to the theme and not to
    /// [`Theme::color_named`] fails here.
    #[test]
    fn every_colour_field_answers_to_its_own_name() {
        let theme = Theme {
            focus_ring: Some(Color::rgb(1, 2, 3)),
            ..Theme::default()
        };
        let toml::Value::Table(fields) = toml::Value::try_from(&theme).expect("a theme serializes")
        else {
            panic!("a theme is a table");
        };
        let mut checked = 0;
        for (name, value) in fields {
            let Some(colour) = value.as_str().and_then(|text| text.parse::<Color>().ok()) else {
                continue;
            };
            assert_eq!(theme.color_named(&name), Some(colour), "{name}");
            checked += 1;
        }
        assert!(checked > 20, "the theme's colours were walked ({checked})");
        assert_eq!(
            theme.color_named("name"),
            None,
            "the theme's name is not a colour"
        );
        assert_eq!(
            theme.color_named("chartreuse"),
            None,
            "not a colour of the theme"
        );
    }
}
