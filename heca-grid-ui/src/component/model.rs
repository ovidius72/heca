//! An owned component tree with private allocation and ordinary builder placement.

use std::ops::{Deref, DerefMut};
use super::Component;

/// An owned widget tree whose concrete type is chosen at runtime.
///
/// Native authors return ordinary components; factories and child builders create this value
/// internally. A model can be placed through the same `.child(...)` builder as any widget.
/// Its allocation is private:
///
/// ```compile_fail
/// use heca_grid_ui::{Label, WidgetModel};
/// let model = WidgetModel { inner: Box::new(Label::new("private")) };
/// ```
pub struct WidgetModel {
    inner: Box<dyn Component>,
}

impl WidgetModel {
    /// Own a concrete component. Ordinary child/factory builders do this conversion for callers.
    pub fn new(component: impl Component + 'static) -> Self {
        Self { inner: Box::new(component) }
    }
}

impl<C: Component + 'static> From<C> for WidgetModel {
    fn from(component: C) -> Self { Self::new(component) }
}

impl Deref for WidgetModel {
    type Target = dyn Component;
    fn deref(&self) -> &Self::Target { self.inner.as_ref() }
}

impl DerefMut for WidgetModel {
    fn deref_mut(&mut self) -> &mut Self::Target { self.inner.as_mut() }
}

impl AsRef<dyn Component> for WidgetModel {
    fn as_ref(&self) -> &(dyn Component + 'static) { self.inner.as_ref() }
}

impl AsMut<dyn Component> for WidgetModel {
    fn as_mut(&mut self) -> &mut (dyn Component + 'static) { self.inner.as_mut() }
}
