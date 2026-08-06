//! [`Toast`] — a compact **notification card**: a bracket-framed surface with a
//! severity-colored leading [`Icon`](super::Icon), a strong title, optional small
//! body text, an optional inline [`Button`](super::Button), and an optional
//! [`IconButton`](super::IconButton) dismiss affordance.
//!
//! This is a **presentation widget only** — it holds *no* queue, timer, or global
//! state. Lifecycle (when a toast appears, how long it lives, auto-dismiss policy,
//! deduplication, sound) is the **host application's** job; the widget merely
//! renders one notification and reports interactions back via callbacks.
//!
//! The card paints only its own chrome. Every piece of content is a real child in
//! [`Base::children`](crate::component::Base::children), so layout, focus, event
//! routing, hints, and accessibility all traverse the same retained tree. Severity
//! maps to theme tokens (`accent`/`success`/`warning`/`danger`), never literals.

use crate::builders::{HintExt, LayoutExt, Parent};
use crate::component::{paint_child, Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::hint::HintTargetId;
use crate::reactive::SignalGet;
use crate::style::{Align, Direction, Length, Spacing, WidgetSize};
use crate::widgets::{Button, Ellipsis, Flex, Glyph, Icon, IconButton, Label};
use heca_core::layout::Point;
use std::rc::Rc;

/// Stable top-level slots. Optional content uses a zero-sized [`Flex::empty`]
/// placeholder so painting and tests never depend on a changing child order.
const LEADING: usize = 0;
const CONTENT: usize = 1;
const DISMISS: usize = 2;

/// Severity of a [`Toast`], mapped to theme tokens at paint time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum ToastSeverity {
    /// Informational (accent).
    #[default]
    Info,
    /// Success (positive).
    Success,
    /// Warning.
    Warning,
    /// Danger (error).
    Danger,
}

#[heca_grid_ui_macros::props]
impl ToastSeverity {
    /// The default leading glyph for this severity (overridable via [`Toast::icon`]).
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    fn default_glyph(self) -> Glyph {
        match self {
            ToastSeverity::Info => Glyph::Info,
            ToastSeverity::Success => Glyph::Check,
            ToastSeverity::Warning => Glyph::Warning,
            ToastSeverity::Danger => Glyph::WarningCircle,
        }
    }
}

/// A compact notification card. Presentation only — the host owns lifecycle.
pub struct Toast {
    base: Base,
    severity: ToastSeverity,
    /// Leading glyph; defaults to the severity glyph, `None` means no override.
    icon: Option<Glyph>,
    show_icon: bool,
    title: String,
    body: Option<String>,
    action_label: Option<String>,
    /// Whether the dismiss affordance is shown (default `true`).
    dismissible: bool,
    on_click: Option<Rc<dyn Fn()>>,
    on_action: Option<Rc<dyn Fn()>>,
    on_dismiss: Option<Rc<dyn Fn()>>,
    /// Opaque runtime ids assigned by the host and carried by the real controls.
    action_target: Option<HintTargetId>,
    dismiss_target: Option<HintTargetId>,
    /// Whole-card press feedback. Action and dismiss own their own flashes.
    flash: Flash,
}

#[heca_grid_ui_macros::props]
impl Toast {
    /// A new info toast showing `title`. Add body text with [`body`](Toast::body),
    /// an action with [`action`](Toast::action), severity via the convenience
    /// constructors, and wire dismissal with [`on_dismiss`](Toast::on_dismiss).
    pub fn new(title: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Start;
        base.style.layout.pad_spacing_x = Some(Spacing::Md);
        base.style.layout.pad_spacing_y = Some(Spacing::Md);
        base.style.layout.gap_spacing = Some(Spacing::Sm);

        let mut toast = Self {
            base,
            severity: ToastSeverity::Info,
            icon: None,
            show_icon: true,
            title: title.into(),
            body: None,
            action_label: None,
            dismissible: true,
            on_click: None,
            on_action: None,
            on_dismiss: None,
            action_target: None,
            dismiss_target: None,
            flash: Flash::new(),
        };
        toast.rebuild_content();
        toast
    }

    /// Convenience constructors, one per severity.
    pub fn info(title: impl Into<String>) -> Self {
        Self::new(title)
    }

