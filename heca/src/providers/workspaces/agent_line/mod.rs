//! Driver-supplied identity and reported work status, following one pane's facts.

use heca_grid_ui::image::ImageSource;
use heca_grid_ui::prop::PropName;
use heca_grid_ui::reactive::{SignalGet, create_memo};
use heca_grid_ui::style::{FontWeight, Spacing, WidgetSize};
use heca_grid_ui::widgets::{Flex, Glyph, Grid, Icon, Label, Visibility};
use heca_grid_ui::{
    Image, Tone,
    builders::{LayoutExt, Parent, PlaceExt, PresenceExt, ToneExt, TooltipExt},
};

use super::pane_row::META_INDENT;
use crate::agent::AgentIcon;
use crate::chrome::panes::pane_items::LineCx;
use crate::components::PaneName;
use heca_config::appearance::AgentSessionLine;

pub(super) fn build(cx: &LineCx) -> Visibility {
    let initial = (cx.facts)();
    // **Every part follows the pane's facts by itself, through its builders** — what each one shows
    // is worked out once below, and nothing writes into a part.
    let facts = cx.facts.clone();
    let agent = create_memo(move |_| facts().agent);
    let statuses = cx.agent_statuses;
    let status = create_memo(move |_| {
        agent
            .get()
            .map(|agent| status_view(&agent, &statuses.get()))
    });
    let primary = text(FontWeight::Bold).text(create_memo(move |_| {
        agent.get().map(|a| identity(&a)).unwrap_or_default()
    }));
    // The status is its glyph alone; its words are the glyph's tooltip.
    let status_text = create_memo(move |_| status.get().map(|s| s.label).unwrap_or_default());
    let task_text = create_memo(move |_| agent.get().and_then(|a| a.task).unwrap_or_default());
    let task = Visibility::new()
        .visible(move || !task_text.get().is_empty())
        .child(text(FontWeight::Regular).muted(true).text(task_text));
    let tooltip = create_memo(move |_| agent.get().and_then(|a| a.detail).unwrap_or_default());

    // Image owns decoding, load errors and fitting. The row only supplies driver metadata — and
    // every part follows the pane's facts by itself, through its builders: the logo the agent sends
    // (or the program's glyph while there is none, or when it cannot be drawn), as tall as the text.
    let facts = cx.facts.clone();
    let logo_of = move || facts().agent.and_then(|agent| agent.icon);
    let (has_logo, no_logo) = (logo_of.clone(), logo_of.clone());
    let facts = cx.facts.clone();
    let program_glyph = move || facts().icon;
    let logo = Image::from_bytes(&[][..])
        .source_from(move || {
            logo_of()
                .map(image_source)
                .unwrap_or_else(|| ImageSource::bytes(Vec::new()))
        })
        .width("1em")
        .height("1em")
        .fallback(Icon::new(initial.icon).glyph(program_glyph.clone()))
        .hidden(move || no_logo().is_none());
    let generic = Icon::new(initial.icon)
        .glyph(program_glyph)
        .hidden(move || has_logo().is_some());
    let indicator =
        Icon::new(Glyph::Circle).glyph(move || status.get().map_or(Glyph::Circle, |s| s.glyph));
    Visibility::new()
        .visible(move || agent.get().is_some())
        .child(
            Flex::column()
                .gap(Spacing::Xs)
                .padding_x(META_INDENT)
                .child(
                    Grid::new()
                        .template_column("auto 1fr auto")
                        .template_area(["logo primary status"])
                        .align("center")
                        // The logo sits close to the model it names.
                        .gap(Spacing::Xs)
                        .child(Flex::row().child(logo).child(generic).area("logo"))
                        .child(primary.area("primary"))
                        .child(
                            Flex::row()
                                .align("center")
                                .content_tone(move || status.get().and_then(|s| s.tone))
                                .child(indicator.tooltip(status_text))
                                .area("status"),
                        ),
                )
                .child(task)
                .tooltip(tooltip),
        )
}

/// **How the agent's status reads**: the words, the glyph and the tone the user's
/// `[agents.statuses]` give it, else the status's own name on a plain circle.
#[derive(Clone, PartialEq)]
struct StatusView {
    label: String,
    glyph: Glyph,
    tone: Option<Tone>,
}

fn status_view(
    agent: &crate::agent::AgentState,
    statuses: &heca_config::settings::AgentStatuses,
) -> StatusView {
    let presentation = statuses.get(agent.status.as_str());
    StatusView {
        label: presentation
            .and_then(|p| p.label.clone())
            .unwrap_or_else(|| agent.status.as_str().to_owned()),
        glyph: presentation
            .and_then(|p| p.icon.as_deref())
            .and_then(Glyph::from_prop_name)
            .unwrap_or(Glyph::Circle),
        tone: presentation
            .and_then(|p| p.tone.as_deref())
            .and_then(Tone::from_prop_name),
    }
}

/// **The line under an agent with its session title** — `agent_session` in a sidebar view's list
/// (`[appearance.sidebar] pane_<view>_view`). A clock turning back and the title, aligned with the pane's other lines; what it
/// says follows `agent_session_line`, and it hides when there is nothing to say.
pub(super) fn session_line(cx: &LineCx) -> Visibility {
    let facts = cx.facts.clone();
    let setting = cx.agent_session_line;
    let title = create_memo(move |_| {
        let facts = facts();
        facts
            .agent
            .as_ref()
            .and_then(|agent| session_title(agent, &facts.title, setting.get()))
            .unwrap_or_default()
    });
    Visibility::new()
        .visible(move || !title.get().is_empty())
        .child(
            Flex::row()
                .align("center")
                .gap(Spacing::Sm)
                .padding_x(META_INDENT)
                .child(Icon::new(Glyph::ClockCounterClockwise).shrink(0.0))
                .child(text(FontWeight::Regular).text(title)),
        )
}

/// **The model the agent answers with, else its name.** Every built-in agent keeps its model
/// current: Codex names it on each event, and Claude's is read from its conversation log at each
/// event (`reporting::command::last_model`). One that names none shows its name.
fn identity(agent: &crate::agent::AgentState) -> String {
    agent
        .model
        .clone()
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| agent.name.clone())
}

/// What the session line says for `agent` in a pane named `pane_title`, as `setting` asks — never
/// the pane's name repeated by `reported`, never the line above it said again.
fn session_title(
    agent: &crate::agent::AgentState,
    pane_title: &str,
    setting: AgentSessionLine,
) -> Option<String> {
    let reported = agent.session_title.clone().filter(|text| !text.is_empty());
    match setting {
        AgentSessionLine::Reported => reported.filter(|title| title != pane_title),
        AgentSessionLine::Always => Some(reported.unwrap_or_else(|| pane_title.to_owned())),
        AgentSessionLine::Never => None,
    }
    .filter(|title| *title != identity(agent))
}

fn text(weight: FontWeight) -> Label {
    PaneName::new("")
        .weight(weight)
        .font_scale(WidgetSize::Normal.font_scale())
        .build()
}

#[cfg(test)]
mod tests;

/// What the image widget loads for an agent's icon.
fn image_source(icon: AgentIcon) -> ImageSource {
    match icon {
        AgentIcon::File(path) => ImageSource::file(path),
        AgentIcon::Svg(svg) => ImageSource::bytes(svg.into_bytes()),
    }
}
