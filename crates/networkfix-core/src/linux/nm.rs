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
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    _backend: Backend,
) -> Vec<Step> {
    let mut steps = Vec::new();
    emit(ProgressEvent::message("enabling ip forwarding"));

    steps.push(sysctl_set(runner, "net.ipv4.ip_forward", "1"));
    steps.push(sysctl_set(runner, "net.ipv4.conf.all.forwarding", "1"));
    steps.push(Step::warn(
        "forwarding persistence",
        "reboot resets sysctl — add to /etc/sysctl.d/ if needed",
    ));

    match default_dev(runner) {
        Some(dev) => steps.push(masquerade(runner, &dev)),
        None => steps.push(Step::warn("nat", "no default route found")),
    }

    steps.push(restart_nm(runner));
    steps.push(flush_dns(runner));
    steps.push(report_hotspot_addrs(runner));
    steps
}

fn sysctl_set(runner: &dyn CommandRunner, key: &str, val: &str) -> Step {
    match runner.run("sysctl", &["-w", &format!("{key}={val}")]) {
        Ok(o) if o.success() => Step::ok(format!("sysctl {key}"), val),
        Ok(o) => Step::warn(format!("sysctl {key}"), o.stderr.trim().to_string()),
        Err(e) => Step::warn(format!("sysctl {key}"), e),
    }
}

fn default_dev(runner: &dyn CommandRunner) -> Option<String> {
    let o = runner.run("ip", &["-4", "route", "show", "default"]).ok()?;
    if !o.success() {
        return None;
    }
    let tokens: Vec<&str> = o.stdout.split_whitespace().collect();
    tokens
        .windows(2)
        .find(|w| w[0] == "dev")
        .map(|w| w[1].to_string())
}

fn masquerade(runner: &dyn CommandRunner, dev: &str) -> Step {
    let check = runner.run(
        "iptables",
        &[
            "-t",
            "nat",
            "-C",
            "POSTROUTING",
            "-o",
            dev,
            "-j",
            "MASQUERADE",
        ],
    );
    if !matches!(check, Ok(o) if o.success()) {
        let add = runner.run(
            "iptables",
            &[
                "-t",
                "nat",
                "-A",
                "POSTROUTING",
                "-o",
                dev,
                "-j",
                "MASQUERADE",
            ],
        );
        if !matches!(add, Ok(o) if o.success()) {
            return Step::warn(
                "masquerade",
                "iptables failed — install iptables or add nft MASQUERADE manually",
            );
        }
    }
    let fwd = runner.run(
        "iptables",
        &[
            "-A",
            "FORWARD",
            "-i",
            dev,
            "-o",
            dev,
            "-m",
            "state",
            "--state",
            "RELATED,ESTABLISHED",
            "-j",
            "ACCEPT",
        ],
    );
    if fwd.is_err() {
        return Step::warn("forward rules", "iptables not available");
    }
    Step::ok(format!("masquerade {dev}"), "NAT enabled")
}

fn report_hotspot_addrs(runner: &dyn CommandRunner) -> Step {
    let Ok(o) = runner.run("ip", &["-4", "-o", "addr", "show"]) else {
        return Step::skipped("hotspot address", "ip command failed");
    };
    let has_private = o
        .stdout
        .lines()
        .any(|l| l.contains("inet 10.") || l.contains("inet 192.168.") || l.contains("inet 172."));
    if has_private {
        Step::ok("hotspot address", "private IPv4 present on an interface")
    } else {
        Step::warn("hotspot address", "no private IPv4 found — is hotspot on?")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn no_internet_enables_forwarding_and_masquerade() {
        let mut m = MockRunner::new();
        m.expect(
            "nmcli",
            CommandOutput {
                status: 0,
                stdout: "running\n".into(),
                stderr: String::new(),
            },
        );
        m.expect_seq(
            "ip",
            vec![
                CommandOutput {
                    status: 0,
                    stdout: "default via 192.0.2.1 dev eth0 proto dhcp\n".into(),
                    stderr: String::new(),
                },
                CommandOutput::ok_empty(),
            ],
        );
        m.set_default(CommandOutput::ok_empty());
        let steps = no_internet_steps(&m, &|_| {}, Backend::NetworkManager);
        let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("forwarding")));
        assert!(names
            .iter()
            .any(|n| n.contains("masquerade") || n.contains("nat")));
        let calls = m.calls.borrow();
        assert!(calls.iter().any(|(p, _)| p == "sysctl"));
        assert!(calls
            .iter()
            .any(|(p, a)| p == "iptables" && a.iter().any(|x| x.contains("MASQUERADE"))));
    }
}
