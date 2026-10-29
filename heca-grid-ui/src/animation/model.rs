//! Opaque storage for a live animation. Authors pass their concrete `Animate` type.
use super::Animate;
use std::ops::{Deref, DerefMut};

/// An owned animation whose storage is private, like [`crate::WidgetModel`].
pub struct AnimationModel(Box<dyn Animate>);

impl AnimationModel {
    pub fn new(animation: impl Animate + 'static) -> Self { Self(Box::new(animation)) }
}

impl Deref for AnimationModel {
    type Target = dyn Animate;
    fn deref(&self) -> &Self::Target { self.0.as_ref() }
}

impl DerefMut for AnimationModel {
    fn deref_mut(&mut self) -> &mut Self::Target { self.0.as_mut() }
}
