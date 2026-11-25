//! **`demo.layer("planner")`** — an overlay that belongs to no dock. *Client side.*
//!
//! ```ignore
//! let demo = heca::extension("demo");
//! demo.layer("planner").view(|cx| {
//!     Overlay::new().panel(PlannerDetail::new(cx.state())).default_open(false)
//! });
//! ```
//!
//! What it gives back is a name, `demo.planner`: `toggle_layer name=demo.planner` opens it from a key binding, a
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
//!
//! **A layer can take arguments** — what it is about, when one layer shows one of many things (a
//! room's chat, a file's history). It declares them with [`arg`](LayerBuilder::arg) and reads them
//! with [`LayerCx::arg`]; `toggle_layer name=demo.chat room=auth` passes them. A required argument
//! that is missing, or one the layer does not take, is refused with a sentence instead of opening
//! an empty surface. The last arguments a layer was shown with are kept, so a rebuild (a theme
//! reload, the session changing under it) shows the same thing.
//!
//! ```ignore
//! demo.layer("chat")
//!     .arg("room", heca::ArgKind::Text, true, "The room whose chat it shows")
//!     .view(|cx| ChatView::new(cx.arg("room").unwrap_or_default()));
//! ```

use std::collections::HashMap;
use std::rc::Rc;

use heca_grid_ui::builders::IntoComponent;
use heca_grid_ui::theme::Theme as GuiTheme;

use crate::app_state::AppState;
use crate::args::{ArgKind, ArgSpec};
use crate::chrome::{ChromeIntentEmitter, StartupQueue, WidgetModel};
use crate::host::StateView;

/// What a layer's tree is built from: the theme it is drawn in, the sink its widgets report to, and
/// a read-only view of the app. The same three a dock's body gets, minus the host's registries.
pub struct LayerCx<'a> {
    theme: &'a GuiTheme,
    emit: &'a ChromeIntentEmitter,
    state: StateView<'a>,
    arguments: &'a [(String, String)],
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

    /// **The value of one of this layer's arguments** — what `toggle_layer name=demo.chat
    /// room=auth` passed as `room`. `None` when it was not given (an optional argument). The layer
    /// declares what it takes with [`LayerBuilder::arg`].
    pub fn arg(&self, name: &str) -> Option<&str> {
        self.arguments
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// How a layer's tree is built.
type Build = dyn Fn(&LayerCx<'_>) -> WidgetModel;

/// A layer as declared: its name, the arguments it takes, and how its tree is built.
struct Declared {
    name: String,
    args: Vec<ArgSpec>,
    build: Rc<Build>,
}

/// A layer being declared. Made by [`Extension::layer`](super::Extension::layer).
pub struct LayerBuilder {
    /// `<extension>.<short>`; empty when the layer was refused (already reported).
    name: String,
    /// The arguments it takes, in the order declared.
    args: Vec<ArgSpec>,
}

impl LayerBuilder {
    pub(super) fn named(name: String) -> Self {
        Self {
            name,
            args: Vec::new(),
        }
    }

    /// **An argument this layer takes** — what it is about when it shows one of many things.
    /// `toggle_layer name=demo.chat room=auth` passes it; [`LayerCx::arg`] reads it as the layer
    /// builds. A `required` one that is missing refuses the action with a sentence, and so does an
    /// argument the layer did not declare.
    ///
    /// ```
    /// use heca_grid_ui::{Label, Overlay};
    ///
    /// let demo = heca::extension("layer_arg_example");
    /// demo.layer("chat")
    ///     .arg("room", heca::ArgKind::Text, true, "The room whose chat it shows")
    ///     .view(|cx| Overlay::new().panel(Label::new(cx.arg("room").unwrap_or_default())));
    /// ```
    pub fn arg(
        mut self,
        name: impl Into<String>,
        kind: ArgKind,
        required: bool,
        description: impl Into<String>,
    ) -> Self {
        self.args.push(ArgSpec {
            name: name.into(),
            kind,
            required,
            description: description.into(),
            values: Vec::new(),
        });
        self
    }
}

thread_local! {
    /// Declarations made before the host took them.
    static QUEUE: StartupQueue<Declared> = const { StartupQueue::new() };
}

impl LayerBuilder {
    /// **What it shows.** Return an ordinary widget or layout; no boxing or cast is required.
    /// Queues it; the host registers it as it starts, hidden.
    ///
    /// ```
    /// use heca_grid_ui::builders::Parent;
    /// use heca_grid_ui::{Label, Overlay, Pane};
    ///
    /// let demo = heca::extension("layer_example");
    /// demo.layer("details").view(|_cx| {
    ///     Overlay::new().panel(Pane::new().child(Label::new("Details")))
    /// });
    /// ```
    pub fn view<C: IntoComponent + 'static>(self, build: impl Fn(&LayerCx<'_>) -> C + 'static) {
        // A refused layer carries no name and was already reported where it was refused.
        let name = self.name;
        if name.is_empty() {
            return;
        }
        let build: Rc<Build> = Rc::new(move |cx| build(cx).into_component());
        let declared = Declared {
            name,
            args: self.args,
            build,
        };
        if let Err(Declared { name, .. }) = QUEUE.with(|q| q.add(declared)) {
            crate::chrome::warn_author(format!(
                "[heca] layer '{name}' was added after the app started, so it does nothing — add \
                 it before `heca::run(..)`"
            ));
        }
    }
}

