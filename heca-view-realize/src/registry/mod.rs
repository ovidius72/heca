//! Host-owned widget definitions: factories and discovery share the same registrations.

use std::collections::BTreeMap;
use std::rc::Rc;
use serde::{Serialize, de::DeserializeOwned};
use heca_grid_ui::{Component, SetProp, Theme};
use heca_grid_ui::builders::IntoComponent;
use heca_view::{ViewEvent, ViewNode, WidgetId};
use crate::{FormBindings, IntentEmitter};

mod builtins;
mod error;
mod state;
mod validation;
pub use error::{RealizeError, RealizeErrorKind};

/// Discovery information belongs to the registered implementation, not another widget list.
#[derive(Clone, Debug, Serialize)]
pub struct WidgetMetadata {
    pub id: WidgetId,
    pub sdk_name: &'static str,
    pub native_name: &'static str,
    pub properties: Vec<String>,
    pub native_bindings: &'static [&'static str],
    pub minimum_version: u32,
    pub maximum_version: u32,
    pub accepts_children: bool,
    pub child_types: Vec<WidgetId>,
    pub events: Vec<String>,
    /// Semantic input consumed by a parent rather than a stand-alone renderable node.
    pub declaration: bool,
}

type Factory = dyn Fn(&ViewNode, &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError>;
type Validate = dyn Fn(&ViewNode, &Theme) -> Result<(), RealizeError>;
type ApplyState = dyn Fn(&mut dyn Component, &ViewNode, &Theme) -> Result<bool, RealizeError>;
type Update = dyn Fn(&mut dyn Component, &ViewNode, &mut RealizeCx<'_>) -> Result<bool, RealizeError>;

/// One host registration supplies a widget's factory and generated property metadata.
#[derive(Clone)]
pub struct WidgetDefinition {
    metadata: WidgetMetadata,
    factory: Rc<Factory>,
    validators: Vec<Rc<Validate>>,
    state_updates: Vec<Rc<ApplyState>>,
    updates: Vec<Rc<Update>>,
}

impl WidgetDefinition {
    /// Registerable construction; callers return a concrete widget without boxing it.
    pub fn new<C: IntoComponent + 'static>(
        id: impl Into<WidgetId>,
        factory: impl Fn(&ViewNode, &mut RealizeCx<'_>) -> Result<C, RealizeError> + 'static,
    ) -> Self {
        Self {
            metadata: WidgetMetadata { id: id.into(), sdk_name: "", native_name: "", properties: Vec::new(), native_bindings: &[], minimum_version: 1, maximum_version: 1, accepts_children: false, child_types: Vec::new(), events: vec![ViewEvent::Hint.name().into()], declaration: false },
            factory: Rc::new(move |node, cx| factory(node, cx).map(IntoComponent::into_component)),
            validators: Vec::new(),
            state_updates: Vec::new(),
            updates: Vec::new(),
        }
    }

    /// Native builders remain the source of declarative property availability.
    pub fn properties<T: SetProp>(mut self) -> Self {
        self.metadata.properties.extend(T::PROP_NAMES.iter().map(|name| (*name).to_string()));
        self.metadata.native_bindings = T::HOST_ONLY_BUILDERS;
        self.validators.push(Rc::new(|node, theme| {
            for property in T::PROP_NAMES {
                let Some(value) = node.props.get(*property) else { continue };
                let input = crate::style::prop_to_input(value, theme)
                    .filter(|input| T::accepts_prop(property, input));
                if input.is_none() {
                    return Err(RealizeError::invalid_property(node.kind.clone(), *property,
                        "value does not match the native builder's type"));
                }
            }
            Ok(())
        }));
        self
    }

    /// Typed constructor inputs define discovery and validation from their serialized fields.
    pub fn inputs<T: Default + Serialize + DeserializeOwned + 'static>(mut self) -> Result<Self, RealizeError> {
        let fields = state::fields::<T>().map_err(|detail| RealizeError::new(RealizeErrorKind::InvalidDefinition {
            widget: self.metadata.id.clone(), detail,
        }))?;
        self.metadata.properties.extend(fields);
        self.metadata.properties.sort();
        self.metadata.properties.dedup();
        self.validators.push(Rc::new(|node, theme| state::read::<T>(node, theme).map(|_| ())));
        Ok(self)
    }

    /// Live native state uses those same typed inputs and the existing native props seam.
    pub fn state<T: Default + Serialize + DeserializeOwned + 'static>(self) -> Result<Self, RealizeError> {
        let mut definition = self.inputs::<T>()?;
        definition.state_updates.push(Rc::new(|component, node, theme| {
            let state = state::read::<T>(node, theme)?;
            if !component.set_props(&state) {
                return Err(RealizeError::new(RealizeErrorKind::InvalidDefinition {
                    widget: node.kind.clone(), detail: "the factory did not bind its declared state inputs".into(),
                }));
            }
            component.base().mark_needs_layout();
            Ok(true)
        }));
        Ok(definition)
    }

    pub fn versions(mut self, minimum: u32, maximum: u32) -> Result<Self, RealizeError> {
        if minimum == 0 || minimum > maximum {
            return Err(RealizeError::new(RealizeErrorKind::InvalidDefinition {
                widget: self.metadata.id.clone(), detail: "invalid supported version range".into(),
            }));
        }
        self.metadata.minimum_version = minimum;
        self.metadata.maximum_version = maximum;
        Ok(self)
    }

    /// Descriptive type names for discovery and native/SDK coverage guards.
    pub fn names(mut self, sdk: &'static str, native: &'static str) -> Self {
        self.metadata.sdk_name = sdk;
        self.metadata.native_name = native;
        self
    }

    pub fn metadata(&self) -> &WidgetMetadata { &self.metadata }

    /// Validate constructor invariants before any factory in the tree acquires resources.
    pub fn validate(mut self, validate: impl Fn(&ViewNode, &Theme) -> Result<(), RealizeError> + 'static) -> Self {
        self.validators.push(Rc::new(validate)); self
    }

    /// Reconcile composed inputs through the native props seam and this registry's context.
    pub fn update(mut self, update: impl Fn(&mut dyn Component, &ViewNode, &mut RealizeCx<'_>) -> Result<bool, RealizeError> + 'static) -> Self {
        self.updates.push(Rc::new(update)); self
    }

    /// A typed declaration, such as a palette command, is interpreted by its parent.
    pub fn declaration(mut self) -> Self { self.metadata.declaration = true; self }

    /// Native state driven by structured child declarations, using the same props seam.
    pub(crate) fn state_node<T: 'static>(mut self,
        read: impl Fn(&ViewNode, &Theme) -> Result<T, RealizeError> + 'static) -> Self {
        self.state_updates.push(Rc::new(move |component, node, theme| {
            let state = read(node, theme)?;
            if !component.set_props(&state) {
                return Err(RealizeError::new(RealizeErrorKind::InvalidDefinition {
                    widget: node.kind.clone(), detail: "structured state is not bound by its factory".into(),
                }));
            }
            component.base().mark_needs_layout();
            Ok(true)
        }));
        self
    }

    /// Arbitrary registered child types, for generic layouts and content containers.
    pub fn children(mut self) -> Self { self.metadata.accepts_children = true; self }

    /// Semantic containers can constrain their children without a closed widget vocabulary.
    pub fn children_of(mut self, ids: &[WidgetId]) -> Self {
        self.metadata.accepts_children = true;
        self.metadata.child_types = ids.to_vec();
        self
    }

    pub fn events(mut self, events: &[ViewEvent]) -> Self {
        self.metadata.events.extend(events.iter().map(|event| event.name().to_string()));
        self.metadata.events.sort();
        self.metadata.events.dedup();
        self
    }
}

