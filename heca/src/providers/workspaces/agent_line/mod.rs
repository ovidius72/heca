//! Driver-supplied identity and reported work status, following one pane's facts.

use heca_grid_ui::image::ImageSource;
use heca_grid_ui::prop::PropName;
use heca_grid_ui::reactive::{SignalGet, create_effect, signal};
use heca_grid_ui::style::{FontWeight, Spacing, WidgetSize};
use heca_grid_ui::widgets::{Flex, Glyph, Grid, Icon, Label, Visibility};
use heca_grid_ui::{
    Image, Tone,
    builders::{LayoutExt, Parent, PlaceExt, PresenceExt, ToneExt, TooltipExt},
};

use super::pane_row::{META_INDENT, set_if_changed};
use crate::agent::AgentIcon;
use crate::chrome::panes::pane_items::LineCx;
use crate::components::PaneName;
use heca_config::appearance::AgentSessionLine;

pub(super) fn build(cx: &LineCx) -> Visibility {
    let initial = (cx.facts)();
    let primary = text(FontWeight::Bold);
    let primary_text = primary.text_signal();
    let session = text(FontWeight::Regular);
    let session_text = session.text_signal();
    let session = Visibility::new().visible(false).child(session);
    let session_visible = session.visible_signal();
    // The status is its glyph alone; its words are the glyph's tooltip.
    let status_text = signal(String::new());
    let task = text(FontWeight::Regular).muted(true);
    let task_text = task.text_signal();
    let task = Visibility::new().visible(false).child(task);
    let task_visible = task.visible_signal();

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
    let indicator = Icon::new(Glyph::Circle);
    let status_glyph = indicator.glyph_signal();
    let status_tone = signal(None);
    let tooltip = signal(String::new());
    let row = Visibility::new().visible(false).child(
        Flex::column()
            .gap(Spacing::Xs)
            .padding_x(META_INDENT)
            .child(
                Grid::new()
                    .template_column("auto 1fr auto")
                    .template_area(["logo primary status", ". session session"])
                    .align("center")
                    .gap(Spacing::Sm)
                    .child(Flex::row().child(logo).child(generic).area("logo"))
                    .child(primary.area("primary"))
                    .child(session.area("session"))
                    .child(
                        Flex::row()
                            .align("center")
                            .content_tone_signal(status_tone)
                            .child(indicator.tooltip_signal(status_text))
                            .area("status"),
                    ),
            )
            .child(task)
            .tooltip_signal(tooltip),
    );
    let visible = row.visible_signal();
    let facts = cx.facts.clone();
    let statuses = cx.agent_statuses;
    let session_line = cx.agent_session_line;
    create_effect(move |_| {
        let facts = facts();
        set_if_changed(visible, facts.agent.is_some());
        let Some(agent) = facts.agent else { return };
        let statuses = statuses.get();
        let presentation = statuses.get(agent.status.as_str());
        let label = presentation
            .and_then(|p| p.label.as_deref())
            .unwrap_or(agent.status.as_str());
        let glyph = presentation
            .and_then(|p| p.icon.as_deref())
            .and_then(Glyph::from_prop_name)
            .unwrap_or(Glyph::Circle);
        set_if_changed(status_glyph, glyph);
        set_if_changed(
            status_tone,
            presentation
                .and_then(|p| p.tone.as_deref())
                .and_then(Tone::from_prop_name),
        );
        // **The model the agent answers with, else its name.** Every built-in agent now keeps its
        // model current: Codex names it on each event, and Claude's is read from its conversation
        // log at each event (`reporting::command::last_model`). One that names none shows its name.
        let primary = agent
            .model
            .filter(|model| !model.is_empty())
            .unwrap_or(agent.name);
        let reported = agent.session_title.filter(|text| !text.is_empty());
        let session = match session_line.get() {
            AgentSessionLine::Reported => reported.filter(|title| *title != facts.title),
            AgentSessionLine::Always => Some(reported.unwrap_or(facts.title)),
            AgentSessionLine::Never => None,
        }
        .unwrap_or_default();
        set_if_changed(session_visible, !session.is_empty() && session != primary);
        set_if_changed(primary_text, primary);
        set_if_changed(session_text, session);
        set_if_changed(status_text, label.to_owned());
        set_if_changed(
            task_visible,
            agent.task.as_ref().is_some_and(|task| !task.is_empty()),
        );
        set_if_changed(task_text, agent.task.unwrap_or_default());
        set_if_changed(tooltip, agent.detail.unwrap_or_default());
    });
    row
}

fn text(weight: FontWeight) -> Label {
    PaneName::new("")
        .weight(weight)
        .font_scale(WidgetSize::Normal.font_scale())
        .build()
        .widget
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
