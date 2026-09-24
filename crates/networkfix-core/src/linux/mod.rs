use crate::mode::Mode;
use crate::progress::ProgressEvent;
use crate::report::{FixReport, Step};
use crate::runner::CommandRunner;

pub fn run_mode(
    mode: Mode,
    _runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    elevated: bool,
) -> FixReport {
    let mut r = FixReport::new(mode, elevated);
    let s = Step::fail("linux backend", "not yet implemented");
    emit(ProgressEvent::Step(s.clone()));
    r.push(s);
    r
}