/// Populated once by a host. Ordinary component placement needs no registry calls.
#[derive(Clone)]
pub struct WidgetRegistry {
    definitions: BTreeMap<WidgetId, WidgetDefinition>,
    diagnostic: Rc<dyn Fn(&RealizeError)>,
}

impl Default for WidgetRegistry {
    fn default() -> Self {
        Self {
            definitions: BTreeMap::new(),
            diagnostic: Rc::new(|diagnostic| eprintln!("{diagnostic}; keeping valid widget data")),
        }
    }
}

impl WidgetRegistry {
    pub fn new() -> Self { Self::default() }

    /// Route recoverable data diagnostics to a host's existing reporting surface.
    /// Without a host sink they are printed to stderr; ordinary placement needs no setup.
    pub fn on_diagnostic(mut self, report: impl Fn(&RealizeError) + 'static) -> Self {
        self.diagnostic = Rc::new(report);
        self
    }

    /// The library definitions, using the same registration/error path as host additions.
    pub fn builtins() -> Result<Self, RealizeError> {
        let mut registry = Self::new();
        builtins::register(&mut registry)?;
        Ok(registry)
    }

    pub fn register(&mut self, definition: WidgetDefinition) -> Result<(), RealizeError> {
        let id = definition.metadata.id.clone();
        if id.as_str().is_empty() || id.as_str().chars().any(char::is_whitespace) {
            return Err(RealizeError::new(RealizeErrorKind::InvalidDefinition {
                widget: id, detail: "widget identifiers must be nonempty and contain no whitespace".into(),
            }));
        }
        if self.definitions.contains_key(&id) {
            return Err(RealizeError::new(RealizeErrorKind::DuplicateWidget(id)));
        }
        self.definitions.insert(id, definition);
        Ok(())
    }

