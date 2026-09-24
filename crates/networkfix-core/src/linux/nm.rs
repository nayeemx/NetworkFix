use crate::progress::ProgressEvent;
use crate::report::{Step, StepStatus};
use crate::runner::CommandRunner;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    NetworkManager,
    SystemdNetworkd,
    Iwd,
    None,
}

pub fn detect_backend(runner: &dyn CommandRunner) -> Backend {
    if let Ok(o) = runner.run("nmcli", &["-t", "-f", "RUNNING", "general"]) {
        if o.success() && o.stdout.contains("running") {
            return Backend::NetworkManager;
        }
    }
    if let Ok(o) = runner.run("systemctl", &["is-active", "NetworkManager"]) {
        if o.success() {
            return Backend::NetworkManager;
        }
    }
    if runner
        .run("networkctl", &["status"])
        .map(|o| o.success())
        .unwrap_or(false)
    {
        return Backend::SystemdNetworkd;
    }
    if runner
        .run("iwctl", &["help"])
        .map(|o| o.success())
        .unwrap_or(false)
    {
        return Backend::Iwd;
    }
    Backend::None
}

pub fn flush_dns(runner: &dyn CommandRunner) -> Step {
    if let Ok(o) = runner.run("resolvectl", &["flush-caches"]) {
        if o.success() {
            return Step::ok("flush dns", "resolvectl");
        }
    }
    if let Ok(o) = runner.run("systemd-resolve", &["--flush-caches"]) {
        if o.success() {
            return Step::ok("flush dns", "systemd-resolve");
        }
    }
    Step::warn("flush dns", "no resolver cache tool found")
}

pub fn restart_nm(runner: &dyn CommandRunner) -> Step {
    match runner.run("systemctl", &["restart", "NetworkManager"]) {
        Ok(o) if o.success() => Step::ok("NetworkManager", "restarted"),
        Ok(o) => Step::warn("NetworkManager", o.stderr.trim().to_string()),
        Err(e) => Step::warn("NetworkManager", e),
    }
}

pub fn wifi_devices(runner: &dyn CommandRunner) -> Vec<String> {
    let Ok(o) = runner.run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE", "dev", "status"]) else {
        return Vec::new();
    };
    o.stdout
        .lines()
        .filter_map(|l| {
            let parts: Vec<&str> = l.split(':').collect();
            if parts.len() >= 2 && (parts[1] == "wifi" || parts[1] == "wwan") {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .collect()
}

pub fn quick_steps(
    runner: &dyn CommandRunner,
    _emit: &dyn Fn(ProgressEvent),
    _backend: Backend,
) -> Vec<Step> {
    let mut steps = vec![];
    steps.push(restart_nm(runner));
    steps.push(flush_dns(runner));
    for dev in wifi_devices(runner).into_iter().take(3) {
        let r = runner.run("nmcli", &["device", "reapply", &dev]);
        steps.push(match r {
            Ok(o) if o.success() => Step::ok(format!("reapply {dev}"), ""),
            Ok(o) => Step::warn(format!("reapply {dev}"), o.stderr.trim().to_string()),
            Err(e) => Step::warn(format!("reapply {dev}"), e),
        });
    }
    if steps.iter().all(|s| s.status == StepStatus::Skipped) {
        steps.push(Step::warn("quick", "no actionable network devices"));
    }
    steps
}

pub fn full_steps(
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    backend: Backend,
) -> Vec<Step> {
    let mut steps = quick_steps(runner, emit, backend);
    for dev in wifi_devices(runner) {
        steps.push(cycle_link(runner, &dev));
    }
    steps.push(restart_wpa(runner));
    steps.push(restart_nm(runner));
    steps
}

fn cycle_link(runner: &dyn CommandRunner, dev: &str) -> Step {
    let _ = runner.run("ip", &["link", "set", "dev", dev, "down"]);
    std::thread::sleep(Duration::from_secs(2));
    match runner.run("ip", &["link", "set", "dev", dev, "up"]) {
        Ok(o) if o.success() => Step::ok(format!("link {dev}"), "cycled"),
        Ok(o) => Step::warn(format!("link {dev}"), o.stderr.trim().to_string()),
        Err(e) => Step::warn(format!("link {dev}"), e),
    }
}

fn restart_wpa(runner: &dyn CommandRunner) -> Step {
    match runner.run("systemctl", &["restart", "wpa_supplicant"]) {
        Ok(o) if o.success() => Step::ok("wpa_supplicant", "restarted"),
        _ => Step::warn("wpa_supplicant", "not restarted (unit missing or failed)"),
    }
}

pub fn hotspot_only_steps(
    runner: &dyn CommandRunner,
    _emit: &dyn Fn(ProgressEvent),
    _backend: Backend,
) -> Vec<Step> {
    let mut steps = vec![];
    let con = runner.run("nmcli", &["-t", "-f", "NAME,TYPE", "con", "show"]);
    let hotspot_name = con.ok().and_then(|o| {
        o.stdout.lines().find_map(|l| {
            let mut it = l.splitn(2, ':');
            let name = it.next()?.to_string();
            let ty = it.next().unwrap_or("");
            if ty.contains("802-11-ap") || name.to_ascii_lowercase().contains("hotspot") {
                Some(name)
            } else {
                None
            }
        })
    });
    match hotspot_name {
        Some(name) => {
            let _ = runner.run("nmcli", &["con", "down", &name]);
            std::thread::sleep(Duration::from_secs(1));
            match runner.run("nmcli", &["con", "up", &name]) {
                Ok(o) if o.success() => {
                    steps.push(Step::ok(format!("hotspot {name}"), "restarted"))
                }
                Ok(o) => steps.push(Step::warn(
                    format!("hotspot {name}"),
                    o.stderr.trim().to_string(),
                )),
                Err(e) => steps.push(Step::warn(format!("hotspot {name}"), e)),
            }
        }
        None => steps.push(Step::skipped(
            "hotspot",
            "no AP/hotspot profile found — turn hotspot on first",
        )),
    }
    steps.push(flush_dns(runner));
    steps
}

pub fn no_internet_steps(
    _runner: &dyn CommandRunner,
    _emit: &dyn Fn(ProgressEvent),
    _backend: Backend,
) -> Vec<Step> {
    vec![Step::skipped("no-internet", "pending task 10")]
}
