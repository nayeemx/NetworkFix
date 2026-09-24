use crate::progress::ProgressEvent;
use crate::report::Step;
use crate::runner::CommandRunner;
use std::time::Duration;

pub const QUICK_SERVICES: &[&str] = &[
    "icssvc",
    "WlanSvc",
    "SharedAccess",
    "EapHost",
    "dot3svc",
    "RasMan",
    "PhoneSvc",
    "tapisrv",
];

pub fn restart_services(
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    services: &[&str],
) -> Vec<Step> {
    let mut steps = Vec::new();
    for name in services {
        emit(ProgressEvent::message(format!("restarting service {name}")));
        let query = match runner.run("sc", &["query", name]) {
            Ok(o) if o.success() || o.stdout.contains("STATE") => o,
            Ok(_) => {
                steps.push(Step::skipped(*name, "service not present"));
                continue;
            }
            Err(e) => {
                steps.push(Step::warn(*name, e));
                continue;
            }
        };
        let was_running = query.stdout.contains("RUNNING");
        if was_running {
            let _ = runner.run("sc", &["stop", name]);
            std::thread::sleep(Duration::from_millis(400));
        }
        match runner.run("sc", &["start", name]) {
            Ok(o) if o.success() => steps.push(Step::ok(*name, "restarted")),
            Ok(o) => steps.push(Step::warn(
                *name,
                format!("start failed: {}", o.stderr.trim()),
            )),
            Err(e) => steps.push(Step::warn(*name, e)),
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    fn out(status: i32, stdout: &str) -> CommandOutput {
        CommandOutput {
            status,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    #[test]
    fn missing_service_is_skipped_not_failed() {
        let mut m = MockRunner::new();
        m.expect("sc", out(1060, ""));
        let steps = restart_services(&m, &|_| {}, &["NoSuchSvc"]);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].status, crate::StepStatus::Skipped);
    }

    #[test]
    fn running_service_is_stopped_then_started() {
        let mut m = MockRunner::new();
        m.expect("sc", out(0, "STATE : 4 RUNNING"));
        let steps = restart_services(&m, &|_| {}, &["WlanSvc"]);
        assert_eq!(steps[0].status, crate::StepStatus::Ok);
        let calls = m.calls.borrow();
        let verbs: Vec<String> = calls
            .iter()
            .filter(|(p, _)| p == "sc")
            .flat_map(|(_, a)| a.iter().cloned())
            .collect();
        assert!(verbs.iter().any(|v| v == "stop"));
        assert!(verbs.iter().any(|v| v == "start"));
    }

    #[test]
    fn emits_progress_per_service() {
        let m = {
            let mut m = MockRunner::new();
            m.expect("sc", out(0, "STATE : 1 STOPPED"));
            m
        };
        let events = std::cell::RefCell::new(Vec::new());
        let _ = restart_services(&m, &|e| events.borrow_mut().push(e), &["A", "B"]);
        assert_eq!(events.borrow().len(), 2);
    }
}
