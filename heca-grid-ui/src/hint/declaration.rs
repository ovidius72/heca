//! **What a pick does, and what it *is*** — the declaration a widget carries.
//!
//! One file, one thing: the value. Collecting them is [`super::collect`], running one is
//! [`super::fire`], and handing out letters is [`super::offer`].

/// **What a pick does to a region, and — when the declaring code could name it — what that *is*.**
///
/// The closure is the whole of the behaviour: this side of the boundary is native, so a pick runs a
/// function. The [`intent`](Hint::intent) beside it is the same act said as **data**, and it exists
/// for one reason: a host cannot ask its own policy about an opaque closure. With it,
/// `chrome::active_hint_targets` asks `route_interaction` per candidate and drops every refused one
/// — one rule, every surface, no special case per widget (F003/P082/T432).
///
/// The case that forced it: with a floating pane active, `prefix+/` lettered every pane and every
/// sidebar row naming one, and pressing a letter did **nothing**, because `ActionPolicy` correctly
/// refuses a `FocusPane` that does not target the active float. A picker offering letters that do
/// nothing is a broken picker.
///
/// **The library stores it and never reads it.** Which targets are excluded is the app's judgement
/// (`docs/hint-architecture.md` § 6); the framework's half is carrying the declaration, exactly as
/// it carries the closure.
///
/// Both authoring paths converge here — one door, so there is no second path to drift:
///
/// ```ignore
/// // native: the app's `fires` already builds the closure FROM the intent, so it hands over both
/// KeyHint::new(row).on_hint(fires(mount, intent, emit))
///
/// // declarative: `realize` writes the node's `hint` event (defaulting to its `press`) into here
/// Row::new().on_hint(Intent::new("docker.reveal").arg("id", id))
///
/// // and a plain closure is still a hint — it simply cannot be judged, so it is always offered
/// KeyHint::new(row).on_hint(move || choose(id))
/// ```
pub struct Hint {
    /// What picking this **would do**, as data — `None` when the declaring code only had a closure.
    ///
    /// `None` means *"nothing to ask about"*, never *"refused"*: a declaration a host cannot judge
    /// is offered a letter as it always was. Silently withholding letters from everything a policy
    /// cannot see would be a worse picker than the one this fixes.
    pub intent: Option<heca_view::Intent>,
    run: Box<dyn Fn()>,
}

impl Hint {
    /// A hint that runs `f` and says what it is.
    pub fn of(intent: heca_view::Intent, f: impl Fn() + 'static) -> Self {
        Self {
            intent: Some(intent),
            run: Box::new(f),
        }
    }

    /// Run it. The only thing the library ever does with a declaration.
    pub fn run(&self) {
        (self.run)()
    }
}

impl std::fmt::Debug for Hint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hint")
            .field("intent", &self.intent)
            .finish()
    }
}

/// A bare closure is a hint with nothing to say about itself — what every native call site wrote
/// before there was anything to ask, and what one still writes when the act has no name.
impl<F: Fn() + 'static> From<F> for Hint {
    fn from(f: F) -> Self {
        Self {
            intent: None,
            run: Box::new(f),
        }
    }
}
