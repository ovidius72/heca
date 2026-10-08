//! **What a frame costs the CPU** — an average, printed, for whoever is tuning the render path.
//!
//! Off unless `HECA_FRAME_TIMES` is set in the environment. When it is, every [`WINDOW`] frames that
//! actually drew print one line: the average time `render_frame` took on the CPU and the average
//! number of render passes it began.
//!
//! **This is CPU time only.** The GPU runs after `submit` returns, so how long it takes is not in
//! the number; a change that moves work from the CPU to the GPU shows as a win here and may not be
//! one.

use std::time::{Duration, Instant};

/// How many drawn frames one printed line averages over.
const WINDOW: u32 = 120;

/// The environment variable that turns the log on.
const SWITCH: &str = "HECA_FRAME_TIMES";

/// The running average. Costs one `Instant::now` pair per frame when on, nothing when off.
pub(crate) struct FrameTimes {
    on: bool,
    started: Option<Instant>,
    frames: u32,
    cpu: Duration,
    passes: u64,
}

impl FrameTimes {
    /// Reads the switch once, at startup.
    pub(crate) fn from_env() -> Self {
        Self {
            on: std::env::var_os(SWITCH).is_some(),
            started: None,
            frames: 0,
            cpu: Duration::ZERO,
            passes: 0,
        }
    }

    /// A frame is about to be drawn.
    pub(crate) fn start(&mut self) {
        if self.on {
            self.started = Some(Instant::now());
        }
    }

    /// The frame was drawn, in `passes` render passes. A call that drew nothing (`passes == 0`: the
    /// loop asked and nothing was dirty) is not a frame and is not counted.
    pub(crate) fn finish(&mut self, passes: u32) {
        let Some(started) = self.started.take() else {
            return;
        };
        if passes == 0 {
            return;
        }
        self.cpu += started.elapsed();
        self.passes += u64::from(passes);
        self.frames += 1;
        if self.frames == WINDOW {
            eprintln!("{}", self.line());
            self.frames = 0;
            self.cpu = Duration::ZERO;
            self.passes = 0;
        }
    }

    /// The line for the frames counted so far.
    fn line(&self) -> String {
        let frames = f64::from(self.frames.max(1));
        format!(
            "[frame-times] {} frames: {:.2} ms CPU, {:.1} render passes per frame (GPU time is not measured)",
            self.frames,
            self.cpu.as_secs_f64() * 1000.0 / frames,
            self.passes as f64 / frames,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> FrameTimes {
        FrameTimes {
            on: true,
            ..FrameTimes::from_env()
        }
    }

    #[test]
    fn the_line_averages_the_frames_it_counted() {
        let mut t = on();
        t.frames = 4;
        t.cpu = Duration::from_millis(10);
        t.passes = 30;
        assert_eq!(
            t.line(),
            "[frame-times] 4 frames: 2.50 ms CPU, 7.5 render passes per frame (GPU time is not measured)"
        );
    }

    #[test]
    fn a_call_that_drew_nothing_is_not_a_frame() {
        let mut t = on();
        t.start();
        t.finish(0);
        assert_eq!(t.frames, 0);
        t.start();
        t.finish(3);
        assert_eq!((t.frames, t.passes), (1, 3));
    }

    #[test]
    fn off_it_counts_nothing() {
        let mut t = FrameTimes {
            on: false,
            ..FrameTimes::from_env()
        };
        t.start();
        t.finish(3);
        assert_eq!(t.frames, 0);
    }
}
