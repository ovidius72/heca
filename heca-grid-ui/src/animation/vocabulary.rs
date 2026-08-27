//! [`Animation`] — the animation a caller *names*, and the one door a description uses too.

use super::{Animate, Fade, Slide, Zoom, ZoomFade};

/// **How a surface arrives and leaves — named, in one word.**
///
/// ```ignore
/// Overlay::new().panel(body).animation(Animation::Fade)
/// Overlay::new().panel(body).animation(Animation::ZoomFade)
/// Overlay::new().panel(body).animation(Animation::Zoom.from(0.8))          // tuned
/// Overlay::new().panel(body).animation(Animation::of(MyWhirl::new()))      // …or your own
/// ```
///
/// # Why a value, when the model is a trait
///
/// The **extension point** is [`Animate`]: anyone can write an animation, in their own crate, and
/// nothing here has to learn the name. But almost nobody wants to compose one — they want the
/// gesture this library already knows, said in one word. So the built-ins are named here, and
/// [`Custom`](Animation::Custom) keeps the door open, which is what makes this a vocabulary rather
/// than a wall: a plugin's own animation is a *type*, and it is passed exactly where a built-in is.
///
/// It is also the **one door for a description**: the variants are the names a `ViewNode` may use
/// (`"fade"`, `"zoom_fade"`), so native code and JSON reach the same builder rather than two.
/// `Custom` has no name, because a live object cannot cross as data — an unknown name leaves the
/// surface with its default, never a panic.
///
/// # Adding one
///
/// A **third party** adds nothing here: they write a type and pass [`of`](Animation::of). A
/// built-in shipped *by this library* is a new file in `animation/` plus one arm here — the name is
/// the only thing that has to be written down, and nothing in the painter, the widgets or any host
/// changes either way.
#[derive(Default)]
pub enum Animation {
    /// A cut: the surface is simply there, then simply gone. The default.
    #[default]
    None,
    /// A dissolve, both ways.
    Fade,
    /// Growing in from smaller, shrinking away again.
    Zoom,
    /// Travelling in from an edge, and back out to it — the notification gesture. Tune the edge
    /// and the distance through [`Slide`]'s own builders and [`of`](Animation::of).
    Slide,
    /// Pulling back into view, and on the way out the dissolve rides the shrink — the exposé's
    /// gesture. See [`ZoomFade`].
    ZoomFade,
    /// An animation this library has never heard of. Build it with [`of`](Animation::of).
    Custom(Box<dyn Animate>),
}

impl Animation {
    /// **An animation of your own** — any type implementing [`Animate`], passed where a built-in
    /// would be. This is the whole extension story: no registration, no name, no host change.
    pub fn of(animation: impl Animate + 'static) -> Self {
        Self::Custom(Box::new(animation))
    }

    /// **How far away it starts**, for the built-ins that travel: below `1.0` grows in from
    /// smaller, above it pulls back from larger.
    ///
    /// A built-in with no such dimension ([`Fade`](Animation::Fade), [`None`](Animation::None)) and
    /// a [`Custom`](Animation::Custom) one — which owns its own settings — are returned unchanged.
    pub fn from(self, scale: f32) -> Self {
        match self {
            Self::Zoom => Self::of(Zoom::new().from(scale)),
            Self::ZoomFade => Self::of(ZoomFade::from_scale(scale)),
            other => other,
        }
    }

    /// **How long one arrival or exit takes.** Zero is a cut.
    ///
    /// A [`Custom`](Animation::Custom) animation owns its own timing and is returned unchanged.
    pub fn seconds(self, seconds: f32) -> Self {
        match self {
            Self::Fade => Self::of(Fade::new().seconds(seconds)),
            Self::Zoom => Self::of(Zoom::new().seconds(seconds)),
            Self::Slide => Self::of(Slide::new().seconds(seconds)),
            other => other,
        }
    }

