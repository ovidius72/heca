use super::*;

#[test]
fn capturing_another_clients_view_never_moves_the_live_view() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 12);
    let before = engine.snapshot((8.0, 14.0));
    let history = engine.snapshot_at((8.0, 14.0), 3);
    let after = engine.snapshot((8.0, 14.0));
    assert_eq!(history.viewport_offset, 3);
    assert_eq!(history.viewport_top_stable_row, before.viewport_top_stable_row - 3);
    assert_eq!(after.to_wire(), before.to_wire());
    assert_eq!(engine.viewport_offset(), 0);
    assert!(!engine.take_viewport_changed());
}

#[test]
fn boot_resize_keeps_the_prompt_on_the_first_row() {
    // Boot at the spawn grid (80x24), let the shell print its two prompt lines,
    // then resize to the pane's real grid — the boot sequence every pane runs
    // when its first layout reports. The engine keeps the prompt on the first
    // visible row: a blank top row on screen is the shell's own output (its
    // prompt-redraw newline), not a viewport or reflow shift.
    let mut engine = viewport_engine(80, 24, 3500);
    engine.advance_bytes(b"~ via v25.4.0\r\n");
    engine.advance_bytes(b"\xe2\x80\xba . ");
    engine.resize(37, 40);

    let snap = engine.snapshot((8.0, 17.0));
    assert_eq!(snap.rows, 40);
    let first = &snap.lines[0];
    let has_glyphs = first
        .cells
        .iter()
        .any(|c| c.text.trim() != "" || c.bg[3] > 0.0);
    eprintln!(
        "row0 {:?}",
        first
            .cells
            .iter()
            .map(|c| c.text.clone())
            .collect::<String>()
    );
    assert!(has_glyphs, "first visible row lost the prompt");
}

#[test]
fn viewport_starts_pinned_to_bottom_with_zero_offset() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 10);

    assert_eq!(
        engine.viewport_offset(),
        0,
        "fresh engine pins to live bottom"
    );
    assert!(engine.at_bottom(), "at_bottom is true at offset 0");
    assert!(
        !engine.take_viewport_changed(),
        "no motion yet ⇒ no viewport damage"
    );

    let snapshot = engine.snapshot((8.0, 14.0));
    assert_eq!(snapshot.viewport_offset, 0);
    assert!(snapshot.at_bottom);
    assert!(
        snapshot.scrollback_rows > 4,
        "scrollback_rows should retain history above the visible rows"
    );
    assert_eq!(
        snapshot.scrollback_rows.saturating_sub(4),
        max_offset(&engine),
        "snapshot scrollback_rows minus visible equals the max offset"
    );
}

#[test]
fn scroll_viewport_clamps_to_max_offset_and_sets_damage_flag() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 10);
    let max_offset = max_offset(&engine);
    assert!(
        max_offset > 0,
        "fixture should produce some scrollback history"
    );

    engine.scroll_viewport(3);
    assert_eq!(engine.viewport_offset(), 3);
    assert!(!engine.at_bottom());
    assert!(
        engine.take_viewport_changed(),
        "motion sets the viewport-changed flag"
    );
    assert!(!engine.take_viewport_changed(), "flag drains once");

    // Overscroll clamps to max_offset.
    engine.scroll_viewport(100);
    assert_eq!(engine.viewport_offset(), max_offset);
    assert!(engine.take_viewport_changed());

    // Scrolling back down clamps at bottom (offset 0).
    engine.scroll_viewport(-100);
    assert_eq!(engine.viewport_offset(), 0);
    assert!(engine.at_bottom());
    assert!(engine.take_viewport_changed());

    // No-op scrolls (already at boundary) do not arm damage.
    engine.scroll_viewport(-1);
    assert_eq!(engine.viewport_offset(), 0);
    assert!(
        !engine.take_viewport_changed(),
        "clamped no-op must not arm damage"
    );
}

