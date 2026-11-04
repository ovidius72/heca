//! **Following the program inside the terminal**: which program is in front, what the shell
//! reports through its escape codes (a command started or finished, its exit code, the folder), and
//! when the pane's program has really ended.

// Named only in the docs below.
#[cfg(doc)]
use crate::backend::PaneBackend;
use super::TerminalBackend;
use super::osc::OscEvent;
use super::process;
use super::pty::PtyRead;
use crate::runtime::ProcessStatus;
use std::io;
use std::time::Instant;

const POST_COMMAND_FG_REFRESHES: u8 = 2;

impl TerminalBackend {
    pub(super) fn configure_agents(&mut self, options: Option<crate::agent::AgentOptions>) -> Result<(), crate::agent::AgentError> {
        if let Some(options) = options {
            let limit = options.max_report_bytes;
            if let Some(session) = &mut self.agent_session {
                session.configure(options)?;
            } else {
                self.agent_session = Some(crate::agent::AgentSession::prepare(options, self.agent_wake.clone())?);
            }
            self.osc.set_notification_limit(limit);
        } else if let Some(session) = &mut self.agent_session {
            session.disable()?;
        }
        let state = self.agent_session.as_ref().and_then(|session| {
            if self.exited || self.runtime.status != ProcessStatus::Running { return None }
            let program = self.runtime.program.as_deref()?;
            let replay = self.last_agent_notification.as_ref().and_then(|notification| {
                session.drivers().read_status(program, crate::agent::AgentInput::Notification(notification)).ok().flatten()
            });
            replay.or_else(|| self.runtime.agent.clone().filter(|agent| {
                session.drivers().recognize(program).is_some_and(|driver| driver.id() == agent.driver)
            }))
        });
        self.set_agent(state);
        Ok(())
    }

    fn set_agent(&mut self, state: Option<crate::agent::AgentState>) {
        let state = state.map(|report| report.retain_session(self.runtime.agent.as_ref()));
        if self.runtime.agent == state { return }
        if let Some(session) = &mut self.agent_session { session.record(state.clone()); }
        self.runtime.agent = state;
    }

    /// Re-sample the PTY's foreground process (event-driven; debounced in
    /// [`update`](PaneBackend::update)). Updates the cached `runtime`
    /// program/status/cwd. `program == None` from the detector ⇒ the shell is
    /// foreground ⇒ report the shell basename + `Idle`.
    pub(super) fn refresh_foreground(&mut self) {
        #[cfg(unix)]
        {
            let Some(master_fd) = self.pty.as_raw_fd() else {
                return;
            };
            let shell_pgrp = self.pty.process_group_leader().unwrap_or(-1);
            let Some(info) = process::detect_foreground(master_fd, shell_pgrp) else {
                return;
            };
            let shell_basename = std::path::Path::new(self.pty.shell_path())
                .file_name()
                .map(|n| n.to_string_lossy().into_owned());
            if self.shell_reported_program_active {
                if let Some(cwd) = info.cwd {
                    self.runtime.cwd = Some(cwd);
                }
                return;
            }
            if !self.semantic_status_active {
                self.runtime.status = if info.program.is_some() {
                    ProcessStatus::Running
                } else {
                    ProcessStatus::Idle
                };
            }
            self.runtime.program = info.program.or(shell_basename);
            if let Some(cwd) = info.cwd {
                self.runtime.cwd = Some(cwd);
            }
        }
        // Non-Unix: no foreground detection (stub); runtime stays at its default.
        #[cfg(not(unix))]
        {
            let _ = &mut self.runtime;
        }
    }

    pub(super) fn apply_osc_event(&mut self, event: OscEvent) -> bool {
        match event {
            OscEvent::PromptStart => true,
            OscEvent::CommandStart | OscEvent::PreExec => {
                self.command_active = true;
                self.command_launch = None;
                self.semantic_status_active = false;
                self.runtime.exit_code = None;
                true
            }
            OscEvent::Program(program) => {
                if !program.is_empty() {
                    self.command_active = true;
                    self.last_agent_notification = None;
                    self.set_agent(None);
                    self.runtime.program = Some(program);
                    self.runtime.status = ProcessStatus::Running;
                    self.semantic_status_active = false;
                    self.runtime.exit_code = None;
                    self.shell_reported_program_active = true;
                }
                false
            }
            OscEvent::CommandFinished(code) => {
                if std::mem::take(&mut self.command_active) {
                    self.pending_alerts.push(crate::backend::BackendAlert::ProcessCompleted { exit_code: code });
                }
                self.command_launch = None;
                self.last_agent_notification = None;
                self.set_agent(None);
                self.runtime.status = ProcessStatus::of_exit(code);
                self.runtime.exit_code = Some(code);
                self.semantic_status_active = true;
                self.shell_reported_program_active = false;
                false
            }
            OscEvent::Cwd(cwd) => {
                self.runtime.cwd = Some(cwd);
                false
            }
            OscEvent::LaunchCommand(command) => {
                self.command_launch = self.pty.command_launch(&command, self.runtime.cwd.as_deref()).ok();
                false
            }
            OscEvent::Notification(notification) => {
                if let Some(session) = &self.agent_session
                    && let Some(program) = self.runtime.program.as_deref()
                    && let Ok(Some(report)) = session.drivers().read_status(
                        program,
                        crate::agent::AgentInput::Notification(&notification),
                    )
                {
                    self.set_agent(Some(report));
                }
                if self.agent_session.is_some() { self.last_agent_notification = Some(notification); }
                // Unrelated and malformed notifications cannot alter process status.
                false
            }
        }
    }
}

