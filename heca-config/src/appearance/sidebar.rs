//! **The sidebar's look** — `[appearance.sidebar]`.

use super::*;

/// Sidebar shell appearance (`[appearance.sidebar]`). Border fields unset →
/// global `[appearance]` default → theme. (Surface → global → theme.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SidebarAppearance {
    /// Frame style. `None` → [`BorderStyle::Bracketed`]. `none | bordered | bracketed`.
    #[serde(default)]
    pub border_style: Option<BorderStyle>,
    /// Border width (logical px). `None` → global `border_width` → theme. Clamped `[0, 10]`.
    #[serde(default)]
    pub border_width: Option<f32>,
    /// Frame color, for every `border_style`: the line of `bordered`, the brackets of
    /// `bracketed`. `None` → `bordered` takes global `border_color` → `theme.border`; `bracketed`
    /// takes the theme accent.
    #[serde(default)]
    pub border_color: Option<Color>,
    /// Corner radius (logical px). `None` → global `border_radius` → theme. Clamped `[0, 40]`.
    #[serde(default)]
    pub border_radius: Option<f32>,
    /// Shell background fill. `None` → the theme-derived sidebar surface color. The
    /// window/chrome transparency still applies on top.
    #[serde(default)]
    pub background_color: Option<Color>,
    /// Gap between the sidebar and the content area (logical px). `None` → 12.0.
    #[serde(default)]
    pub gap: Option<f32>,
    /// Sidebar width (logical px). `None` → 300; clamped to `[MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH]`.
    #[serde(default)]
    pub width: Option<f32>,
    /// **The view the sidebar starts in** — which of the three lists below its pane cards show.
    /// The view chosen with the `sidebar_pane_view` action is remembered and wins at the next start;
    /// changing this in the file and reloading switches to it.
    #[serde(default)]
    pub pane_view: PaneView,
    /// The lines of a pane card in each view, by **name**, in order: `"title"` (the pip, the
    /// program's icon and the pane's name), `"cwd"`, `"git"`, `"agent"`, `"agent_session"`, or a
    /// line a program built on heca adds. A name nothing provides is reported and skipped.
    #[serde(default = "default_compact_view")]
    pub pane_compact_view: Vec<String>,
    #[serde(default = "default_normal_view")]
    pub pane_normal_view: Vec<String>,
    #[serde(default = "default_extended_view")]
    pub pane_extended_view: Vec<String>,
    /// The lines of the **current** pane's card, whatever the view — what sets it apart from the
    /// others. Unset, the current pane follows the view like every other.
    #[serde(default)]
    pub pane_current_view: Option<Vec<String>>,
    /// The line under a pane's agent: the session title the agent reported. See
    /// [`AgentSessionLine`].
    #[serde(default)]
    pub agent_session_line: AgentSessionLine,
    /// Show each workspace's number before its name (1–9, then 0 for the tenth). On by default.
    #[serde(default = "shown")]
    pub workspace_numbers: bool,
}

/// **When the line under an agent shows its session title** — `[appearance.sidebar]
/// agent_session_line`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentSessionLine {
    /// Only a title the agent reported, and only when it is not the pane's name already.
    #[default]
    Reported,
    /// The reported title, or the pane's name when the agent reported none.
    Always,
    /// Never.
    Never,
}

impl Default for SidebarAppearance {
    fn default() -> Self {
        Self {
            border_style: None,
            border_width: None,
            border_color: None,
            border_radius: None,
            background_color: None,
            gap: None,
            width: None,
            pane_view: PaneView::default(),
            pane_compact_view: default_compact_view(),
            pane_normal_view: default_normal_view(),
            pane_extended_view: default_extended_view(),
            pane_current_view: None,
            agent_session_line: AgentSessionLine::default(),
            workspace_numbers: true,
        }
    }
}

/// **Which list of lines a sidebar's pane cards show** — `[appearance.sidebar] pane_view`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaneView {
    /// `pane_compact_view`.
    Compact,
    /// `pane_normal_view`.
    Normal,
    /// `pane_extended_view`.
    #[default]
    Extended,
}

impl PaneView {
    /// Every view, in the order `sidebar_pane_view` cycles through them.
    pub const ALL: [PaneView; 3] = [PaneView::Compact, PaneView::Normal, PaneView::Extended];

    /// The view after this one, back to the first after the last.
    pub fn next(self) -> PaneView {
        let at = Self::ALL.iter().position(|view| *view == self).unwrap_or(0);
        Self::ALL[(at + 1) % Self::ALL.len()]
    }

    /// Its name, as config and the action write it.
    pub fn name(self) -> &'static str {
        match self {
            PaneView::Compact => "compact",
            PaneView::Normal => "normal",
            PaneView::Extended => "extended",
        }
    }

    /// The view called `name`, if there is one.
    pub fn from_name(name: &str) -> Option<PaneView> {
        Self::ALL.into_iter().find(|view| view.name() == name)
    }
}

impl std::str::FromStr for PaneView {
    type Err = String;
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        PaneView::from_name(name)
            .ok_or_else(|| format!("no view called '{name}' — compact, normal or extended"))
    }
}

impl SidebarAppearance {
    /// The lines of a pane card in `view`.
    pub fn view_lines(&self, view: PaneView) -> &[String] {
        match view {
            PaneView::Compact => &self.pane_compact_view,
            PaneView::Normal => &self.pane_normal_view,
            PaneView::Extended => &self.pane_extended_view,
        }
    }
}

/// **A view's line list from the embedded configuration**, until the user supplies their own — the
/// list `[appearance.sidebar] pane_<view>_view` is compared with, to tell "the user wrote their own"
/// from "still heca's" (what other crates add shows only on heca's).
///
/// A line is **named**, not typed: which names exist is answered in the app, where a program built
/// on heca can add its own (`demo.status`).
pub fn default_view_lines(view: PaneView) -> Vec<String> {
    static LINES: std::sync::OnceLock<[Vec<String>; 3]> = std::sync::OnceLock::new();
    let lines = LINES.get_or_init(|| {
        let defaults: toml::Value = toml::from_str(crate::loader::CONFIG_DEFAULT)
            .expect("embedded config.default.toml must be valid TOML");
        PaneView::ALL.map(|view| {
            let key = format!("pane_{}_view", view.name());
            defaults["appearance"]["sidebar"][key.as_str()]
                .clone()
                .try_into()
                .expect("embedded appearance.sidebar.pane_<view>_view must be a list of names")
        })
    });
    let at = PaneView::ALL.iter().position(|v| *v == view).unwrap_or(0);
    lines[at].clone()
}

fn default_compact_view() -> Vec<String> {
    default_view_lines(PaneView::Compact)
}

fn default_normal_view() -> Vec<String> {
    default_view_lines(PaneView::Normal)
}

fn default_extended_view() -> Vec<String> {
    default_view_lines(PaneView::Extended)
}

fn shown() -> bool {
    true
}

#[cfg(test)]
mod pane_view_tests {
    use super::PaneView;

    /// **The views cycle in order and come back round**, and each is found by its name.
    #[test]
    fn the_views_cycle_and_are_found_by_name() {
        assert_eq!(PaneView::Compact.next(), PaneView::Normal);
        assert_eq!(PaneView::Normal.next(), PaneView::Extended);
        assert_eq!(
            PaneView::Extended.next(),
            PaneView::Compact,
            "back to the first"
        );
        for view in PaneView::ALL {
            assert_eq!(PaneView::from_name(view.name()), Some(view));
        }
        assert_eq!(PaneView::from_name("tiny"), None);
    }
}