/// The builders of the layers another crate added, by name — kept for the life of the app so a layer
/// can be built afresh each time it is asked for. Lives on the client's state.
#[derive(Default)]
pub(crate) struct AddedLayers {
    by_name: HashMap<String, Added>,
}

/// One layer another crate added: what it takes, how it is built, and the arguments it was last
/// shown with.
struct Added {
    args: Vec<ArgSpec>,
    build: Rc<Build>,
    shown_with: Vec<(String, String)>,
}

impl AddedLayers {
    /// The arguments the added layer `name` was last shown with — empty for one never given any, or
    /// for a name nothing added.
    pub(crate) fn shown_with(&self, name: &str) -> &[(String, String)] {
        self.by_name
            .get(name)
            .map_or(&[], |added| added.shown_with.as_slice())
    }

    /// **Check `arguments` against what the added layer `name` takes**: every required one given,
    /// none it does not take. The sentence says what is wrong, for the user or the script that
    /// asked. A name nothing added (a host layer) takes whatever it is given.
    pub(crate) fn check(&self, name: &str, arguments: &[(String, String)]) -> Result<(), String> {
        let Some(added) = self.by_name.get(name) else {
            return Ok(());
        };
        if let Some((unknown, _)) = arguments
            .iter()
            .find(|(key, _)| !added.args.iter().any(|arg| &arg.name == key))
        {
            return Err(format!("layer '{name}' takes no argument '{unknown}'"));
        }
        if let Some(missing) = added
            .args
            .iter()
            .find(|arg| arg.required && !arguments.iter().any(|(key, _)| key == &arg.name))
        {
            return Err(format!(
                "layer '{name}' needs '{}': {}",
                missing.name, missing.description
            ));
        }
        Ok(())
    }

    /// **Show the added layer `name` with `arguments` from now on** — kept, so every later rebuild
    /// shows the same thing until other arguments are given.
    pub(crate) fn remember(&mut self, name: &str, arguments: &[(String, String)]) {
        if let Some(added) = self.by_name.get_mut(name) {
            added.shown_with = arguments.to_vec();
        }
    }
}

/// Take the queue, close it, and register every layer — hidden, under its name. The host calls this
/// once, as it starts.
pub(crate) fn register_queued_layers(state: &mut AppState) {
    for Declared { name, args, build } in QUEUE.with(StartupQueue::take) {
        let added = Added {
            args,
            build,
            shown_with: Vec::new(),
        };
        state.added_layers.by_name.insert(name.clone(), added);
        rebuild(state, &name);
    }
}