    pub fn success(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Success)
    }

    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Warning)
    }

    pub fn danger(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Danger)
    }

    /// Set the severity (hue + default leading glyph).
    #[heca_grid_ui_macros::prop]
    pub fn severity(mut self, severity: ToastSeverity) -> Self {
        self.severity = severity;
        self.rebuild_content();
        self
    }

    /// Override the leading glyph (default: the severity glyph).
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self.show_icon = true;
        self.rebuild_content();
        self
    }

    /// Hide the leading icon entirely.
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub fn no_icon(mut self) -> Self {
        self.show_icon = false;
        self.rebuild_content();
        self
    }

    /// Set the small body text (a second line under the title).
    #[heca_grid_ui_macros::prop]
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self.rebuild_content();
        self
    }

    /// Add an inline action button with `label` + callback.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn action(mut self, label: impl Into<String>, f: impl Fn() + 'static) -> Self {
        self.action_label = Some(label.into());
        self.on_action = Some(Rc::new(f));
        self.rebuild_content();
        self
    }

    /// Attach the host's opaque runtime target to the inline action. The id is
    /// ignored while no action exists, so it can never create a ghost target.
    #[heca_grid_ui_macros::host_only("runtime registry ids are host-owned, not declarative properties")]
    pub fn action_target(mut self, id: HintTargetId) -> Self {
        self.action_target = Some(id);
        self.rebuild_content();
        self
    }

    /// Whether the dismiss affordance is shown (default `true`).
    #[heca_grid_ui_macros::prop]
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self.rebuild_content();
        self
    }

    /// Attach the host's opaque runtime target to the dismiss control. The id is
    /// ignored when dismissal is hidden, so `dismissible(false)` removes both.
    #[heca_grid_ui_macros::host_only("runtime registry ids are host-owned, not declarative properties")]
    pub fn dismiss_target(mut self, id: HintTargetId) -> Self {
        self.dismiss_target = Some(id);
        self.rebuild_content();
        self
    }

    /// Make the whole card clickable. The action and dismiss children receive
    /// pointer input first and therefore never also trigger this callback.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self.base.focusable = true;
        self
    }

    /// Set the callback fired when dismiss is activated. The host removes the toast.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self.rebuild_content();
        self
    }

    /// Carry transient control state across a keyed [`ToastStack`](super::ToastStack)
    /// refresh. Content is rebuilt from the new spec, but a semantically unchanged
    /// action/dismiss control is retained wholesale (hover, flash, and focus included).
    /// When its label or target changed, only keyboard-focus identity survives.
    pub(crate) fn preserve_runtime_from(&mut self, mut old: Toast) {
        self.base.bounds = old.base.bounds;
        self.base.font = old.base.font;
        self.base.focused = old.base.focused;
        self.base.focus_visible = old.base.focus_visible;
        self.flash = old.flash;

        let new_action = self
            .action_label
            .as_ref()
            .map(|_| 1 + usize::from(self.body.is_some()));
        let old_action = old
            .action_label
            .as_ref()
            .map(|_| 1 + usize::from(old.body.is_some()));
        if let (Some(new_idx), Some(old_idx)) = (new_action, old_action) {
            let same_control = self.action_label == old.action_label
                && self.action_target == old.action_target;
            if same_control {
                let new_children = &mut self.base.children[CONTENT].base_mut().children;
                let old_children = &mut old.base.children[CONTENT].base_mut().children;
                std::mem::swap(&mut new_children[new_idx], &mut old_children[old_idx]);
            } else {
                let old_base = old.base.children[CONTENT].base().children[old_idx].base();
                let focused = old_base.focused;
                let focus_visible = old_base.focus_visible;
                let new_base = self.base.children[CONTENT].base_mut().children[new_idx].base_mut();
                new_base.focused = focused;
                new_base.focus_visible = focus_visible;
            }
        }

        if self.dismissible && old.dismissible {
            let same_control = self.dismiss_target == old.dismiss_target;
            if same_control {
                std::mem::swap(
                    &mut self.base.children[DISMISS],
                    &mut old.base.children[DISMISS],
                );
            } else {
                let old_base = old.base.children[DISMISS].base();
                let focused = old_base.focused;
                let focus_visible = old_base.focus_visible;
                let new_base = self.base.children[DISMISS].base_mut();
                new_base.focused = focused;
                new_base.focus_visible = focus_visible;
            }
        }
    }

    /// Rebuild the retained content tree while builder calls are assembling the
    /// widget. Runtime state lives in the child controls after construction; no
    /// per-frame rebuild occurs.
    fn rebuild_content(&mut self) {
        let disabled = self.base.disabled;

        let leading: Box<dyn Component> = if self.show_icon {
            Box::new(Icon::new(self.icon.unwrap_or(self.severity.default_glyph())))
        } else {
            Box::new(Flex::empty())
        };

        let mut content = Flex::column()
            .gap_spacing(Spacing::Xs)
            .grow(1.0)
            .align(Align::Start);
        // A fixed-width host such as ToastStack must be allowed to make the
        // text column narrower than its natural label width.
        content.base_mut().style.layout.min_width = Some(Length::Px(0.0));
        content = content.child(Label::new(self.title.clone()).bold(true).truncate(Ellipsis::End));
        if let Some(body) = &self.body {
            content = content.child(
                Label::new(body.clone())
                    .muted(true)
                    .truncate(Ellipsis::End)
                    .size(WidgetSize::Small),
            );
        }
        if let Some(label) = &self.action_label {
            let callback = self.on_action.clone();
            let mut action = Button::ghost(label.clone())
                .size(WidgetSize::Small)
                .align_self(Align::Start)
                .on_click(move || {
                    if let Some(f) = &callback {
                        f();
                    }
                });
            if let Some(id) = self.action_target {
                action = action.hint_target(id);
            }
            // `.disabled(true)` on the Toast and the composed controls share
            // one signal, so focus and input state cannot diverge.
            action.base_mut().disabled = disabled;
            content = content.child(action);
        }

        let dismiss: Box<dyn Component> = if self.dismissible {
            let callback = self.on_dismiss.clone();
            let mut button = IconButton::new(Icon::new(Glyph::Close))
                .size(WidgetSize::Small)
                .on_click(move || {
                    if let Some(f) = &callback {
                        f();
                    }
                });
            if let Some(id) = self.dismiss_target {
                button = button.hint_target(id);
            }
            button.base_mut().disabled = disabled;
            Box::new(button)
        } else {
            Box::new(Flex::empty())
        };

        self.base.children = vec![leading, Box::new(content), dismiss];
        debug_assert_eq!(self.base.children.len(), 3, "Toast children: [leading, content, dismiss]");
    }

    fn activate_card(&mut self) {
        self.flash.trigger();
        if let Some(f) = &self.on_click {
            f();
        }
    }

    fn contains(&self, point: Point) -> bool {
        self.base.bounds.contains(point)
    }
}

