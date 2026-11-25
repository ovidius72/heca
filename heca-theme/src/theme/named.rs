//! **A theme colour by its name** — what a widget writes when it wants a colour that follows the
//! theme (`Surface::new().background(Fill::theme("surface"))`) and what a description's token
//! (`"fill": "surface"`) resolves through.

use super::Theme;
use crate::Color;

impl Theme {
    /// **The colour this theme gives `name`**, the name being the colour's own field (`"surface"`,
    /// `"accent"`, `"border"`, …) — the **effective** colour: one the theme derives when it leaves
    /// it unset (`top_bottom_pane_background`, `selected_background`, …) answers with what it
    /// derives, the same value its `effective_*` method gives. `None` for a name that is not a
    /// colour of the theme, and for an optional colour with nothing to derive from.
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
            "selected_background" => Some(self.effective_selected_background()),
            "previous_background" => Some(self.effective_previous_background()),
            "workspace_active_background" => Some(self.effective_workspace_active_background()),
            "workspace_previous_background" => Some(self.effective_workspace_previous_background()),
            "card_previous_background" => Some(self.effective_card_previous_background()),
            "danger" => Some(self.danger),
            "success" => Some(self.success),
            "warning" => Some(self.warning),
            "left_sidebar_background" => Some(self.effective_left_sidebar_background()),
            "right_sidebar_background" => Some(self.effective_right_sidebar_background()),
            "top_bottom_pane_background" => Some(self.effective_top_bottom_pane_background()),
            "background_gradient_top" => Some(self.effective_background_gradient_top()),
            "background_gradient_bottom" => Some(self.effective_background_gradient_bottom()),
            "focus_ring" => Some(self.effective_focus_ring()),
            "hint_color" => Some(self.effective_hint_color()),
            "drag_edge_color" => self.drag_edge_color,
            "drag_edge_target_color" => self.drag_edge_target_color,
            "float_background" => Some(self.float_background),
            "pane_border" => Some(self.effective_pane_border()),
            "pane_active_border" => Some(self.effective_pane_active_border()),
            "float_pane_border" => Some(self.float_pane_border),
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

impl Theme {
    /// **The opacity this theme gives `name`** — one of its interaction alphas (`"scrim"`,
    /// `"row_hover_fill"`, …), what `Fill::theme(..).alpha(..)` reads when it draws. `None` for a
    /// name that is not one.
    pub fn alpha_named(&self, name: &str) -> Option<u8> {
        match name {
            "toggle_on_fill" => Some(self.interaction.toggle_on_fill),
            "control_hover_fill" => Some(self.interaction.control_hover_fill),
            "control_hover_border" => Some(self.interaction.control_hover_border),
            "control_active_fill" => Some(self.interaction.control_active_fill),
            "control_active_border" => Some(self.interaction.control_active_border),
            "control_rest_border" => Some(self.interaction.control_rest_border),
            "control_rest_glow" => Some(self.interaction.control_rest_glow),
            "row_hover_fill" => Some(self.interaction.row_hover_fill),
            "row_active_fill" => Some(self.interaction.row_active_fill),
            "row_active_border" => Some(self.interaction.row_active_border),
            "row_active_tint" => Some(self.interaction.row_active_tint),
            "row_hover_tint" => Some(self.interaction.row_hover_tint),
            "nav_outline" => Some(self.interaction.nav_outline),
            "nav_wash" => Some(self.interaction.nav_wash),
            "nav_fill" => Some(self.interaction.nav_fill),
            "badge_fill" => Some(self.interaction.badge_fill),
            "tag_fill" => Some(self.interaction.tag_fill),
            "tag_border" => Some(self.interaction.tag_border),
            "toast_tint" => Some(self.interaction.toast_tint),
            "outline_rest" => Some(self.interaction.outline_rest),
            "outline_hover" => Some(self.interaction.outline_hover),
            "selection" => Some(self.interaction.selection),
            "hilite" => Some(self.interaction.hilite),
            "scrim" => Some(self.interaction.scrim),
            "keycap" => Some(self.interaction.keycap),
            "panel_border" => Some(self.interaction.panel_border),
            "panel_row_border" => Some(self.interaction.panel_row_border),
            "panel_row_fill" => Some(self.interaction.panel_row_fill),
            "tooltip_border" => Some(self.interaction.tooltip_border),
            "menu_shortcut" => Some(self.interaction.menu_shortcut),
            "menu_shortcut_dim" => Some(self.interaction.menu_shortcut_dim),
            "thumb_rest" => Some(self.interaction.thumb_rest),
            "thumb_hover" => Some(self.interaction.thumb_hover),
            "unlit" => Some(self.interaction.unlit),
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

    /// **A colour the theme derives answers with what it derives** when the theme leaves it unset.
    #[test]
    fn an_unset_derived_colour_answers_with_its_effective_value() {
        let theme = Theme {
            top_bottom_pane_background: None,
            selected_background: None,
            ..Theme::default()
        };
        assert_eq!(
            theme.color_named("top_bottom_pane_background"),
            Some(theme.effective_top_bottom_pane_background())
        );
        assert_eq!(
            theme.color_named("selected_background"),
            Some(theme.effective_selected_background())
        );
    }

    /// **Every interaction alpha answers to its own name** — walked from the serialized theme, so an
    /// alpha added to the theme and not to [`Theme::alpha_named`] fails here.
    #[test]
    fn every_interaction_alpha_answers_to_its_own_name() {
        let theme = Theme::default();
        let toml::Value::Table(fields) =
            toml::Value::try_from(theme.interaction).expect("the alphas serialize")
        else {
            panic!("the alphas are a table");
        };
        let mut checked = 0;
        for (name, value) in fields {
            let Some(alpha) = value.as_integer() else {
                continue;
            };
            assert_eq!(theme.alpha_named(&name), Some(alpha as u8), "{name}");
            checked += 1;
        }
        assert!(checked > 20, "the alphas were walked ({checked})");
        assert_eq!(theme.alpha_named("chartreuse"), None);
    }
}