    /// The live animation this names — **`None` for a surface that simply appears and goes**.
    ///
    /// An absence, not a do-nothing object: a surface with no animation is never mid-gesture, so
    /// nothing waits for it and nothing anywhere has a case for it.
    pub fn build(self) -> Option<Box<dyn Animate>> {
        match self {
            Self::None => Option::None,
            Self::Fade => Some(Box::new(Fade::new())),
            Self::Zoom => Some(Box::new(Zoom::new())),
            Self::Slide => Some(Box::new(Slide::new())),
            Self::ZoomFade => Some(Box::new(ZoomFade::new())),
            Self::Custom(animation) => Some(animation),
        }
    }
}

impl std::fmt::Debug for Animation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::None => "None",
            Self::Fade => "Fade",
            Self::Zoom => "Zoom",
            Self::Slide => "Slide",
            Self::ZoomFade => "ZoomFade",
            Self::Custom(_) => "Custom",
        })
    }
}

/// The names a **description** may use. `Custom` carries a live object, so it has no name — which
/// is exactly why this is derived from the variants rather than written out: the vocabulary cannot
/// fall behind the type, and the one entry that cannot travel as data is skipped by construction.
impl crate::PropName for Animation {
    const VARIANT_NAMES: &'static [&'static str] = &["none", "fade", "zoom", "slide", "zoom_fade"];

    fn from_prop_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "fade" => Some(Self::Fade),
            "zoom" => Some(Self::Zoom),
            "slide" => Some(Self::Slide),
            "zoom_fade" => Some(Self::ZoomFade),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::AnimationFrame;
    use super::*;
    use crate::PropName;

    /// One word gets the gesture; the tuners refine it without a composition at the call site.
    #[test]
    fn a_named_animation_builds_the_gesture_it_says() {
        for named in [Animation::Fade, Animation::Zoom, Animation::Slide, Animation::ZoomFade] {
            let label = format!("{named:?}");
            let mut a = named.build().expect("a named gesture");
            a.leave();
            assert!(a.is_leaving(), "{label} holds the surface while it goes");
            let mut frames = 0;
            while a.tick(1.0 / 60.0) && frames < 600 {
                frames += 1;
            }
            assert!(!a.is_leaving(), "{label} lets go at the end");
        }

        assert!(Animation::None.build().is_none(), "no animation is an absence, not an object");

        let mut tuned = Animation::Zoom.from(0.5).build().expect("a zoom");
        tuned.enter();
        assert!((tuned.frame().scale - 0.5).abs() < 0.01, "it starts where it was told");
    }

    /// **A description names the same animations**, and an unknown name is not a panic.
    #[test]
    fn the_variants_are_the_vocabulary_a_description_writes() {
        assert!(matches!(Animation::from_prop_name("zoom_fade"), Some(Animation::ZoomFade)));
        assert!(matches!(Animation::from_prop_name("fade"), Some(Animation::Fade)));
        assert!(matches!(Animation::from_prop_name("slide"), Some(Animation::Slide)));
        assert!(Animation::from_prop_name("whirl").is_none(), "unknown ⇒ the default");
        assert!(
            !Animation::VARIANT_NAMES.contains(&"custom"),
            "a live object has no name a description could write",
        );
    }

    /// An animation from outside this crate is passed exactly where a built-in is.
    #[test]
    fn an_animation_of_your_own_goes_where_a_built_in_goes() {
        struct Whirl(bool);
        impl Animate for Whirl {
            fn enter(&mut self) {}
            fn leave(&mut self) {
                self.0 = true;
            }
            fn cancel(&mut self) {
                self.0 = false;
            }
            fn tick(&mut self, _dt: f32) -> bool {
                false
            }
            fn is_leaving(&self) -> bool {
                self.0
            }
            fn frame(&self) -> AnimationFrame {
                AnimationFrame::IDENTITY
            }
        }

        let mut mine = Animation::of(Whirl(false)).build().expect("mine");
        mine.leave();
        assert!(mine.is_leaving(), "the surface waits for a gesture the library never heard of");
    }
}