/// Build the added layer called `name` afresh and put it in the stack, keeping its place, whether
/// it is up, and the arguments it was last shown with. `false` when nothing added by another crate
/// has that name.
pub(crate) fn rebuild(state: &mut AppState, name: &str) -> bool {
    let Some((build, arguments)) = state
        .added_layers
        .by_name
        .get(name)
        .map(|added| (added.build.clone(), added.shown_with.clone()))
    else {
        return false;
    };
    let theme = crate::chrome::chrome_gui_theme(state);
    let store = state.chrome_state.clone();
    crate::chrome::register_named_layer(state, name, |emit| {
        build(&LayerCx {
            theme: &theme,
            emit,
            state: StateView::over(&store),
            arguments: &arguments,
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
        let demo = super::super::extension("demo");
        demo.layer("planner")
            .view(|_| heca_grid_ui::Label::new("Only built when shown"));
        demo.layer("a.b")
            .view(|_| heca_grid_ui::Label::new("Refused"));
        let names: Vec<String> =
            QUEUE.with(|q| q.peek(|items| items.iter().map(|d| d.name.clone()).collect()));
        assert_eq!(names, ["demo.planner"]);
    }

    #[test]
    fn a_plain_component_factory_builds_fresh_content_without_author_storage_conversions() {
        use heca_grid_ui::builders::Parent;
        use heca_grid_ui::{Flex, Label};
        use std::cell::Cell;

        super::super::extension::reset();
        QUEUE.with(StartupQueue::reset);
        let calls = Rc::new(Cell::new(0));
        let count = calls.clone();
        super::super::extension("plain_layer")
            .layer("content")
            .view(move |_| {
                count.set(count.get() + 1);
                Flex::column().child(Label::new(format!("Build {}", count.get())))
            });
        assert_eq!(
            calls.get(),
            0,
            "startup declares the builder rather than running it"
        );
        let build = QUEUE.with(|queue| queue.peek(|items| items[0].build.clone()));
        let theme = GuiTheme::default();
        let emit = ChromeIntentEmitter::of(
            crate::app::interaction::InteractionSource::Keyboard,
            |_, _| {},
        );
        let store = crate::chrome::SharedChromeState::new(240.0, 240.0);
        let cx = LayerCx {
            theme: &theme,
            emit: &emit,
            state: StateView::over(&store),
            arguments: &[],
        };
        let first = build(&cx);
        let second = build(&cx);
        assert_eq!(
            first.base().children[0].text_summary().as_deref(),
            Some("Build 1")
        );
        assert_eq!(
            second.base().children[0].text_summary().as_deref(),
            Some("Build 2")
        );
        assert_eq!(calls.get(), 2);
    }

    fn added(args: Vec<ArgSpec>) -> AddedLayers {
        let mut layers = AddedLayers::default();
        layers.by_name.insert(
            "demo.chat".into(),
            Added {
                args,
                build: Rc::new(|_| heca_grid_ui::Label::new("chat").into_component()),
                shown_with: Vec::new(),
            },
        );
        layers
    }

    fn room() -> ArgSpec {
        ArgSpec {
            name: "room".into(),
            kind: ArgKind::Text,
            required: true,
            description: "The room whose chat it shows".into(),
            values: Vec::new(),
        }
    }

    fn given(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// **A layer declares the arguments it takes, and reads the ones it was given as it builds.**
    #[test]
    fn a_layer_declares_its_arguments_and_reads_them() {
        super::super::extension::reset();
        QUEUE.with(StartupQueue::reset);
        super::super::extension("args_layer")
            .layer("chat")
            .arg("room", ArgKind::Text, true, "The room whose chat it shows")
            .view(|cx| heca_grid_ui::Label::new(cx.arg("room").unwrap_or("none").to_string()));
        let (args, build) =
            QUEUE.with(|queue| queue.peek(|items| (items[0].args.clone(), items[0].build.clone())));
        assert_eq!(args.len(), 1);
        assert!(args[0].required && args[0].name == "room");
        let theme = GuiTheme::default();
        let emit = ChromeIntentEmitter::of(
            crate::app::interaction::InteractionSource::Keyboard,
            |_, _| {},
        );
        let store = crate::chrome::SharedChromeState::new(240.0, 240.0);
        let arguments = given(&[("room", "auth")]);
        let built = build(&LayerCx {
            theme: &theme,
            emit: &emit,
            state: StateView::over(&store),
            arguments: &arguments,
        });
        assert_eq!(built.text_summary().as_deref(), Some("auth"));
    }

    /// **A missing required argument, or one the layer does not take, is a sentence — not an empty
    /// surface.** A host layer (nothing added under its name) takes what it is given.
    #[test]
    fn arguments_are_checked_against_what_the_layer_takes() {
        let layers = added(vec![room()]);
        assert_eq!(
            layers.check("demo.chat", &given(&[("room", "auth")])),
            Ok(())
        );
        let missing = layers.check("demo.chat", &[]).unwrap_err();
        assert!(missing.contains("needs 'room'"), "{missing}");
        let unknown = layers
            .check("demo.chat", &given(&[("room", "auth"), ("rooom", "x")]))
            .unwrap_err();
        assert!(unknown.contains("no argument 'rooom'"), "{unknown}");
        assert_eq!(layers.check("heca.expose", &given(&[("any", "x")])), Ok(()));
    }

    /// **The arguments a layer was last shown with are kept** for every later rebuild.
    #[test]
    fn the_last_arguments_are_kept() {
        let mut layers = added(vec![room()]);
        assert!(layers.shown_with("demo.chat").is_empty());
        layers.remember("demo.chat", &given(&[("room", "auth")]));
        assert_eq!(
            layers.shown_with("demo.chat"),
            given(&[("room", "auth")]).as_slice()
        );
        assert!(layers.shown_with("nothing.added").is_empty());
    }
}
