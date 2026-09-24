use crate::progress::ProgressEvent;
use crate::report::Step;
use crate::runner::CommandRunner;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Wwan,
    Wifi,
    Other,
}

#[derive(Debug, Clone)]
pub struct AdapterInfo {
    pub name: String,
    pub description: String,
    pub status: String,
}

const WWAN_TOKENS: &[&str] = &[
    "mobile",
    "wwan",
    "fibocom",
    "sierra",
    "quectel",
    "broadband",
    "lte",
    "hspa",
    "cellular",
];
const WIFI_TOKENS: &[&str] = &[
    "wi-fi",
    "wifi",
    "wireless",
    "wlan",
    "802.11",
    "hosted network",
    "wi-fi direct",
];

fn contains_token(hay: &str, token: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = hay[start..].find(token) {
        let idx = start + pos;
        let before = hay[..idx].chars().next_back();
        let after = hay[idx + token.len()..].chars().next();
        let boundary = |c: Option<char>| c.map(|c| !c.is_alphanumeric()).unwrap_or(true);
        if boundary(before) && boundary(after) {
            return true;
        }
        start = idx + token.len();
    }
    false
}

pub fn classify(name: &str, desc: &str) -> AdapterKind {
    let hay = format!("{} {}", name, desc).to_ascii_lowercase();
    if WWAN_TOKENS.iter().any(|t| contains_token(&hay, t)) {
        AdapterKind::Wwan
    } else if WIFI_TOKENS.iter().any(|t| contains_token(&hay, t)) {
        AdapterKind::Wifi
    } else {
        AdapterKind::Other
    }
}

pub fn find_adapters(runner: &dyn CommandRunner) -> Vec<AdapterInfo> {
    let Ok(out) = runner.run("netsh", &["interface", "show", "interface"]) else {
        return Vec::new();
    };
    out.stdout
        .lines()
        .filter(|l| l.contains("Connected") || l.contains("Disconnected") || l.contains("Disabled"))
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                return None;
            }
            let status = parts[1..3].join(" ");
            let name = parts[3..].join(" ");
            Some(AdapterInfo {
                name,
                description: String::new(),
                status,
            })
        })
        .collect()
}

pub fn restart_adapter(
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    name: &str,
) -> Step {
    emit(ProgressEvent::message(format!("cycling adapter {name}")));
    let step_name = format!("adapter {name}");
    let name_arg = format!("name={name}");
    let disable = runner.run(
        "netsh",
        &["interface", "set", "interface", &name_arg, "admin=disable"],
    );
    match disable {
        Ok(o) if o.success() || o.stdout.is_empty() || o.stderr.is_empty() => {}
        Ok(o) => {
            return Step::warn(step_name, format!("disable: {}", o.stderr.trim()));
        }
        Err(e) => return Step::warn(step_name, e),
    }
    std::thread::sleep(Duration::from_secs(2));
    match runner.run(
        "netsh",
        &["interface", "set", "interface", &name_arg, "admin=enable"],
    ) {
        Ok(o) if o.success() => Step::ok(step_name, "cycled"),
        Ok(o) => Step::warn(step_name, format!("enable: {}", o.stderr.trim())),
        Err(e) => Step::warn(step_name, e),
    }
}

pub fn flush_dns(runner: &dyn CommandRunner) -> Step {
    match runner.run("ipconfig", &["/flushdns"]) {
        Ok(o) if o.success() => Step::ok("flush dns", "cache cleared"),
        Ok(o) => Step::warn("flush dns", o.stderr.trim().to_string()),
        Err(e) => Step::warn("flush dns", e),
    }
}

pub fn renew_dhcp(runner: &dyn CommandRunner, iface: &str) -> Step {
    let name_arg = format!("name={iface}");
    let a = runner.run(
        "netsh",
        &[
            "interface",
            "ipv4",
            "set",
            "address",
            &name_arg,
            "source=dhcp",
        ],
    );
    let b = runner.run(
        "netsh",
        &[
            "interface",
            "ipv4",
            "set",
            "dnsservers",
            &name_arg,
            "source=dhcp",
        ],
    );
    match (a, b) {
        (Ok(x), Ok(y)) if x.success() && y.success() => Step::ok(format!("renew {iface}"), "dhcp"),
        (Ok(x), _) if !x.success() => {
            Step::warn(format!("renew {iface}"), x.stderr.trim().to_string())
        }
        (_, Ok(y)) if !y.success() => {
            Step::warn(format!("renew {iface}"), y.stderr.trim().to_string())
        }
        (Err(e), _) | (_, Err(e)) => Step::warn(format!("renew {iface}"), e),
        (Ok(_), Ok(_)) => Step::warn(format!("renew {iface}"), "netsh returned error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn classify_matches_wwan_and_wifi() {
        assert_eq!(
            classify("Cellular", "HP Mobile Broadband"),
            AdapterKind::Wwan
        );
        assert_eq!(classify("Wi-Fi", "Intel Wireless-AC"), AdapterKind::Wifi);
        assert_eq!(classify("Ethernet", "Realtek PCIe"), AdapterKind::Other);
    }

    #[test]
    fn restart_uses_netsh_disable_enable() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput::ok_empty());
        let step = restart_adapter(&m, &|_| {}, "Wi-Fi");
        assert_eq!(step.status, crate::StepStatus::Ok);
        let calls = m.calls.borrow();
        let netsh_args: Vec<&Vec<String>> = calls
            .iter()
            .filter(|(p, _)| p == "netsh")
            .map(|(_, a)| a)
            .collect();
        assert!(netsh_args
            .iter()
            .any(|a| a.contains(&"admin=disable".to_string())));
        assert!(netsh_args
            .iter()
            .any(|a| a.contains(&"admin=enable".to_string())));
        assert!(netsh_args
            .iter()
            .any(|a| a.contains(&"name=Wi-Fi".to_string())));
    }

    #[test]
    fn flush_dns_reports_ok() {
        let mut m = MockRunner::new();
        m.expect("ipconfig", CommandOutput::ok_empty());
        assert_eq!(flush_dns(&m).status, crate::StepStatus::Ok);
    }
}