#[test]
fn scroll_to_top_and_bottom_jump_the_viewport() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 10);
    let max_offset = max_offset(&engine);
    assert!(max_offset > 0);

    engine.scroll_to_top();
    assert_eq!(engine.viewport_offset(), max_offset);
    assert!(engine.take_viewport_changed());

    engine.scroll_to_top();
    assert!(
        !engine.take_viewport_changed(),
        "already-at-top no-op must not arm damage"
    );

    engine.scroll_to_bottom();
    assert_eq!(engine.viewport_offset(), 0);
    assert!(engine.at_bottom());
    assert!(engine.take_viewport_changed());

    engine.scroll_to_bottom();
    assert!(
        !engine.take_viewport_changed(),
        "already-at-bottom no-op must not arm damage"
    );
}

#[test]
fn animated_scroll_settles_at_target() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 20);
    let max = max_offset(&engine);
    let target = 5.min(max);
    assert!(target > 0, "fixture must have enough scrollback");
    // 0ms duration ⇒ animation completes on the first advance (done path),
    // with no real-time sleep.
    engine.set_viewport_anim_config_for_test(AnimationConfig {
        duration_ms: 0,
        easing: crate::layout::animation::linear,
    });

    engine.scroll_viewport_animated(target as i32);
    // Single advance settles: the animation reports done and snaps to target.
    assert!(
        !engine.advance_animation(),
        "0ms animation completes on first advance"
    );
    assert_eq!(engine.viewport_offset(), target, "offset settles at target");
    assert!(
        !engine.advance_animation(),
        "no animation left after settle"
    );
}

#[test]
fn animated_scroll_re_targets_on_new_jump_without_queueing() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 20);
    let max = max_offset(&engine);
    let expected = 10.min(max);
    assert!(expected >= 5, "fixture must have enough scrollback");
    engine.set_viewport_anim_config_for_test(AnimationConfig {
        duration_ms: 0,
        easing: crate::layout::animation::linear,
    });

    // Jump up 5, then immediately jump up 5 more → should target 10 (re-target,
    // not 5). The second call bases its delta on the first animation's target.
    engine.scroll_viewport_animated(5);
    engine.scroll_viewport_animated(5);
    assert!(
        !engine.advance_animation(),
        "0ms animation completes on first advance"
    );
    assert_eq!(
        engine.viewport_offset(),
        expected,
        "re-targeted jump lands at 10, not 5"
    );
}

#[test]
fn disabled_animation_degrades_to_immediate_scroll() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 20);
    let max = max_offset(&engine);
    let target = 5.min(max);
    assert!(target > 0, "fixture must have enough scrollback");
    engine.set_scroll_animations_enabled(false);

    engine.scroll_viewport_animated(target as i32);
    assert_eq!(
        engine.viewport_offset(),
        target,
        "disabled animation jumps immediately"
    );
    assert!(
        !engine.advance_animation(),
        "no animation left when disabled"
    );
}

#[test]
fn wheel_scroll_does_not_animate_and_clears_ongoing_animation() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 20);
    let max = max_offset(&engine);
    assert!(max >= 10, "fixture must have enough scrollback");
    // A long duration keeps the animation "ongoing" without any sleep, so we
    // can observe the wheel clearing it.
    engine.set_viewport_anim_config_for_test(AnimationConfig {
        duration_ms: 60_000,
        easing: crate::layout::animation::linear,
    });

    // Start an animated jump; it is still running (long duration).
    engine.scroll_viewport_animated(10);
    assert!(
        engine.advance_animation(),
        "long animation is still running"
    );
    // Wheel scroll (immediate path) clears the animation and applies at once.
    engine.scroll_viewport(3);
    assert_eq!(engine.viewport_offset(), 3, "wheel applied immediately");
    assert!(!engine.advance_animation(), "wheel cleared the animation");
}

#[test]
fn snapshot_projects_history_rows_when_viewport_is_scrolled() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 10);
    // Scrolling up 4 rows should reveal history rows instead of the live bottom.
    engine.scroll_viewport(4);
    let snapshot = engine.snapshot((8.0, 14.0));
    assert_eq!(snapshot.viewport_offset, 4);
    assert!(!snapshot.at_bottom);
    assert_eq!(
        snapshot.lines.len(),
        4,
        "snapshot still reports exactly `rows` lines"
    );
    snapshot.debug_assert_valid();
}

