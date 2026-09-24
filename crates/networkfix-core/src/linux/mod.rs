pub mod nm;

use crate::mode::Mode;
use crate::progress::ProgressEvent;
use crate::report::{FixReport, Step};
use crate::runner::CommandRunner;
pub use nm::{detect_backend, Backend};

pub fn run_mode(
    mode: Mode,
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    elevated: bool,
) -> FixReport {
    let mut report = FixReport::new(mode, elevated);
    let backend = detect_backend(runner);
    emit(ProgressEvent::message(format!("backend: {backend:?}")));
    if backend == Backend::None {
        let s = Step::fail(
            "backend",
            "no NetworkManager / systemd-networkd / iwd detected",
        );
        emit(ProgressEvent::Step(s.clone()));
        report.push(s);
        return report;
    }
    let steps = match mode {
        Mode::Quick => nm::quick_steps(runner, emit, backend),
        Mode::Full => nm::full_steps(runner, emit, backend),
        Mode::HotspotOnly => nm::hotspot_only_steps(runner, emit, backend),
        Mode::NoInternet => nm::no_internet_steps(runner, emit, backend),
    };
    for s in steps {
        emit(ProgressEvent::Step(s.clone()));
        report.push(s);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::StepStatus;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn detects_networkmanager_from_nmcli() {
        let mut m = MockRunner::new();
        m.expect(
            "nmcli",
            CommandOutput {
                status: 0,
                stdout: "running\n".into(),
                stderr: String::new(),
            },
        );
        assert_eq!(detect_backend(&m), Backend::NetworkManager);
    }

    #[test]
    fn none_backend_fails_gracefully() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput {
            status: 1,
            stdout: String::new(),
            stderr: "not found".into(),
        });
        assert_eq!(detect_backend(&m), Backend::None);
        let report = run_mode(Mode::Quick, &m, &|_| {}, true);
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].status, StepStatus::Fail);
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn quick_with_nm_restarts_networkmanager_and_flushes_dns() {
        let mut m = MockRunner::new();
        m.expect(
            "nmcli",
            CommandOutput {
                status: 0,
                stdout: "running\n".into(),
                stderr: String::new(),
            },
        );
        m.set_default(CommandOutput::ok_empty());
        let report = run_mode(Mode::Quick, &m, &|_| {}, true);
        assert!(report
            .steps
            .iter()
            .any(|s| s.name.contains("NetworkManager")));
        assert!(report.steps.iter().any(|s| s.name == "flush dns"));
        let calls = m.calls.borrow();
        assert!(calls.iter().any(|(p, a)| {
            p == "systemctl"
                && a.contains(&"restart".to_string())
                && a.contains(&"NetworkManager".to_string())
        }));
        assert!(report.exit_code() <= 1);
    }

    #[test]
    fn hotspot_only_skips_when_no_ap_profile() {
        let mut m = MockRunner::new();
        m.expect_seq(
            "nmcli",
            vec![
                CommandOutput {
                    status: 0,
                    stdout: "running\n".into(),
                    stderr: String::new(),
                },
                CommandOutput::ok_empty(),
            ],
        );
        m.set_default(CommandOutput::ok_empty());
        let report = run_mode(Mode::HotspotOnly, &m, &|_| {}, true);
        assert!(!report.steps.is_empty());
        assert!(report
            .steps
            .iter()
            .any(|s| s.name == "hotspot" && s.status == StepStatus::Skipped));
        assert_eq!(report.exit_code(), 0);
    }
}
