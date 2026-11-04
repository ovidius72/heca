use super::*;
use std::{sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::Duration};
use crate::{Color, Component, DrawCommand, FlashExt, LayoutEngine, Scene, Size, Theme};

struct CustomWidget { base: Base }

impl Component for CustomWidget {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn tick(&mut self, _dt: f32) -> bool { false }
}

fn widget(handle: FlashHandle) -> CustomWidget {
    let mut widget = CustomWidget { base: Base::new() }.flash(handle);
    widget.base.style.layout.width = crate::style::Length::Px(120.0);
    widget.base.style.layout.height = crate::style::Length::Px(40.0);
    LayoutEngine::new().compute(&mut widget, Size::new(120.0, 40.0));
    widget
}

fn scene(widget: &impl Component, theme: &Theme) -> Scene {
    let mut scene = Scene::new();
    crate::paint_child(widget, &mut PaintCx::new(&mut scene, theme));
    scene
}

fn borders(scene: &Scene) -> Vec<Color> {
    scene.iter().filter_map(|cmd| match cmd {
        DrawCommand::Rect(rect) => rect.border.map(|border| border.color), _ => None,
    }).collect()
}

#[test]
fn pulses_have_a_dark_interval_and_finite_bursts_stop() {
    let start = Instant::now();
    let request = Request { started: start, tone: Tone::Success, playback: Playback::Burst };
    let style = heca_theme::FlashStyle { duration_s: 1.0, pause_s: 0.5, pulses: 2, ..Default::default() };
    assert_eq!(amount(request, style, start), Some(1.0));
    assert_eq!(amount(request, style, start + Duration::from_millis(500)), Some(0.5));
    assert_eq!(amount(request, style, start + Duration::from_millis(1250)), Some(0.0));
    assert_eq!(amount(request, style, start + Duration::from_millis(1500)), Some(1.0));
    assert_eq!(amount(request, style, start + Duration::from_secs(3)), None);
    let repeating = Request { playback: Playback::Repeat { duration: None }, ..request };
    assert_eq!(amount(repeating, style, start + Duration::from_millis(300500)), Some(1.0));
    for duration in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(amount(request, heca_theme::FlashStyle { duration_s: duration, ..style }, start), None);
    }
    for pause in [-1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(amount(request, heca_theme::FlashStyle { pause_s: pause, ..style }, start), None);
    }
    assert_eq!(amount(request, heca_theme::FlashStyle { enabled: false, ..style }, start), None);
}

#[test]
fn a_custom_widget_needs_no_animation_tick_or_paint_forwarding() {
    let frames = Arc::new(AtomicUsize::new(0));
    let requested = frames.clone();
    crate::install_frame_request(move || { requested.fetch_add(1, Ordering::Relaxed); });
    let handle = FlashHandle::new();
    let mut widget = widget(handle);
    let theme = Theme::default();
    assert!(borders(&scene(&widget, &theme)).is_empty());
    handle.trigger(Tone::Success);
    assert!(!widget.tick(0.0), "the custom widget owns no flash animation");
    let painted = scene(&widget, &theme);
    assert_eq!(borders(&painted)[0].with_alpha(255), theme.colors.success.with_alpha(255));
    assert!(painted.iter().any(|cmd| matches!(cmd, DrawCommand::Rect(rect)
        if rect.fill.a > 0 && rect.fill.a <= theme.colors.flash.fill_alpha)), "soft light follows the theme");
    assert!(widget.next_redraw().is_some(), "pulses use the widget deadline path");
    let before = frames.load(Ordering::Relaxed);
    crate::request_frame();
    assert_eq!(frames.load(Ordering::Relaxed), before + 1);
    crate::request_frame();
    assert_eq!(frames.load(Ordering::Relaxed), before + 1, "pending frame requests coalesce");
    scene(&widget, &theme);
    crate::request_frame();
    assert_eq!(frames.load(Ordering::Relaxed), before + 2, "painting automatically re-arms the next frame");
    handle.request.set(Some(Request { tone: Tone::Success, playback: Playback::Burst,
        started: Instant::now() - Duration::from_secs_f32(
            (theme.colors.flash.duration_s + theme.colors.flash.pause_s) * theme.colors.flash.pulses as f32 + 1.0) }));
    let before = frames.load(Ordering::Relaxed);
    assert!(borders(&scene(&widget, &theme)).is_empty());
    assert_eq!(frames.load(Ordering::Relaxed), before, "a completed flash stops requesting frames");
    assert_eq!(widget.next_redraw(), None);
}

#[test]
fn one_pulse_stops_after_the_themed_duration_even_when_bursts_have_many_pulses() {
    let handle = FlashHandle::new();
    handle.pulse(Tone::Warning);
    let request = handle.request.get_untracked().unwrap();
    let style = heca_theme::FlashStyle { duration_s: 1.0, pause_s: 0.5, pulses: 8, ..Default::default() };
    assert_eq!(amount(request, style, request.started), Some(1.0));
    assert_eq!(amount(request, style, request.started + Duration::from_millis(500)), Some(0.5));
    assert_eq!(amount(request, style, request.started + Duration::from_secs(1)), None);
    assert_eq!(amount(request, style, request.started + Duration::from_secs(3)), None);
}