    pub fn get(&self, id: &WidgetId) -> Option<&WidgetDefinition> { self.definitions.get(id) }
    pub fn ids(&self) -> impl Iterator<Item = &WidgetId> { self.definitions.keys() }
    pub fn definitions(&self) -> impl Iterator<Item = &WidgetDefinition> { self.definitions.values() }

    /// Apply described state through the native props seam, without recreating the component.
    pub fn update_state(&self, component: &mut dyn Component, node: &ViewNode, theme: &Theme) -> Result<bool, RealizeError> {
        let node = self.prepare(node, theme)?;
        let Some(definition) = self.get(&node.kind) else {
            return Err(RealizeError::new(RealizeErrorKind::UnknownWidget(node.kind.clone())));
        };
        let mut applied = false;
        for update in &definition.state_updates { applied |= update(component, &node, theme)?; }
        if !definition.updates.is_empty() {
            return Err(RealizeError::new(RealizeErrorKind::InvalidDefinition {
                widget: node.kind, detail: "composed updates require update() with the host emitter and form bindings".into(),
            }));
        }
        Ok(applied)
    }

    /// In-place updates that can also realize changed children, retaining native identity/state.
    pub fn update(&self, component: &mut dyn Component, node: &impl AsRef<ViewNode>, theme: &Theme,
        emit: &IntentEmitter, forms: &mut FormBindings) -> Result<bool, RealizeError> {
        self.update_for(None, component, node, theme, emit, forms)
    }

    pub fn update_for(&self, owner: Option<&str>, component: &mut dyn Component, node: &impl AsRef<ViewNode>,
        theme: &Theme, emit: &IntentEmitter, forms: &mut FormBindings) -> Result<bool, RealizeError> {
        let node = self.prepare(node.as_ref(), theme)?;
        let definition = self.get(&node.kind).expect("prepared definition");
        let mut applied = false;
        for update in &definition.state_updates { applied |= update(component, &node, theme)?; }
        let mut cx = RealizeCx { registry: self, owner, theme, emit, forms, path: Vec::new() };
        for update in &definition.updates { applied |= update(component, &node, &mut cx)?; }
        Ok(applied)
    }

