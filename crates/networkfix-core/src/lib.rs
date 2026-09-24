pub mod elevation;
#[cfg(any(target_os = "linux", test))]
pub mod linux;
pub mod mode;
pub mod progress;
pub mod report;
pub mod runner;
#[cfg(any(target_os = "windows", test))]
pub mod windows;

use std::time::Instant;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use elevation::{is_elevated, relaunch_elevated, Elevation};
pub use mode::Mode;
pub use progress::ProgressEvent;
pub use report::{FixReport, Step, StepStatus};
pub use runner::{CommandOutput, CommandRunner, SystemRunner};

pub fn run_fix_with(
    mode: Mode,
    runner: &dyn CommandRunner,
    elevated: bool,
    on_event: &dyn Fn(ProgressEvent),
) -> FixReport {
    on_event(ProgressEvent::Start { mode });
    let started = Instant::now();
    #[cfg(target_os = "windows")]
    let mut report = windows::run_mode(mode, runner, on_event, elevated);
    #[cfg(target_os = "linux")]
    let mut report = linux::run_mode(mode, runner, on_event, elevated);
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let mut report = {
        let mut r = FixReport::new(mode, elevated);
        r.push(Step::fail("platform", "unsupported operating system"));
        r
    };
    report.duration_ms = started.elapsed().as_millis() as u64;
    on_event(ProgressEvent::Done { report: report.clone() });
    report
}

pub fn run_fix(mode: Mode, on_event: impl Fn(ProgressEvent)) -> FixReport {
    let elevated = is_elevated();
    run_fix_with(mode, &SystemRunner, elevated, &on_event)
}

#[cfg(test)]
mod run_fix_tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn run_fix_with_emits_start_done_and_duration() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput::ok_empty());
        let events = std::cell::RefCell::new(Vec::new());
        let report = run_fix_with(Mode::Quick, &m, true, &|e| events.borrow_mut().push(e));
        let events = events.borrow();
        assert!(matches!(events.first(), Some(ProgressEvent::Start { mode: Mode::Quick })));
        assert!(matches!(events.last(), Some(ProgressEvent::Done { .. })));
        assert!(report.elevated);
    }
}
