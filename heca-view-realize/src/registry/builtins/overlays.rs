//! Registered overlays factories.

use super::*;

pub(super) fn overlay(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            let panel: heca_grid_ui::WidgetModel = match node.children.len() {
                1 => cx.child(node, 0)?,
                _ => attach_children(heca_grid_ui::WidgetModel::new(Flex::column()), node, cx)?,
            };
            let mut overlay = with_props(Overlay::new().panel(panel), node, theme);
            // **What the dismiss key means, declared like any other behaviour.** The overlay has
            // always held the keys; it answers this one, and this is the
            // described spelling — so a plugin's surface closes on Escape by saying what closing
            // means, and never by writing a key.
            if let Some(intent) = node.events.get(ViewEvent::Dismiss.name()) {
                let (intent, emit) = (intent.clone(), emit.clone());
                overlay = overlay.on_dismiss(move || emit(intent.clone()));
            }
            heca_grid_ui::WidgetModel::new(overlay)
        })
}

pub(super) fn key_hint_group(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    Ok({
            let subtree: heca_grid_ui::WidgetModel = match node.children.len() {
                1 => cx.child(node, 0)?,
                _ => attach_children(heca_grid_ui::WidgetModel::new(Flex::column()), node, cx)?,
            };
            heca_grid_ui::WidgetModel::new(with_props(KeyHintGroup::new(subtree), node, theme))
        })
}

pub(super) fn toast(node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    let theme = cx.theme;
    let emit = cx.emit;
    Ok({
            let mut toast = Toast::new(text_of(node)).severity(severity_prop(node));
            let inputs: super::surfaces::ToastInputs = cx.state(node)?;
            if let Some(millis) = inputs.countdown_ms { toast = toast.countdown(std::time::Duration::from_millis(millis)); }
            if let Some(glyph) = glyph_prop(node) {
                toast = toast.icon(glyph);
            }
            // `body_text` is applied first and a described `body` child overwrites the same slot
            // below, so children win without the arm having to look ahead.
            if let Some(body) = node.props.get("body_text").and_then(PropValue::as_text) {
                toast = toast.body_text(body);
            }
            if let Some(opened) = node.props.get("opened").and_then(PropValue::as_bool) {
                toast = toast.default_open(opened);
            }
            if let Some(position) = node
                .props
                .get("position")
                .and_then(PropValue::as_text)
                .and_then(toast_position)
            {
                toast = toast.position(position);
            }
            toast = with_props(toast, node, theme);

            // ── Slots ────────────────────────────────────────────────────────────────────────
            // `body` is the **default** slot, so an unslotted child is the body; `actions` are the
            // controls under it, one per child. The card once had no slots, because it
            // hand-drew itself and could not hold arbitrary content; it now lays out real children.
            //
            // A described action is an **ordinary described `Button`** carrying its own `press`
            // intent, which is what makes it a `prefix+/` target with nothing hint-related written:
            // being pickable is not opt-in.
            let mut described_actions = false;
            for (index, child) in node.children.iter().enumerate() {
                let realized = cx.child(node, index)?;
                match slot_of(child) {
                    Some("actions") => {
                        described_actions = true;
                        toast = toast.action(realized);
                    }
                    Some("body") | None => toast = toast.body(realized),
                    other => {
                        warn_unknown_slot(node, child, other, &["body", "actions"]);
                        toast = toast.body(realized);
                    }
                }
            }

            // **Children win over the text sugar**, the way a `Button`'s children win over its
            // `text`/`icon`: one content model, two spellings, never two paint paths. The text
            // props stay because they are the plain-data path a `ToastSpec` uses.
            if !described_actions
                && let Some(label) = node.props.get("action_text").and_then(PropValue::as_text)
                && let Some(carrier) = intent_carrier(node, ViewEvent::Action)
            {
                let emit = emit.clone();
                toast = toast.action(Button::new(label).on_click(move || emit(carrier.clone())));
            }
            if let Some(carrier) = intent_carrier(node, ViewEvent::Press) {
                let emit = emit.clone();
                toast = toast.on_click(move || emit(carrier.clone()));
            }
            if let Some(carrier) = intent_carrier(node, ViewEvent::Dismiss) {
                let emit = emit.clone();
                toast = toast.on_dismiss(move || emit(carrier.clone()));
            }
            heca_grid_ui::WidgetModel::new(toast)
        })
}
