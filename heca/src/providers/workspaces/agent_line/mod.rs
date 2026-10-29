//! Driver-supplied identity and reported work status, following one pane's facts.

use heca_grid_ui::{Image, Tone, builders::{LayoutExt, Parent, PlaceExt, ToneExt, TooltipExt}};
use heca_grid_ui::image::ImageSource;
use heca_grid_ui::prop::PropName;
use heca_grid_ui::reactive::{create_effect, signal, SignalGet};
use heca_grid_ui::style::{Length, Spacing, WidgetSize};
use heca_grid_ui::widgets::{Flex, Glyph, Grid, Icon, Label, Visibility};

use crate::agent::AgentIcon;
use crate::chrome::panes::pane_items::LineCx;
use crate::components::PaneName;
use super::pane_row::{META_INDENT, set_if_changed};

pub(super) fn build(cx: &LineCx<'_>) -> Visibility {
    let initial = (cx.facts)();
    let primary = text(cx, true);
    let primary_text = primary.text_signal();
    let session = text(cx, false);
    let session_text = session.text_signal();
    let session = Visibility::new(session, false);
    let session_visible = session.visible_signal();
    let status = text(cx, false);
    let status_text = status.text_signal();
    let task = text(cx, false).muted(true);
    let task_text = task.text_signal();
    let task = Visibility::new(task, false);
    let task_visible = task.visible_signal();

    // Image owns decoding, load errors and fitting. The row only supplies driver metadata.
    let fallback = Icon::new(initial.icon);
    let fallback_glyph = fallback.glyph_signal();
    let logo = Image::from_bytes(&[][..])
        .width(cx.theme.font_size).height(cx.theme.font_size)
        .fallback(fallback);
    let logo_source = logo.source_signal();
    let logo = Visibility::new(logo, false);
    let logo_visible = logo.visible_signal();
    let generic = Icon::new(initial.icon);
    let generic_glyph = generic.glyph_signal();
    let generic = Visibility::new(generic, true);
    let generic_visible = generic.visible_signal();
    let indicator = Icon::new(Glyph::Circle);
    let status_glyph = indicator.glyph_signal();
    let status_tone = signal(None);
    let tooltip = signal(String::new());
    let row = Visibility::new(
        Flex::column().gap(Spacing::Xs).padding_x(META_INDENT).width(Length::FULL)
            .child(Grid::new()
                .template_column("auto 1fr auto")
                .template_area(["logo primary status", ". session session"])
                .align("center").gap(Spacing::Sm)
                .child(Flex::row().child(logo).child(generic).area("logo"))
                .child(primary.area("primary"))
                .child(session.area("session"))
                .child(Flex::row().align("center").gap(Spacing::Xs).content_tone_signal(status_tone)
                    .child(indicator).child(status).area("status")))
            .child(task).tooltip_signal(tooltip),
        false,
    );
    let visible = row.visible_signal();
    let facts = cx.facts.clone();
    let statuses = cx.agent_statuses;
    create_effect(move |_| {
        let facts = facts();
        set_if_changed(visible, facts.agent.is_some());
        let Some(agent) = facts.agent else { return };
        let statuses = statuses.get();
        let presentation = statuses.get(agent.status.as_str());
        let label = presentation.and_then(|p| p.label.as_deref()).unwrap_or(agent.status.as_str());
        let glyph = presentation.and_then(|p| p.icon.as_deref()).and_then(Glyph::from_prop_name)
            .unwrap_or(Glyph::Circle);
        set_if_changed(status_glyph, glyph);
        set_if_changed(status_tone, presentation.and_then(|p| p.tone.as_deref()).and_then(Tone::from_prop_name));
        set_if_changed(fallback_glyph, facts.icon);
        set_if_changed(generic_glyph, facts.icon);
        set_if_changed(logo_visible, agent.icon.is_some());
        set_if_changed(generic_visible, agent.icon.is_none());
        if let Some(icon) = agent.icon {
            let source = match icon {
                AgentIcon::File(path) => ImageSource::file(path),
                AgentIcon::Svg(svg) => ImageSource::bytes(svg.into_bytes()),
            };
            set_if_changed(logo_source, source);
        }
        let primary = agent.model.filter(|text| !text.is_empty()).unwrap_or(agent.name);
        let session = agent.session_title.filter(|text| !text.is_empty()).unwrap_or(facts.title);
        set_if_changed(session_visible, !session.is_empty() && session != primary);
        set_if_changed(primary_text, primary);
        set_if_changed(session_text, session);
        set_if_changed(status_text, label.to_owned());
        set_if_changed(task_visible, agent.task.as_ref().is_some_and(|task| !task.is_empty()));
        set_if_changed(task_text, agent.task.unwrap_or_default());
        set_if_changed(tooltip, agent.detail.unwrap_or_default());
    });
    row
}

fn text(cx: &LineCx<'_>, bold: bool) -> Label {
    PaneName { text: "", color: None, bold, font_scale: WidgetSize::Normal.font_scale(), theme: cx.theme }.build().widget
}

#[cfg(test)]
mod tests;