impl Component for Toast {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Paint only the card chrome, then let the real children paint their own
    /// content and interaction feedback.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let (surface, card_radius, toast_tint) = {
            let t = cx.theme();
            (t.colors.surface, t.colors.border_radius, t.colors.interaction.toast_tint)
        };
        let tone = match self.severity {
            ToastSeverity::Info => cx.theme().colors.accent,
            ToastSeverity::Success => cx.theme().colors.success,
            ToastSeverity::Warning => cx.theme().colors.warning,
            ToastSeverity::Danger => cx.theme().colors.danger,
        };
        let bounds = self.base.bounds;

        // The rest halo radius follows the resolved font; its presence and
        // strength still come from the theme's glow_size/rest-glow tokens.
        let glow = cx.rest_glow(self.base.font);
        cx.rect(
            bounds,
            surface.lerp(tone, toast_tint as f32 / 255.0),
            None,
            card_radius,
            glow,
        );
        cx.bracket_frame(bounds);

        // Severity colors the leading icon and title. The body opts into the
        // theme muted token; the action button owns its own control state.
        cx.with_content_color(tone, |cx| {
            paint_child(self.base.children[LEADING].as_ref(), cx);
            paint_child(self.base.children[CONTENT].as_ref(), cx);
        });
        paint_child(self.base.children[DISMISS].as_ref(), cx);

        cx.flash(bounds, self.flash.amount() * 0.4, card_radius);
        if self.base.disabled.get_untracked() {
            cx.dim(bounds, card_radius);
        }
        if self.focusable() && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            cx.focus_ring(bounds, cx.theme().colors.focus_ring_tone(tone), card_radius);
        }
    }

    /// A key delivered to the whole-card target activates it before descendant
    /// controls see it. FocusManager delivers keys aimed at an action/dismiss
    /// directly to that child, so the three targets remain independent.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() || self.on_click.is_none() {
            return Handled::No;
        }
        match ev {
            Event::Key { key: GridKey::Enter | GridKey::Space, pressed: true } => {
                self.activate_card();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// Children receive pointer events first. A press none of them handled may
    /// activate the card itself; an inert card passes the press through.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() || self.on_click.is_none() {
            return Handled::No;
        }
        match ev {
            Event::PointerPressed { pos } if self.contains(*pos) => {
                self.activate_card();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = self.flash.tick(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for Toast {}
