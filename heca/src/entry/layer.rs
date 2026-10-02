//! **`pro.layer("planner")`** — an overlay that belongs to no dock. *Client side.*
//!
//! ```ignore
//! let pro = heca::extension("pro");
//! pro.layer("planner").view(|cx| {
//!     Overlay::new().panel(PlannerDetail::new(cx.state())).default_open(false)
//! });
//! ```
//!
//! What it gives back is a name, `pro.planner`: `toggle_layer name=pro.planner` opens it from a key binding, a
//! menu entry, the palette and RPC alike, because the layer is registered under that name like the
//! exposé is. There is nothing else to remember — no id, no registry.
//!
//! **The closure builds the whole tree, and is called again each time the layer is asked for**, so
//! what it shows is current: a layer's content is structural, and a signal replaces a value, never
//! a child. Which is also what a theme reload needs. What the surface *is* — that it takes the
//! keyboard, that it covers the panes, its animation, its frost — is said on the widget it
//! returns, the way every surface says it.
//!
//! **Names are `<extension>.<short>`**, the owner half coming from the [`Extension`](super::Extension)
//! the layer hangs off — never written per layer.

use std::collections::HashMap;
use std::rc::Rc;

use heca_grid_ui::theme::Theme as GuiTheme;

use crate::app_state::AppState;
use crate::chrome::{ChromeIntentEmitter, StartupQueue, WidgetModel};
use crate::host::StateView;

/// What a layer's tree is built from: the theme it is drawn in, the sink its widgets report to, and
/// a read-only view of the app. The same three a dock's body gets, minus the host's registries.
pub struct LayerCx<'a> {
    theme: &'a GuiTheme,
    emit: &'a ChromeIntentEmitter,
    state: StateView<'a>,
}

impl<'a> LayerCx<'a> {
    /// The theme this frame is drawn in. Read every colour and size from it.
    pub fn theme(&self) -> &GuiTheme {
        self.theme
    }

    /// Where this layer's widgets report what the user did — `fires(mount, intent, cx.emit())`.
    /// Everything it sends is stamped as coming from this layer, so the router can tell "the
    /// surface acted on itself" from "the user typed at the app behind it".
    pub fn emit(&self) -> &ChromeIntentEmitter {
        self.emit
    }

    /// Read-only questions about the app.
    pub fn state(&self) -> &StateView<'a> {
        &self.state
    }
}

/// How a layer's tree is built.
type Build = dyn Fn(&LayerCx<'_>) -> WidgetModel;

/// A layer being declared. Made by [`Extension::layer`](super::Extension::layer).
pub struct LayerBuilder {
    /// `<extension>.<short>`; empty when the layer was refused (already reported).
    name: String,
}

impl LayerBuilder {
    pub(super) fn named(name: String) -> Self {
        Self { name }
    }
}

thread_local! {
    /// Declarations made before the host took them.
    static QUEUE: StartupQueue<(String, Rc<Build>)> = const { StartupQueue::new() };
}

impl LayerBuilder {
    /// **What it shows.** Queues it; the host registers it as it starts, hidden.
    pub fn view(self, build: impl Fn(&LayerCx<'_>) -> WidgetModel + 'static) {
        // A refused layer carries no name and was already reported where it was refused.
        let name = self.name;
        if name.is_empty() {
            return;
        }
        if let Err((name, _)) = QUEUE.with(|q| q.add((name, Rc::new(build)))) {
            crate::chrome::warn_author(format!(
                "[heca] layer '{name}' was added after the app started, so it does nothing — add \
                 it before `heca::run()`"
            ));
        }
    }
}

/// The builders of the layers another crate added, by name — kept for the life of the app so a layer
/// can be built afresh each time it is asked for. Lives on the client's state.
#[derive(Default)]
pub(crate) struct AddedLayers {
    by_name: HashMap<String, Rc<Build>>,
}

/// Take the queue, close it, and register every layer — hidden, under its name. The host calls this
/// once, as it starts.
pub(crate) fn register_queued_layers(state: &mut AppState) {
    for (name, build) in QUEUE.with(StartupQueue::take) {
        state.added_layers.by_name.insert(name.clone(), build);
        rebuild(state, &name);
    }
}

/// Build the added layer called `name` afresh and put it in the stack, keeping its place and whether
/// it is up. `false` when nothing added by another crate has that name.
pub(crate) fn rebuild(state: &mut AppState, name: &str) -> bool {
    let Some(build) = state.added_layers.by_name.get(name).cloned() else {
        return false;
    };
    let theme = crate::chrome::chrome_gui_theme(state);
    let store = state.chrome_state.clone();
    crate::chrome::register_named_layer(state, name, |emit| {
        build(&LayerCx {
            theme: &theme,
            emit,
            state: StateView::over(&store),
        })
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A layer is queued under `<extension>.<short>`, and a refused one is not queued.**
    #[test]
    fn a_layer_is_queued_under_its_extensions_name() {
        super::super::extension::reset();
        QUEUE.with(StartupQueue::reset);
        let pro = super::super::extension("pro");
        pro.layer("planner")
            .view(|_| unreachable!("only built when shown"));
        pro.layer("a.b").view(|_| unreachable!("refused"));
        let names: Vec<String> =
            QUEUE.with(|q| q.peek(|items| items.iter().map(|(n, _)| n.clone()).collect()));
        assert_eq!(names, ["pro.planner"]);
    }
}