/// **What reading the program's output found**: whether anything came, and whether the shell named
/// the program it started.
#[derive(Clone, Copy)]
pub(super) struct Output {
    pub(super) had_data: bool,
    pub(super) saw_program_event: bool,
    pub(super) agent_changed: bool,
}

impl TerminalBackend {
    /// **Read everything the program wrote since the last wake**, feeding it to the emulator and
    /// acting on the shell's escape codes on the way.
    pub(super) fn read_output(&mut self) -> Output {
        let previous_agent = self.runtime.agent.clone();
        let mut had_data = false;
        let mut saw_program_event = false;

        loop {
            match self.pty.try_read() {
                PtyRead::Data(bytes) => {
                    let mut force_fg_refresh = false;
                    for event in self.osc.observe(&bytes) {
                        saw_program_event |= matches!(&event, OscEvent::Program(_));
                        force_fg_refresh |= self.apply_osc_event(event);
                    }
                    self.engine.advance_bytes(&bytes);
                    if force_fg_refresh && !self.exited {
                        self.post_command_fg_refreshes = POST_COMMAND_FG_REFRESHES;
                        if !saw_program_event {
                            self.last_fg_check = Instant::now();
                            self.refresh_foreground();
                        }
                    }
                    had_data = true;
                }
                PtyRead::Empty => break,
                PtyRead::Disconnected => {
                    self.reader_disconnected = true;
                    break;
                }
            }
        }
        let reports = self.agent_session.as_ref().map(|session| session.drain()).unwrap_or_default();
        for report in reports {
            if let Some(session) = &self.agent_session
                && self.runtime.status == ProcessStatus::Running
                && self.runtime.program.as_deref()
                    .and_then(|program| session.drivers().recognize(program))
                    .is_some_and(|driver| driver.id() == report.driver)
            {
                self.set_agent(Some(report));
            }
        }
        Output {
            had_data,
            saw_program_event,
            agent_changed: previous_agent != self.runtime.agent,
        }
    }

    /// **Notice the program ending** — reap it, and keep its exit code for the pane.
    ///
    /// Exit detection is the event: a reader-EOF wake, then `try_wait`. The exit code is captured
    /// so the per-wake monitor can emit `pane.exited{code}`. No polling loop.
    pub(super) fn notice_exit(&mut self) {
        let was_reaped = self.reaped;
        let mut reap_result: io::Result<bool> = Ok(false);
        let mut exit_code: Option<i32> = None;
        if !self.reaped {
            match self.pty.try_wait() {
                Ok(Some(status)) => {
                    exit_code = Some(status.exit_code() as i32);
                    reap_result = Ok(true);
                }
                Ok(None) => reap_result = Ok(false),
                Err(_) => reap_result = Err(io::Error::other("try_wait")),
            }
        }
        reconcile_exit_state(
            self.reader_disconnected,
            &mut self.exited,
            &mut self.reaped,
            reap_result,
        );
        if !was_reaped
            && self.reaped
            && let Some(code) = exit_code
        {
            if !self.semantic_status_active {
                self.pending_alerts.push(crate::backend::BackendAlert::ProcessCompleted { exit_code: code });
            }
            self.command_active = false;
            self.runtime.status = ProcessStatus::of_exit(code);
            self.last_agent_notification = None;
            self.set_agent(None);
            self.runtime.exit_code = Some(code);
            self.pending_exit = Some(code);
        }
    }

    /// **Look again at which program is in front** — soon after a command starts, and otherwise at
    /// most once per debounce, while the shell is alive. Event-first: never on a timer.
    pub(super) fn refresh_foreground_after(&mut self, output: Output) {
        if output.had_data
            && self.post_command_fg_refreshes > 0
            && !self.exited
            && !output.saw_program_event
        {
            self.last_fg_check = Instant::now();
            self.refresh_foreground();
            self.post_command_fg_refreshes -= 1;
        }

        if !self.exited {
            let now = Instant::now();
            if process::fg_re_sample_due(now.duration_since(self.last_fg_check)) {
                self.last_fg_check = now;
                self.refresh_foreground();
            }
        }
    }
}

pub(super) fn reconcile_exit_state(
    reader_disconnected: bool,
    exited: &mut bool,
    reaped: &mut bool,
    reap_result: io::Result<bool>,
) {
    if !*reaped {
        match reap_result {
            Ok(true) => {
                *exited = true;
                *reaped = true;
            }
            // **A failed reap is "I do not know yet", never "it exited."**
            //
            // It used to mean exited, called "conservative" — but the two outcomes are not
            // symmetric. Getting it wrong in this direction **destroys a live pane and everything
            // running in it**; getting it wrong the other way leaves a pane open one wake longer,
            // and the next wake reaps it properly. `try_wait` can fail transiently (a signal
            // interrupting the wait is the ordinary case), so a single unlucky call was enough.
            //
            // That is not theoretical: dragging a pane divider with the mouse **resized the PTY on
            // every frame**, hundreds of times a second, and a pane would vanish mid-drag after a
            // few attempts — while the keyboard resize, which fires a handful of times, never did
            // it. Nothing looked wrong in the layout: the pane was simply gone from the session.
            //
            // The genuinely unreachable child is still caught, one line down and just below: when
            // the reader has ALSO disconnected there is nothing left to read and nothing left to
            // reap, so the pane closes rather than lingering forever.
            Err(_) if reader_disconnected => {
                *exited = true;
                *reaped = true;
            }
            Err(_) | Ok(false) => {}
        }
    }

    if reader_disconnected && *reaped {
        *exited = true;
    }
}