/// **The cursor stays on the prompt when you scroll back**. It moves down with its text, and is
/// hidden once the prompt has scrolled out of view — never left behind on old output.
#[test]
fn the_cursor_moves_with_its_prompt_when_the_view_is_scrolled_back() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 10);
    let live = engine.snapshot((8.0, 14.0)).cursor;
    assert!(live.visible);

    engine.scroll_viewport(1);
    let scrolled = engine.snapshot((8.0, 14.0)).cursor;
    if live.row + 1 < 4 {
        assert!(scrolled.visible);
        assert_eq!(
            scrolled.row,
            live.row + 1,
            "one row back, the prompt is one row lower"
        );
    } else {
        assert!(
            !scrolled.visible,
            "the prompt left the view, so does the cursor"
        );
    }

    engine.scroll_viewport(3);
    assert!(
        !engine.snapshot((8.0, 14.0)).cursor.visible,
        "scrolled a whole view back, the prompt is off screen"
    );
}

#[test]
fn resize_re_clamps_viewport_offset_when_scrollback_shrinks() {
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 10);
    let total = engine.snapshot((8.0, 14.0)).scrollback_rows;
    engine.scroll_to_top();
    assert_eq!(engine.viewport_offset(), total.saturating_sub(4));
    let _ = engine.take_viewport_changed();

    // Grow visible rows so max_offset shrinks; the stored offset must clamp.
    engine.resize(20, 8);
    assert_eq!(
        engine.viewport_offset(),
        total.saturating_sub(8),
        "resize must re-clamp offset to new max (total - visible 8)"
    );
    assert!(
        engine.take_viewport_changed(),
        "re-clamp on resize arms damage"
    );
}

#[test]
fn reconcile_viewport_offset_snaps_to_bottom_when_scrollback_shrinks() {
    // The drift fix targets the case where scrollback shrinks without a resize
    // (alt-screen entry / clear dropping `scrollback_rows`), leaving the stored
    // offset pointing past the new top. `scroll_viewport`/`scroll_to_top`/resize
    // all clamp on write, so the only way to construct a stale stored offset is
    // to set it directly — this exercises `reconcile_viewport_offset` as a unit,
    // decoupled from wezterm escape-sequence semantics.
    let mut engine = viewport_engine(20, 4, 3500);
    fill_scrollback(&mut engine, 20, 10);
    let top = max_offset(&engine);
    assert!(top > 0, "fixture should produce scrollback history");

    // Force the stored offset past the boundary (simulating a shrink that
    // hasn't been reconciled yet).
    engine.set_viewport_offset_for_test(top + 5);
    assert_eq!(engine.viewport_offset(), top + 5);
    let _ = engine.take_viewport_changed();

    let changed = engine.reconcile_viewport_offset();
    assert_eq!(
        engine.viewport_offset(),
        top,
        "reconcile must write the stored offset back to the current max"
    );
    assert!(
        changed,
        "reconcile must report a correction when the stored offset was stale"
    );
    assert!(
        engine.take_viewport_changed(),
        "correction arms viewport damage"
    );

    // A second reconcile on already-consistent state is a no-op.
    assert!(!engine.reconcile_viewport_offset());
    assert!(!engine.take_viewport_changed());

    // When scrollback has fully collapsed (max_offset == 0), reconcile snaps to
    // the live bottom — the alt-screen case.
    engine.set_viewport_offset_for_test(3);
    // Collapse max_offset to 0 by clearing scrollback via the alt screen.
    engine.advance_bytes(b"\x1b[?1049h");
    let collapsed = max_offset(&engine);
    if collapsed == 0 {
        assert!(
            engine.reconcile_viewport_offset(),
            "alt-screen shrink must correct"
        );
        assert_eq!(
            engine.viewport_offset(),
            0,
            "reconcile snaps to bottom on alt screen"
        );
        assert!(engine.take_viewport_changed());
    }
    // Leave the alt screen so the shared default-config cache / other tests are unaffected.
    engine.advance_bytes(b"\x1b[?1049l");
}