    pub fn realize(&self, node: &impl AsRef<ViewNode>, theme: &Theme, emit: &IntentEmitter, forms: &mut FormBindings) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
        self.realize_for(None, node, theme, emit, forms)
    }

    /// A host supplies the trusted owner out of band. A description cannot impersonate one.
    pub fn realize_for(&self, owner: Option<&str>, node: &impl AsRef<ViewNode>, theme: &Theme, emit: &IntentEmitter, forms: &mut FormBindings) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
        let node = self.prepare(node.as_ref(), theme)?;
        let mut cx = RealizeCx { registry: self, owner, theme, emit, forms, path: Vec::new() };
        cx.build(&node)
    }
}

/// Services a factory receives from its host; recursion keeps one registry and error path.
pub struct RealizeCx<'a> {
    registry: &'a WidgetRegistry,
    owner: Option<&'a str>,
    pub(crate) theme: &'a Theme,
    pub(crate) emit: &'a IntentEmitter,
    pub(crate) forms: &'a mut FormBindings,
    path: Vec<usize>,
}

impl RealizeCx<'_> {
    pub fn theme(&self) -> &Theme { self.theme }
    pub fn emit(&self) -> &IntentEmitter { self.emit }
    pub fn owner(&self) -> Option<&str> { self.owner }

    pub fn state<T: Default + Serialize + DeserializeOwned>(&self, node: &ViewNode) -> Result<T, RealizeError> {
        state::read(node, self.theme)
    }

    fn build(&mut self, node: &ViewNode) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
        let definition = self.registry.get(&node.kind).ok_or_else(|| RealizeError {
            kind: RealizeErrorKind::UnknownWidget(node.kind.clone()), path: self.path.clone(),
        })?;
        let factory = definition.factory.clone();
        let state_updates = definition.state_updates.clone();
        let updates = definition.updates.clone();
        let mut built = factory(node, self).map_err(|mut error| {
            if error.path.is_empty() { error.path = self.path.clone(); }
            error
        })?;
        for update in state_updates {
            update(built.as_mut(), node, self.theme).map_err(|mut error| { error.path = self.path.clone(); error })?;
        }
        for update in updates { update(built.as_mut(), node, self)?; }
        crate::apply_common_props(built.as_mut(), node, self.theme, self.emit, self.forms);
        Ok(built)
    }

    /// Realize a child in this host, including registered custom implementations.
    pub fn child(&mut self, parent: &ViewNode, index: usize) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
        self.at_child(parent, index, |node, cx| cx.build(node))
    }

    pub(crate) fn typed_child<C: Component + SetProp>(&mut self, parent: &ViewNode, index: usize, build: impl FnOnce(&ViewNode, &mut Self) -> Result<C, RealizeError>) -> Result<C, RealizeError> {
        self.at_child(parent, index, |node, cx| {
            let mut built = crate::style::with_props(build(node, cx)?, node, cx.theme);
            crate::apply_common_props(&mut built, node, cx.theme, cx.emit, cx.forms);
            Ok(built)
        })
    }

    fn at_child<C>(&mut self, parent: &ViewNode, index: usize, build: impl FnOnce(&ViewNode, &mut Self) -> Result<C, RealizeError>) -> Result<C, RealizeError> {
        let node = parent.children.get(index).ok_or_else(|| RealizeError {
            kind: RealizeErrorKind::InvalidChild { widget: parent.kind.clone(), index, detail: "child index out of bounds".into() }, path: self.path.clone(),
        })?;
        self.path.push(index);
        let result = build(node, self).map_err(|mut error| { if error.path.is_empty() { error.path = self.path.clone(); } error });
        self.path.pop();
        result
    }

    pub fn children(&mut self, parent: &ViewNode) -> Result<Vec<heca_grid_ui::WidgetModel>, RealizeError> {
        (0..parent.children.len()).map(|index| self.child(parent, index)).collect()
    }
}
