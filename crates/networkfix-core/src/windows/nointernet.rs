use super::adapters::find_adapters;
use super::services::restart_services;
use crate::progress::ProgressEvent;
use crate::report::Step;
use crate::runner::CommandRunner;

const HOTSPOT_IP: &str = "192.168.137.1";

pub fn fix_no_internet(runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent)) -> Vec<Step> {
    let mut steps = Vec::new();

    steps.extend(restart_services(runner, emit, &["SharedAccess"]));
    steps.push(shared_access_startup(runner, emit));

    let fwd = runner.run(
        "netsh",
        &["int", "ipv4", "set", "global", "forwarding=enabled"],
    );
    let mfw = runner.run(
        "netsh",
        &[
            "int",
            "ipv4",
            "set",
            "global",
            "multicastforwarding=enabled",
        ],
    );
    steps.push(match (fwd, mfw) {
        (Ok(a), Ok(b)) if a.success() && b.success() => Step::ok("ipv4 forwarding", "enabled"),
        _ => Step::warn("ipv4 forwarding", "netsh returned error"),
    });

    steps.extend(ensure_hotspot_ip(runner, emit));

    steps.extend(restart_services(runner, emit, &["icssvc", "WlanSvc"]));

    steps.push(toggle_tethering(runner, emit));

    steps.push(enable_ics_firewall_rules(runner, emit));

    steps.extend(ensure_hotspot_ip(runner, emit));
    steps
}

fn ensure_hotspot_ip(runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent)) -> Vec<Step> {
    emit(ProgressEvent::message("checking hotspot private IP"));
    let mut steps = Vec::new();
    let adapters = find_adapters(runner);
    let targets: Vec<_> = adapters
        .iter()
        .filter(|a| {
            a.name.contains("Local Area Connection*")
                || a.name.contains('*')
                || a.name.contains("Wi-Fi Direct")
        })
        .collect();
    if targets.is_empty() {
        steps.push(Step::skipped(
            "private ip",
            "no Wi-Fi Direct adapter (hotspot may be off)",
        ));
        return steps;
    }
    for a in targets {
        let out = runner.run(
            "netsh",
            &[
                "interface",
                "ipv4",
                "set",
                "address",
                &format!("name={}", a.name),
                "static",
                HOTSPOT_IP,
                "255.255.255.0",
            ],
        );
        steps.push(match out {
            Ok(o) if o.success() => Step::ok(format!("private ip {}", a.name), HOTSPOT_IP),
            Ok(o) => Step::warn(
                format!("private ip {}", a.name),
                o.stderr.trim().to_string(),
            ),
            Err(e) => Step::warn(format!("private ip {}", a.name), e),
        });
    }
    steps
}

fn shared_access_startup(runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent)) -> Step {
    emit(ProgressEvent::message(
        "setting SharedAccess startup to automatic",
    ));
    match runner.run("sc", &["config", "SharedAccess", "start=", "auto"]) {
        Ok(o) if o.success() => Step::ok("SharedAccess startup", "automatic"),
        Ok(o) => Step::warn("SharedAccess startup", o.stderr.trim().to_string()),
        Err(e) => Step::warn("SharedAccess startup", e),
    }
}

fn toggle_tethering(runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent)) -> Step {
    emit(ProgressEvent::message("toggling mobile hotspot"));
    let script = r#"
$p = [Windows.Networking.Connectivity.NetworkInformation, Windows.Networking.Connectivity, ContentType = WindowsRuntime]::GetInternetConnectionProfile()
if (-not $p) { exit 1 }
$tm = [Windows.Networking.NetworkOperators.NetworkOperatorTetheringManager, Windows.Networking.NetworkOperators, ContentType = WindowsRuntime]::CreateFromConnectionProfile($p)
$null = $tm.StopTetheringAsync().AsTask().Wait(15000)
Start-Sleep -Seconds 2
$null = $tm.StartTetheringAsync().AsTask().Wait(20000)
exit 0
"#;
    match runner.run("powershell", &["-NoProfile", "-Command", script]) {
        Ok(o) if o.success() => Step::ok("tethering toggle", "restarted"),
        Ok(o) => Step::warn(
            "tethering toggle",
            format!(
                "toggle failed ({}); manually toggle hotspot in Settings",
                o.stderr.trim()
            ),
        ),
        Err(e) => Step::warn("tethering toggle", e),
    }
}