#[test]
fn repeating_flash_sleeps_through_the_dark_interval() {
    let frames = Arc::new(AtomicUsize::new(0));
    let requested = frames.clone();
    crate::install_frame_request(move || { requested.fetch_add(1, Ordering::Relaxed); });
    let handle = FlashHandle::new();
    let widget = widget(handle);
    let mut theme = Theme::default();
    theme.colors.flash.duration_s = 1.0;
    theme.colors.flash.pause_s = 10.0;
    handle.repeat(Tone::Warning);
    handle.request.update(|request| request.as_mut().unwrap().started -= Duration::from_secs(2));
    let before = frames.load(Ordering::Relaxed);
    assert!(borders(&scene(&widget, &theme)).is_empty());
    assert_eq!(frames.load(Ordering::Relaxed), before, "paint must not start a busy redraw loop");
    assert!(widget.next_redraw().unwrap() > 8.0, "the dark phase sleeps until the next pulse");
    handle.request.update(|request| request.as_mut().unwrap().started = Instant::now() - Duration::from_millis(500));
    assert!(!borders(&scene(&widget, &theme)).is_empty());
    handle.clear();
    let before = frames.load(Ordering::Relaxed);
    assert!(borders(&scene(&widget, &theme)).is_empty());
    assert_eq!(frames.load(Ordering::Relaxed), before);
    assert_eq!(widget.next_redraw(), None);
}

#[test]
fn repeating_pulses_rise_and_fall_softly_and_obey_the_total_duration() {
    let handle = FlashHandle::new();
    handle.repeat_for(Tone::Warning, Duration::from_millis(3500));
    let request = handle.request.get_untracked().unwrap();
    let style = heca_theme::FlashStyle { duration_s: 2.0, pause_s: 1.0, ..Default::default() };
    assert_eq!(amount(request, style, request.started), Some(0.0));
    assert!((amount(request, style, request.started + Duration::from_millis(500)).unwrap() - 0.5).abs() < 0.001);
    assert_eq!(amount(request, style, request.started + Duration::from_secs(1)), Some(1.0));
    assert!((amount(request, style, request.started + Duration::from_millis(1500)).unwrap() - 0.5).abs() < 0.001);
    assert_eq!(amount(request, style, request.started + Duration::from_millis(2500)), Some(0.0));
    assert_eq!(amount(request, style, request.started + Duration::from_millis(3500)), None);
}

#[test]
fn a_timed_repeat_expires_even_during_the_dark_pause() {
    let handle = FlashHandle::new();
    let widget = widget(handle);
    let mut theme = Theme::default();
    theme.colors.flash.duration_s = 1.0;
    theme.colors.flash.pause_s = 10.0;
    handle.repeat_for(Tone::Warning, Duration::from_millis(2500));
    handle.request.update(|request| request.as_mut().unwrap().started -= Duration::from_secs(2));
    scene(&widget, &theme);
    assert!(widget.next_redraw().unwrap() <= 0.5, "timeout wins over the next pulse");
    handle.request.update(|request| request.as_mut().unwrap().started -= Duration::from_secs(1));
    assert!(borders(&scene(&widget, &theme)).is_empty());
    assert_eq!(widget.next_redraw(), None);
}

#[test]
fn updating_a_repeat_timeout_preserves_its_original_start() {
    let handle = FlashHandle::new();
    handle.repeat(Tone::Warning);
    let started = handle.request.get_untracked().unwrap().started;
    handle.set_repeat_duration(Some(Duration::from_secs(3)));
    let timed = handle.request.get_untracked().unwrap();
    assert_eq!(timed.started, started);
    assert_eq!(amount(timed, Default::default(), started + Duration::from_secs(4)), None);
    handle.set_repeat_duration(None);
    let endless = handle.request.get_untracked().unwrap();
    assert_eq!(endless.started, started);
    assert!(amount(endless, Default::default(), started + Duration::from_secs(400)).is_some());
}

#[test]
fn row_and_flash_share_the_configured_radius_without_clipping_the_outer_stroke() {
    use crate::builders::{LayoutExt, StyleExt};
    for radius in [0.0, 4.0, 12.0, 24.0] {
        let flash = FlashHandle::new();
        let mut row = crate::widgets::Row::new().flash(flash).radius(radius)
            .background(Color::rgb(20, 30, 40)).width(120.0).height(60.0);
        LayoutEngine::new().compute(&mut row, Size::new(120.0, 60.0));
        flash.trigger(Tone::Danger);
        let mut painted = Scene::new();
        let theme = Theme::default();
        let parent_clip = crate::Rectangle::new(crate::Point::new(-10.0, -10.0), Size::new(140.0, 80.0));
        PaintCx::new(&mut painted, &theme).with_clip(parent_clip, |cx| crate::paint_child(&row, cx));
        let rectangles = painted.iter().filter_map(|command| match command {
            DrawCommand::Rect(rectangle) => Some(rectangle), _ => None,
        }).collect::<Vec<_>>();
        let surface = rectangles.first().unwrap();
        let overlay = rectangles.last().unwrap();
        assert_eq!(overlay.rect, surface.rect);
        assert_eq!(overlay.radius, surface.radius);
        assert_eq!(overlay.radius, radius);
        assert!(overlay.border.is_some());
        let clips = painted.iter().filter_map(|command| match command {
            DrawCommand::PushClip(rectangle) => Some(*rectangle), _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(clips, vec![parent_clip], "the outer stroke keeps only the inherited clip");
    }
}

#[test]
fn shared_handles_follow_the_current_theme_and_clear_every_target() {
    let handle = FlashHandle::new();
    let first = widget(handle);
    let second = widget(handle);
    handle.trigger(Tone::Danger);
    let mut theme = Theme::default();
    theme.colors.danger = Color::rgb(220, 30, 70);
    for target in [&first, &second] {
        assert_eq!(borders(&scene(target, &theme))[0].with_alpha(255), theme.colors.danger.with_alpha(255));
    }
    theme.colors.flash.enabled = false;
    assert!(borders(&scene(&first, &theme)).is_empty());
    handle.clear();
    assert!(borders(&scene(&second, &Theme::default())).is_empty());
}
