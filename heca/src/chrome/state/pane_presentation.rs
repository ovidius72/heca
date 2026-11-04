//! Stable display names beside immediate process facts.

use std::time::{Duration, Instant};

use heca_core::runtime::{PaneRuntime, ProcessStatus};

/// A pane's display program and its unchanged runtime facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanePresentation {
    pub(crate) program: Option<String>,
    pub(crate) runtime: PaneRuntime,
}

impl PanePresentation {
    /// The program name after the configured display delay.
    pub fn program(&self) -> Option<&str> { self.program.as_deref() }

    /// Raw process, agent and status facts, with no presentation delay.
    pub fn runtime(&self) -> &PaneRuntime { &self.runtime }

    #[cfg(test)]
    pub(crate) fn immediate(runtime: PaneRuntime) -> Self {
        Self { program: runtime.program.clone(), runtime }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ProgramName {
    pub shown: Option<String>,
    source: Option<String>,
    resting: Option<String>,
    running: bool,
    returning: bool,
    since: Option<Instant>,
}

impl ProgramName {
    pub fn observe(&mut self, runtime: &PaneRuntime, delay: Duration, now: Instant) {
        let source = runtime.program.as_ref().filter(|name| !name.is_empty()).cloned();
        let running = runtime.status == ProcessStatus::Running;
        let changed = self.source != source;
        let started = running && !self.running;
        let completed = self.running && !running;
        let finished_visible = completed && self.source == self.shown;
        self.returning = !running && (self.returning || finished_visible);
        self.source = source;
        self.running = running;
        let returns_to_rest = changed && !running && (self.source == self.resting || self.returning);
        if finished_visible && self.resting.is_some() && !delay.is_zero() {
            self.shown = self.resting.clone();
            self.since = None;
            self.returning = false;
        } else if delay.is_zero() || self.shown.is_none() || returns_to_rest {
            self.shown = self.source.clone();
            if !running && self.resting.is_none() { self.resting = self.source.clone(); }
            self.since = None;
            self.returning = false;
        } else if self.source == self.shown || (completed && !changed) {
            // Completion can precede the final foreground sample that restores the shell.
            // Cancel its pending title instead of publishing an already-finished command.
            self.since = None;
        } else if changed || started {
            self.since = Some(now);
        }
        self.advance(delay, now);
    }

    pub fn advance(&mut self, delay: Duration, now: Instant) {
        if self.since.is_some_and(|since| now.saturating_duration_since(since) >= delay) {
            self.shown = self.source.clone();
            if !self.running { self.resting = self.source.clone(); }
            self.since = None;
        }
    }

    pub fn deadline(&self, delay: Duration) -> Option<Instant> {
        self.since.and_then(|since| since.checked_add(delay))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(program: &str, status: ProcessStatus) -> PaneRuntime {
        PaneRuntime { program: Some(program.into()), status, ..Default::default() }
    }

    #[test]
    fn short_commands_never_replace_the_shell_even_before_the_final_sample() {
        let now = Instant::now();
        let delay = Duration::from_secs(1);
        let mut name = ProgramName::default();
        name.observe(&runtime("zsh", ProcessStatus::Idle), delay, now);
        for command in ["ls", "cd", "false"] {
            name.observe(&runtime(command, ProcessStatus::Running), delay, now);
            name.observe(&runtime(command, ProcessStatus::Success), delay, now);
            name.observe(&runtime(command, ProcessStatus::Success), delay, now);
            name.advance(delay, now + delay);
            assert_eq!(name.shown.as_deref(), Some("zsh"));
            assert_eq!(name.deadline(delay), None);
            name.observe(&runtime("zsh", ProcessStatus::Success), delay, now);
        }
        // A server snapshot can coalesce away the short command's Running state entirely.
        name.observe(&runtime("ls", ProcessStatus::Success), delay, now);
        assert_eq!(name.shown.as_deref(), Some("zsh"));
        name.observe(&runtime("zsh", ProcessStatus::Success), delay, now);
        assert_eq!(name.deadline(delay), None);
    }

    #[test]
    fn persistent_programs_publish_at_the_deadline_and_restore_the_shell_immediately() {
        let now = Instant::now();
        let delay = Duration::from_secs(1);
        for program in ["vim", "a-plugin-program"] {
            let mut name = ProgramName::default();
            name.observe(&runtime("zsh", ProcessStatus::Idle), delay, now);
            name.observe(&runtime(program, ProcessStatus::Running), delay, now);
            name.advance(delay, now + delay - Duration::from_nanos(1));
            assert_eq!(name.shown.as_deref(), Some("zsh"));
            name.advance(delay, now + delay);
            assert_eq!(name.shown.as_deref(), Some(program));
            name.observe(&runtime("zsh", ProcessStatus::Success), delay, now + delay);
            assert_eq!(name.shown.as_deref(), Some("zsh"));
            assert_eq!(name.deadline(delay), None);
        }
    }

    #[test]
    fn a_new_candidate_restarts_the_delay_and_zero_disables_it() {
        let now = Instant::now();
        let delay = Duration::from_secs(1);
        let mut name = ProgramName::default();
        name.observe(&runtime("zsh", ProcessStatus::Idle), delay, now);
        name.observe(&runtime("ls", ProcessStatus::Running), delay, now);
        name.observe(&runtime("vim", ProcessStatus::Running), delay, now + delay);
        assert_eq!(name.shown.as_deref(), Some("zsh"));
        assert_eq!(name.deadline(delay), Some(now + delay + delay));
        name.observe(&runtime("vim", ProcessStatus::Running), Duration::ZERO, now + delay);
        assert_eq!(name.shown.as_deref(), Some("vim"));
        assert_eq!(name.deadline(delay), None);
    }

    #[test]
    fn repeated_reports_keep_the_original_deadline_and_reload_reuses_elapsed_time() {
        let now = Instant::now();
        let delay = Duration::from_secs(2);
        let mut name = ProgramName::default();
        name.observe(&runtime("zsh", ProcessStatus::Idle), delay, now);
        name.observe(&runtime("editor", ProcessStatus::Running), delay, now);
        name.observe(&runtime("editor", ProcessStatus::Running), delay, now + Duration::from_secs(1));
        assert_eq!(name.deadline(delay), Some(now + delay));
        name.observe(&runtime("editor", ProcessStatus::Running), Duration::from_secs(1), now + Duration::from_secs(1));
        assert_eq!(name.shown.as_deref(), Some("editor"));
        assert_eq!(name.deadline(delay), None);
    }

    #[test]
    fn attaching_after_completion_learns_the_returned_shell_without_getting_stuck() {
        let now = Instant::now();
        let delay = Duration::from_secs(1);
        let mut name = ProgramName::default();
        name.observe(&runtime("editor", ProcessStatus::Success), delay, now);
        name.observe(&runtime("zsh", ProcessStatus::Success), delay, now);
        name.observe(&runtime("zsh", ProcessStatus::Success), delay, now + Duration::from_millis(500));
        name.advance(delay, now + delay);
        assert_eq!(name.shown.as_deref(), Some("zsh"));
        name.observe(&runtime("editor", ProcessStatus::Running), delay, now + delay);
        name.advance(delay, now + delay + delay);
        name.observe(&runtime("zsh", ProcessStatus::Success), delay, now + delay + delay);
        assert_eq!(name.shown.as_deref(), Some("zsh"));
        assert_eq!(name.deadline(delay), None);
    }

    #[test]
    fn completion_restores_a_known_shell_before_the_foreground_sample_arrives() {
        let now = Instant::now();
        let delay = Duration::from_secs(1);
        let mut name = ProgramName::default();
        name.observe(&runtime("zsh", ProcessStatus::Idle), delay, now);
        name.observe(&runtime("editor", ProcessStatus::Running), delay, now);
        name.advance(delay, now + delay);
        name.observe(&runtime("editor", ProcessStatus::Success), delay, now + delay);
        assert_eq!(name.shown.as_deref(), Some("zsh"));
        name.observe(&runtime("editor", ProcessStatus::Success), delay, now + delay);
        assert_eq!(name.shown.as_deref(), Some("zsh"));
        assert_eq!(name.deadline(delay), None);

        let mut attached = ProgramName::default();
        attached.observe(&runtime("editor", ProcessStatus::Running), delay, now);
        attached.observe(&runtime("editor", ProcessStatus::Success), delay, now);
        attached.observe(&runtime("zsh", ProcessStatus::Success), delay, now);
        assert_eq!(attached.shown.as_deref(), Some("zsh"));
        assert_eq!(attached.deadline(delay), None);
    }
}
