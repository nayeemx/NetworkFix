use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_networkfix"))
}

#[test]
fn json_quick_outputs_valid_json_on_stdout() {
    let out = bin()
        .env("NETWORKFIX_DRY_RUN", "1")
        .args(["--json", "--no-elevate", "quick"])
        .output()
        .expect("run");
    // stdout must be pure JSON
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout not JSON: {e}\n{}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    assert_eq!(v["mode"], "quick");
    assert!(v["steps"].is_array());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("dry-run"),
        "stderr should note dry-run (no live network): {err}"
    );
}

#[test]
fn unknown_mode_exits_nonzero_with_message() {
    let out = bin().args(["bogus"]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("unknown mode") || err.contains("error"));
}

#[test]
fn version_flag_prints_version() {
    let out = bin().arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("networkfix"));
}
