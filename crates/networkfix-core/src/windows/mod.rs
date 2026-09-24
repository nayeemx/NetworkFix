pub mod adapters;
pub mod nointernet;
pub mod services;

use crate::mode::Mode;
use crate::progress::ProgressEvent;
use crate::report::{FixReport, Step};
use crate::runner::CommandRunner;
use adapters::{classify, find_adapters, flush_dns, renew_dhcp, restart_adapter, AdapterKind};
use services::{restart_services, QUICK_SERVICES};

fn push_step(report: &mut FixReport, emit: &dyn Fn(ProgressEvent), step: Step) {
    emit(ProgressEvent::Step(step.clone()));
    report.push(step);
}

fn push_steps(report: &mut FixReport, emit: &dyn Fn(ProgressEvent), steps: Vec<Step>) {
    for s in steps {
        push_step(report, emit, s);
    }
}

pub fn run_mode(
    mode: Mode,
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    elevated: bool,
) -> FixReport {
    let mut report = FixReport::new(mode, elevated);

    match mode {
        Mode::NoInternet => {
            push_steps(&mut report, emit, nointernet::fix_no_internet(runner, emit));
        }
        Mode::HotspotOnly => {
            push_steps(
                &mut report,
                emit,
                restart_services(runner, emit, &["WlanSvc", "SharedAccess", "icssvc"]),
            );
            for a in find_adapters(runner) {
                if classify(&a.name, &a.description) == AdapterKind::Wifi {
                    let s = restart_adapter(runner, emit, &a.name);
                    push_step(&mut report, emit, s);
                }
            }
            let s = flush_dns(runner);
            push_step(&mut report, emit, s);
        }
        Mode::Quick | Mode::Full => {
            push_steps(
                &mut report,
                emit,
                restart_services(runner, emit, QUICK_SERVICES),
            );
            let fwd = runner.run(
                "netsh",
                &["int", "ipv4", "set", "global", "forwarding=enabled"],
            );
            let step = match fwd {
                Ok(o) if o.success() => Step::ok("ipv4 forwarding", "enabled"),
                _ => Step::warn("ipv4 forwarding", "netsh error"),
            };
            push_step(&mut report, emit, step);

            let adapters = find_adapters(runner);
            let wwan: Vec<_> = adapters
                .iter()
                .filter(|a| classify(&a.name, &a.description) == AdapterKind::Wwan)
                .collect();
            if wwan.is_empty() {
                let s = Step::skipped("wwan adapter", "not present");
                push_step(&mut report, emit, s);
            } else {
                for a in &wwan {
                    let s = restart_adapter(runner, emit, &a.name);
                    push_step(&mut report, emit, s);
                }
            }

            let dns = flush_dns(runner);
            push_step(&mut report, emit, dns);

            if let Some(a) = wwan.first() {
                let s = renew_dhcp(runner, &a.name);
                push_step(&mut report, emit, s);
            }

            if mode == Mode::Full {
                for args in [
                    vec!["winsock", "reset"],
                    vec!["int", "ip", "reset"],
                    vec!["int", "tcp", "reset"],
                ] {
                    let r = runner.run("netsh", &args);
                    let name = format!("netsh {}", args.join(" "));
                    let s = match r {
                        Ok(o) if o.success() => Step::ok(name, "done"),
                        Ok(o) => Step::warn(name, o.stderr.trim().to_string()),
                        Err(e) => Step::warn(name, e),
                    };
                    push_step(&mut report, emit, s);
                }
                for a in find_adapters(runner) {
                    if classify(&a.name, &a.description) == AdapterKind::Wifi {
                        let s = restart_adapter(runner, emit, &a.name);
                        push_step(&mut report, emit, s);
                    }
                }
                push_steps(
                    &mut report,
                    emit,
                    restart_services(runner, emit, &["icssvc", "WlanSvc"]),
                );
            }
        }
    }
    report
}

#[cfg(test)]
mod mode_tests {
    use super::*;
    use crate::mode::Mode;
    use crate::runner::{CommandOutput, MockRunner};
    use crate::StepStatus;

    fn mock() -> MockRunner {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput::ok_empty());
        m
    }

    #[test]
    fn quick_skips_wifi_cycle_but_cycles_wwan_if_present() {
        let m = mock();
        let report = run_mode(Mode::Quick, &m, &|_| {}, true);
        assert_eq!(report.mode, Mode::Quick);
        assert!(!report.steps.is_empty());
        assert!(report
            .steps
            .iter()
            .any(|s| s.status == StepStatus::Skipped && s.name.contains("adapter")));
    }

    #[test]
    fn full_includes_winsock_reset() {
        let m = mock();
        let _ = run_mode(Mode::Full, &m, &|_| {}, true);
        let calls = m.calls.borrow();
        assert!(calls
            .iter()
            .any(|(p, a)| p == "netsh" && a.contains(&"winsock".to_string())));
    }

    #[test]
    fn hotspot_only_does_not_restart_phone_tapisrv_set() {
        let m = mock();
        let report = run_mode(Mode::HotspotOnly, &m, &|_| {}, true);
        assert!(!report.steps.iter().any(|s| s.name == "PhoneSvc"));
        assert!(!report.steps.iter().any(|s| s.name == "tapisrv"));
        assert!(report.steps.iter().any(|s| s.name == "WlanSvc"));
    }

    #[test]
    fn never_panics_on_empty_environment() {
        let m = mock();
        for mode in [Mode::Quick, Mode::Full, Mode::HotspotOnly, Mode::NoInternet] {
            let _ = run_mode(mode, &m, &|_| {}, false);
        }
        let failing = MockRunner::new();
        for mode in [Mode::Quick, Mode::Full, Mode::HotspotOnly] {
            let _ = run_mode(mode, &failing, &|_| {}, false);
        }
    }
}