fn enable_ics_firewall_rules(runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent)) -> Step {
    emit(ProgressEvent::message("checking ICS firewall rules"));
    let script = r#"
$rules = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object {
  $_.DisplayName -match 'Internet Connection Sharing|ICS|Mobile Hotspot|SharedAccess' -or
  $_.Name -match 'SharedAccess|Ics'
}
$enabled = 0
foreach ($r in $rules) { if ($r.Enabled -eq 'False') { Enable-NetFirewallRule -Name $r.Name -ErrorAction SilentlyContinue; $enabled++ } }
Write-Output "enabled=$enabled"
exit 0
"#;
    match runner.run("powershell", &["-NoProfile", "-Command", script]) {
        Ok(o) if o.success() => Step::ok("ics firewall", o.stdout.trim().to_string()),
        Ok(o) => Step::warn("ics firewall", o.stderr.trim().to_string()),
        Err(e) => Step::warn("ics firewall", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn nointernet_runs_forwarding_and_shared_access() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput::ok_empty());
        let steps = fix_no_internet(&m, &|_| {});
        let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("SharedAccess")));
        assert!(names.iter().any(|n| n.contains("forwarding")));
        assert!(names.iter().any(|n| n.contains("SharedAccess startup")));
        let calls = m.calls.borrow();
        assert!(calls.iter().any(|(p, a)| {
            p == "sc"
                && a.contains(&"config".to_string())
                && a.contains(&"SharedAccess".to_string())
                && a.contains(&"start=".to_string())
                && a.contains(&"auto".to_string())
        }));
        assert!(!calls
            .iter()
            .any(|(p, a)| p == "sc" && a.iter().any(|x| x.contains("start= auto"))));
        assert!(steps.iter().all(|s| matches!(
            s.status,
            crate::StepStatus::Ok | crate::StepStatus::Warn | crate::StepStatus::Skipped
        )));
    }

    fn iface_table() -> CommandOutput {
        CommandOutput {
            status: 0,
            stdout: concat!(
                "Admin State    Met     State         Name\n",
                "-------------  ------  ------------  -------------------\n",
                "Connected      0       Connected     Ethernet\n",
                "Connected      0       Connected     Local Area Connection\n",
                "Connected      0       Connected     Local Area Connection* 12\n",
                "Connected      0       Connected     Wi-Fi Direct\n",
            )
            .to_string(),
            stderr: String::new(),
        }
    }

    #[test]
    fn hotspot_ip_only_targets_virtual_adapters_not_bare_lac() {
        let mut m = MockRunner::new();
        m.expect("netsh", iface_table());
        let _steps = ensure_hotspot_ip(&m, &|_| {});
        let calls = m.calls.borrow();
        let set_addrs: Vec<&Vec<String>> = calls
            .iter()
            .filter(|(p, a)| {
                p == "netsh" && a.contains(&"set".to_string()) && a.contains(&"address".to_string())
            })
            .map(|(_, a)| a)
            .collect();
        assert!(set_addrs
            .iter()
            .any(|a| a.contains(&"name=Local Area Connection* 12".to_string())));
        assert!(set_addrs
            .iter()
            .any(|a| a.contains(&"name=Wi-Fi Direct".to_string())));
        assert!(!set_addrs
            .iter()
            .any(|a| a.contains(&"name=Local Area Connection".to_string())));
        assert!(!set_addrs
            .iter()
            .any(|a| a.contains(&"name=Ethernet".to_string())));
    }
}
