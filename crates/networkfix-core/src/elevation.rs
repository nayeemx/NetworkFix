use crate::runner::{hide_console_window, CommandRunner, SystemRunner};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    AlreadyElevated,
    RelaunchRequested,
    Denied,
    Unknown,
}

#[cfg(any(unix, test))]
pub(crate) fn elevated_from_id(r: &dyn CommandRunner) -> bool {
    r.run("id", &["-u"])
        .map(|o| o.stdout.trim() == "0")
        .unwrap_or(false)
}

#[cfg(unix)]
pub fn is_elevated() -> bool {
    elevated_from_id(&SystemRunner)
}

#[cfg(windows)]
pub fn is_elevated() -> bool {
    SystemRunner
        .run("net", &["session"])
        .map(|o| o.success())
        .unwrap_or(false)
}

#[cfg(any(windows, test))]
fn quote_windows_arg(arg: &str) -> String {
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            other => {
                if backslashes > 0 {
                    out.push_str(&"\\".repeat(backslashes));
                    backslashes = 0;
                }
                out.push(other);
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

#[cfg(any(windows, test))]
pub(crate) fn windows_relaunch_script(exe: &Path, args: &[String]) -> String {
    let exe = exe.display().to_string().replace('\'', "''");
    let mut launch = format!("Start-Process -FilePath '{exe}' -Verb RunAs -Wait");
    if !args.is_empty() {
        let child_cmd = args
            .iter()
            .map(|a| quote_windows_arg(a))
            .collect::<Vec<_>>()
            .join(" ");
        let ps = child_cmd.replace('\'', "''");
        launch.push_str(&format!(" -ArgumentList '{ps}'"));
    }
    format!("try {{ {launch} -ErrorAction Stop; exit 0 }} catch {{ exit 1 }}")
}

#[cfg(any(windows, unix, test))]
fn elevation_from_child_exit(code: Option<i32>) -> Elevation {
    match code {
        Some(0) => Elevation::RelaunchRequested,
        _ => Elevation::Denied,
    }
}

#[cfg(windows)]
pub fn relaunch_elevated(current_exe: &Path, args: &[String]) -> Result<Elevation, String> {
    let script = windows_relaunch_script(current_exe, args);
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut cmd);
    let status = cmd.status().map_err(|e| format!("powershell: {e}"))?;
    Ok(elevation_from_child_exit(status.code()))
}

#[cfg(unix)]
pub fn relaunch_elevated(current_exe: &Path, args: &[String]) -> Result<Elevation, String> {
    let exe = current_exe.display().to_string();

    let mut pkexec = vec![exe.clone()];
    pkexec.extend(args.iter().cloned());
    let pkexec_refs: Vec<&str> = pkexec.iter().map(|s| s.as_str()).collect();
    match Command::new("pkexec")
        .args(&pkexec_refs)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(st) => return Ok(elevation_from_child_exit(st.code())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("pkexec: {e}")),
    }

    let mut sudo = vec![exe];
    sudo.extend(args.iter().cloned());
    let sudo_refs: Vec<&str> = sudo.iter().map(|s| s.as_str()).collect();
    match Command::new("sudo")
        .args(&sudo_refs)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(st) => Ok(elevation_from_child_exit(st.code())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err("neither pkexec nor sudo is available; run with sudo".to_string())
        }
        Err(e) => Err(format!("sudo: {e}")),
    }
}

#[cfg(all(not(unix), not(windows)))]
pub fn is_elevated() -> bool {
    false
}

#[cfg(all(not(unix), not(windows)))]
pub fn relaunch_elevated(_current_exe: &Path, _args: &[String]) -> Result<Elevation, String> {
    Ok(Elevation::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn linux_elevation_reads_id_output() {
        let mut m = MockRunner::new();
        m.expect(
            "id",
            CommandOutput {
                status: 0,
                stdout: "0\n".into(),
                stderr: String::new(),
            },
        );
        assert!(elevated_from_id(&m));
        let mut m2 = MockRunner::new();
        m2.expect(
            "id",
            CommandOutput {
                status: 0,
                stdout: "1000\n".into(),
                stderr: String::new(),
            },
        );
        assert!(!elevated_from_id(&m2));
    }

    #[test]
    fn elevated_from_id_handles_runner_error() {
        let m = MockRunner::new();
        assert!(!elevated_from_id(&m));
    }

    #[cfg(any(windows, test))]
    #[test]
    fn windows_script_single_quotes_exe_path_with_spaces() {
        let s = windows_relaunch_script(
            Path::new(r"C:\Program Files\NetworkFix\networkfix.exe"),
            &[],
        );
        assert!(s.contains(r"-FilePath 'C:\Program Files\NetworkFix\networkfix.exe'"));
        assert!(s.contains("-Verb RunAs"));
        assert!(!s.contains("-ArgumentList"));
    }

    #[cfg(any(windows, test))]
    #[test]
    fn windows_script_quotes_args_with_spaces_as_units() {
        let s = windows_relaunch_script(
            Path::new(r"C:\app\networkfix.exe"),
            &[
                "hello world".to_string(),
                "--mode".to_string(),
                "quick".to_string(),
            ],
        );
        assert!(s.contains(r#"-ArgumentList '"hello world" "--mode" "quick"'"#));
    }

    #[cfg(any(windows, test))]
    #[test]
    fn windows_script_escapes_single_and_double_quotes() {
        let s = windows_relaunch_script(
            Path::new(r"C:\app's dir\app.exe"),
            &["say \"hi\"".to_string()],
        );
        assert!(s.contains(r"-FilePath 'C:\app''s dir\app.exe'"));
        assert!(s.contains(r#"-ArgumentList '"say \"hi\""'"#));
    }

    #[cfg(any(windows, test))]
    #[test]
    fn windows_script_waits_and_reports_denial_via_exit_codes() {
        let s = windows_relaunch_script(Path::new(r"C:\app\networkfix.exe"), &[]);
        assert!(s.starts_with("try {"), "script: {s}");
        assert!(s.contains("-Verb RunAs -Wait"), "script: {s}");
        assert!(s.contains("-ErrorAction Stop"), "script: {s}");
        assert!(s.contains("exit 0"), "script: {s}");
        assert!(s.contains("exit 1"), "script: {s}");
        assert!(s.contains("catch"), "script: {s}");
    }

    #[cfg(any(windows, unix, test))]
    #[test]
    fn child_exit_zero_means_completed_otherwise_denied() {
        assert_eq!(
            elevation_from_child_exit(Some(0)),
            Elevation::RelaunchRequested
        );
        assert_eq!(elevation_from_child_exit(Some(1)), Elevation::Denied);
        assert_eq!(elevation_from_child_exit(Some(126)), Elevation::Denied);
        assert_eq!(elevation_from_child_exit(None), Elevation::Denied);
    }
}
