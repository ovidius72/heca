//! Registered controls factories.

use super::*;

pub(super) fn button(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    realize_button(node, cx)
}

pub(super) fn landing_slot(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok(heca_grid_ui::WidgetModel::new(with_props(LandingSlot::new(), node, theme)))
}

pub(super) fn splitter(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            let mut splitter = with_props(Splitter::vertical(), node, theme);
            if let Some(intent) = node.intent(ViewEvent::Resize) {
                let (intent, emit) = (intent.clone(), emit.clone());
                splitter = splitter
                    .on_resize(move |delta| emit(intent.clone().arg("delta", delta.into())));
            }
            heca_grid_ui::WidgetModel::new(splitter)
        })
}

pub(super) fn button_group(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            let mut group = with_props(heca_grid_ui::widgets::ButtonGroup::new(), node, theme);
            for (index, child) in node.children.iter().enumerate() {
                if child.kind == WidgetId::Button {
                    group = group.child(cx.typed_child(node, index, button_of)?);
                } else {
                    return Err(RealizeError::new(crate::RealizeErrorKind::InvalidChild {
                        widget: node.kind.clone(), index, detail: "button groups require Button children".into(),
                    }));
                }
            }
            heca_grid_ui::WidgetModel::new(group)
        })
}

pub(super) fn input(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            let mut input = with_props(Input::new().value(text_of(node)), node, theme);
            if let Some(name) = name_prop(node) {
                let sig = input.text();
                cx.forms.bind(
                    name.clone(),
                    Box::new(move || PropValue::Text(sig.get_untracked())),
                );
                // Also expose the live signal for reactive validation (disabled submit button).
                cx.forms.bind_text_signal(name, sig);
            }
            if let Some(carrier) = change_intent(node) {
                let emit = emit.clone();
                input = input.on_change(move |_a: Action| emit(carrier.clone()));
            }
            if let Some(carrier) = intent_carrier(node, ViewEvent::Submit) {
                let emit = emit.clone();
                input = input.on_submit(move |action| {
                    if let heca_grid_ui::SignalData::String(value) = action.data {
                        emit(carrier.clone().arg("value", PropValue::Text(value)));
                    }
                });
            }
            heca_grid_ui::WidgetModel::new(input)
        })
}

pub(super) fn toggle(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            let mut t = with_props(Toggle::new(), node, theme);
            if let Some(name) = name_prop(node) {
                let sig = t.state();
                cx.forms.bind(name, Box::new(move || PropValue::Bool(sig.get_untracked())));
            }
            if let Some(carrier) = change_intent(node) {
                let emit = emit.clone();
                t = t.on_change(move |_a: Action| emit(carrier.clone()));
            }
            heca_grid_ui::WidgetModel::new(t)
        })
}

pub(super) fn checkbox(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            // `checked` arrives through the surface; the widget's own default is already false.
            let mut c = with_props(Checkbox::new().label(text_of(node)), node, theme);
            if let Some(name) = name_prop(node) {
                let sig = c.state();
                cx.forms.bind(name, Box::new(move || PropValue::Bool(sig.get_untracked())));
            }
            if let Some(carrier) = change_intent(node) {
                let emit = emit.clone();
                c = c.on_change(move |_a: Action| emit(carrier.clone()));
            }
            heca_grid_ui::WidgetModel::new(c)
        })
}

pub(super) fn badge_button(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let emit = cx.emit;
    Ok({
            let mut b = with_props(BadgeButton::new(text_of(node)), node, cx.theme());
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                b = b.on_click(move || emit(carrier.clone()));
            }
            heca_grid_ui::WidgetModel::new(b)
        })
}

pub(super) fn icon_button(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let emit = cx.emit;
    Ok({
            let icon = Icon::new(glyph_prop(node).unwrap_or(Glyph::Circle));
            let mut b = with_props(IconButton::new(icon), node, cx.theme());
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                b = b.on_click(move || emit(carrier.clone()));
            }
            heca_grid_ui::WidgetModel::new(b)
        })
}

pub(super) fn item(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let emit = cx.emit;
    Ok({
            let mut it = Item::new(text_of(node));
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                it = it.on_activate(move || emit(carrier.clone()));
            }
            // Slots: the row's leading / trailing affordances. It has **no default slot** — its
            // middle is the label, which comes from `text` — so a child with neither slot name is
            // ignored rather than silently dropped somewhere it doesn't belong.
            for (index, child) in node.children.iter().enumerate() {
                let realized = cx.child(node, index)?;
                match slot_of(child) {
                    Some("leading") => it = it.leading(realized),
                    Some("trailing") => it = it.trailing(realized),
                    other => warn_unknown_slot(node, child, other, &["leading", "trailing"]),
                }
            }
            heca_grid_ui::WidgetModel::new(it)
        })
}

pub(super) fn rail_cell(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let emit = cx.emit;
    Ok({
            let icon = Icon::new(glyph_prop(node).unwrap_or(Glyph::Circle));
            let mut cell = with_props(RailCell::new(icon), node, cx.theme());
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                cell = cell.on_activate(move || emit(carrier.clone()));
            }
            heca_grid_ui::WidgetModel::new(cell)
        })
}

pub(super) fn choice(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let emit = cx.emit;
    Ok({
            let mut choice = realize_choice(node, cx)?;
            // A standalone `Choice` (outside a Select/Tabs) is activatable on its own. Inside a
            // container the container owns the click, so a `press` there is ignored, not half-wired.
            if let Some(carrier) = press_intent(node) {
                let emit = emit.clone();
                choice = choice.on_activate(move || emit(carrier.clone()));
            }
            heca_grid_ui::WidgetModel::new(choice)
        })
}

pub(super) fn select(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            let mut select = Select::empty();
            for option in realize_options(node, cx)? {
                select = select.option(option);
            }
            // After the options, so `selected` clamps against the real count. That ordering is a
            // property of this arm's construction, not something a property author must know.
            select = with_props(select, node, theme);
            if let Some(on_change) = option_change(node, emit) {
                select = select.on_change(on_change);
            }
            // A named `Select` is a form field like `Input`/`Toggle`/`Checkbox`: its chosen
            // option's **value** (not the index) is marshalled into the submitted `data`. The live
            // selection is an index signal, so bind a reader that maps it through the option values.
            if let Some(name) = name_prop(node) {
                let values = option_values(node);
                let idx = select.state();
                cx.forms.bind(
                    name,
                    Box::new(move || {
                        PropValue::Text(
                            values.get(idx.get_untracked()).cloned().unwrap_or_default(),
                        )
                    }),
                );
            }
            heca_grid_ui::WidgetModel::new(select)
        })
}

pub(super) fn tabs(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            let mut tabs = Tabs::empty();
            for option in realize_options(node, cx)? {
                tabs = tabs.tab(option);
            }
            tabs = with_props(tabs, node, theme);
            if let Some(on_change) = option_change(node, emit) {
                tabs = tabs.on_change(on_change);
            }
            heca_grid_ui::WidgetModel::new(tabs)
        })
}
