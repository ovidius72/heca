//! [`Elapsed`] — how long ago something happened, kept current without rebuilding the tree.
//!
//! "2m", "3h": a [`Label`] the caller styles, whose words this widget writes as time passes. It
//! never redraws on a timer of its own: it tells the host, through
//! [`next_redraw`](Component::next_redraw), when its words next change, and the host sleeps until
//! then — once a second for the first minute, then once a minute, an hour, a day.

use std::time::{Duration, SystemTime};

use crate::component::{Base, Component};
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use crate::widgets::Label;

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// How an elapsed time reads. The default is [`Elapsed::compact`].
pub type ElapsedFormat = fn(Duration) -> String;

/// **"45s", "2m", "3h", "4d"** — the largest whole unit, unit symbols rather than words, so it
/// reads the same in every language. A caller with words of its own passes
/// [`format`](Elapsed::format).
fn compact(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    match secs {
        s if s < MINUTE => format!("{s}s"),
        s if s < HOUR => format!("{}m", s / MINUTE),
        s if s < DAY => format!("{}h", s / HOUR),
        s => format!("{}d", s / DAY),
    }
}

/// The time from `elapsed` to the next moment the shown unit's words change: the next whole second,
/// minute, hour or day, by the unit it is showing now.
fn until_next_change(elapsed: Duration) -> Duration {
    let unit = match elapsed.as_secs() {
        s if s < MINUTE => 1,
        s if s < HOUR => MINUTE,
        s if s < DAY => HOUR,
        _ => DAY,
    };
    let unit = Duration::from_secs(unit);
    let into = Duration::from_nanos((elapsed.as_nanos() % unit.as_nanos()) as u64);
    unit - into
}

/// How long ago `at` was, shown in a [`Label`] the caller styles:
///
/// ```ignore
/// Elapsed::since(sent_at, Label::new("").size(WidgetSize::Caption).muted(true))
/// ```
pub struct Elapsed {
    base: Base,
    /// When it happened, on the library's clock ([`crate::clock`]), so a test can move time.
    at: std::time::Instant,
    text: Signal<String>,
    format: ElapsedFormat,
}

/// `at` on the library's clock: a moment in the future is now.
fn on_clock(at: SystemTime) -> std::time::Instant {
    let ago = SystemTime::now().duration_since(at).unwrap_or_default();
    crate::clock::now().checked_sub(ago).unwrap_or_else(crate::clock::now)
}

#[heca_grid_ui_macros::props]
impl Elapsed {
    /// How long ago `at` was, written into `label`. A moment in the future reads as no time at all.
    pub fn since(at: SystemTime, label: Label) -> Self {
        let at = on_clock(at);
        let text = label.text_signal();
        let label: crate::WidgetModel = crate::builders::IntoComponent::into_component(label);
        let mut base = Base::container();
        crate::component::wrap_transparently(&mut base, label.as_ref());
        base.children = vec![label];
        let elapsed = Self { base, at, text, format: compact };
        elapsed.write();
        elapsed
    }

    /// **When it happened, as milliseconds since the Unix epoch** — the form a stored record
    /// carries.
    #[heca_grid_ui_macros::host_only(
        "a description's numbers travel as f32, which cannot hold an epoch time to the second"
    )]
    pub fn at_millis(mut self, epoch_ms: u64) -> Self {
        self.at = on_clock(SystemTime::UNIX_EPOCH + Duration::from_millis(epoch_ms));
        self.write();
        self
    }

    /// **"45s", "2m", "3h", "4d"** — the default words: the largest whole unit, as unit symbols.
    pub fn compact(elapsed: Duration) -> String {
        compact(elapsed)
    }

    /// **The words for an elapsed time**, when the default [`compact`](Self::compact) is not what
    /// the caller shows — its own language, or a longer form. They are rewritten when the shown
    /// unit changes: each second for the first minute, then each minute, hour, day.
    #[heca_grid_ui_macros::host_only("a function cannot cross as a static property")]
    pub fn format(mut self, format: ElapsedFormat) -> Self {
        self.format = format;
        self.write();
        self
    }

    fn elapsed(&self) -> Duration {
        crate::clock::now().saturating_duration_since(self.at)
    }

    /// Write the words for now, only when they differ.
    fn write(&self) {
        let words = (self.format)(self.elapsed());
        if self.text.get_untracked() != words {
            self.text.set(words);
        }
    }
}

impl Component for Elapsed {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The words follow the clock; the label under it picks them up in its own tick. The rest is
    /// the default walk, which an override cannot call.
    fn tick(&mut self, dt: f32) -> bool {
        self.write();
        let mut animating = self.base.tick_presence(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// When the words next change — the host wakes then, and not before — or sooner when the label
    /// under it asks.
    fn next_redraw(&self) -> Option<f32> {
        if self.base.style.layout.hidden {
            return None;
        }
        let own = until_next_change(self.elapsed()).as_secs_f32();
        let label = self.base.children.iter().filter_map(|child| child.next_redraw());
        Some(label.fold(own, f32::min))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn the_words_are_the_largest_whole_unit() {
        assert_eq!(compact(secs(0)), "0s");
        assert_eq!(compact(secs(59)), "59s");
        assert_eq!(compact(secs(60)), "1m");
        assert_eq!(compact(secs(HOUR - 1)), "59m");
        assert_eq!(compact(secs(HOUR)), "1h");
        assert_eq!(compact(secs(DAY + HOUR)), "1d");
    }

    #[test]
    fn the_next_change_is_the_next_whole_unit_being_shown() {
        assert_eq!(until_next_change(Duration::from_millis(1_250)), Duration::from_millis(750));
        assert_eq!(until_next_change(secs(90)), secs(30));
        assert_eq!(until_next_change(secs(HOUR + 60)), secs(HOUR - 60));
        assert_eq!(until_next_change(secs(2 * DAY)), secs(DAY));
    }

    #[test]
    fn the_label_follows_the_clock_and_the_host_is_woken_only_at_a_change() {
        crate::clock::hold();
        let mut elapsed = Elapsed::since(SystemTime::now() - secs(58), Label::new(""));
        let text = elapsed.text;
        assert_eq!(text.get_untracked(), "58s");
        let wake = elapsed.next_redraw().expect("a timed wake while shown");
        assert!(wake <= 1.0, "within the second: {wake}");

        crate::clock::advance(secs(3));
        elapsed.tick(0.0);
        assert_eq!(text.get_untracked(), "1m");
        let wake = elapsed.next_redraw().unwrap();
        assert!(wake > 50.0 && wake <= 60.0, "about a minute later: {wake}");
        crate::clock::release();
    }

    #[test]
    fn a_caller_can_give_its_own_words() {
        let elapsed = Elapsed::since(SystemTime::now() - secs(120), Label::new(""))
            .format(|ago| format!("{} min ago", ago.as_secs() / MINUTE));
        assert_eq!(elapsed.text.get_untracked(), "2 min ago");
    }

    #[test]
    fn a_stored_moment_in_epoch_milliseconds_reads_the_same() {
        let at = SystemTime::now() - secs(3 * HOUR);
        let millis = at.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as u64;
        let elapsed = Elapsed::since(SystemTime::now(), Label::new("")).at_millis(millis);
        assert_eq!(elapsed.text.get_untracked(), "3h");
    }

    #[test]
    fn a_hidden_elapsed_asks_for_no_wake() {
        let mut elapsed = Elapsed::since(SystemTime::now(), Label::new(""));
        elapsed.base_mut().set_hidden(true);
        assert_eq!(elapsed.next_redraw(), None);
    }
}
