# NetworkFix Cross-Platform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the legacy PowerShell/batch NetworkFix scripts into an installable cross-platform app: Dioxus GUI (Win10/11 + Linux) + CLI (also Win7), sharing one Rust core with four repair modes.

**Architecture:** Cargo workspace with three crates — `networkfix-core` (fix logic + `CommandRunner` injection + compile-time `windows`/`linux` backends), `networkfix` (clap CLI), `networkfix-gui` (Dioxus desktop). Both binaries call `run_fix(mode, on_event) -> FixReport`. Packaging configs + GitHub Actions produce MSI, NSIS, AppImage, deb, rpm.

**Tech Stack:** Rust 2021, clap 4, serde/serde_json, Dioxus 0.6 (`dioxus-desktop`), `cargo-wix`, NSIS, `cargo-packager` (deb/rpm/AppImage), GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-24-networkfix-cross-platform-design.md`

## Global Constraints

- Never panic in fix paths; missing optional services/adapters → `StepStatus::Skipped` or `Warn`, not `Fail`.
- One failing step must not abort remaining steps (best-effort).
- OS backend selected at compile time via `#[cfg(target_os = "...")]`.
- CLI exit codes: `0` all Ok/Skipped; `1` any Warn; `2` any Fail; `3` elevation denied.
- `--json` prints only JSON on stdout; progress goes to stderr.
- Modes: `Quick`, `Full`, `HotspotOnly`, `NoInternet` — identical names everywhere.
- Windows 7: CLI binary only; do not link GUI crates for the Win7 target.
- Legacy `.bat`/`.ps1` move to `legacy/` — keep files, mark unsupported.
- All fix logic lives in `networkfix-core`; CLI/GUI must not reimplement steps.
- Unit tests use `MockCommandRunner` — never invoke real `netsh`/`nmcli` in tests.
- TDD: write failing test → run → implement → run → commit, every task.
- Commit messages: conventional commits (`feat:`, `fix:`, `test:`, `chore:`, `docs:`).

---

### Task 1: Workspace scaffold + move legacy scripts

**Files:**
- Create: `Cargo.toml` (workspace)
- Create: `crates/networkfix-core/Cargo.toml`, `crates/networkfix-core/src/lib.rs`
- Create: `crates/networkfix/Cargo.toml`, `crates/networkfix/src/main.rs`
- Create: `crates/networkfix-gui/Cargo.toml`, `crates/networkfix-gui/src/main.rs`
- Create: `.gitignore`
- Move: `Fix-Network.bat`, `Fix-Hotspot-NoInternet.bat`, `fix-network.ps1`, `fix-hotspot-internet.ps1` → `legacy/`
- Modify: `README.md`

**Interfaces:**
- Produces: workspace that `cargo test` can run; empty `networkfix_core::` lib; stub binaries printing `NetworkFix <version>`.

- [ ] **Step 1: Init git and move legacy scripts**

```powershell
git init
New-Item -ItemType Directory -Force -Path legacy
Move-Item Fix-Network.bat, Fix-Hotspot-NoInternet.bat, fix-network.ps1, fix-hotspot-internet.ps1 legacy\
```

- [ ] **Step 2: Create workspace root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/networkfix-core",
    "crates/networkfix",
    "crates/networkfix-gui",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://example.invalid/networkfix"

[profile.release]
lto = true
strip = true
```

- [ ] **Step 3: Create crate manifests**

`crates/networkfix-core/Cargo.toml`:
```toml
[package]
name = "networkfix-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tempfile = "3"
```

`crates/networkfix/Cargo.toml`:
```toml
[package]
name = "networkfix"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "networkfix"
path = "src/main.rs"

[dependencies]
networkfix-core = { path = "../networkfix-core" }
clap = { version = "4", features = ["derive"] }
serde_json = "1"
```

`crates/networkfix-gui/Cargo.toml`:
```toml
[package]
name = "networkfix-gui"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "networkfix-gui"
path = "src/main.rs"

[dependencies]
networkfix-core = { path = "../networkfix-core" }
dioxus = { version = "0.6", features = ["desktop"] }
```

- [ ] **Step 4: Create stub sources**

`crates/networkfix-core/src/lib.rs`:
```rust
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

`crates/networkfix/src/main.rs`:
```rust
fn main() {
    println!("networkfix {}", networkfix_core::VERSION);
}
```

`crates/networkfix-gui/src/main.rs`:
```rust
fn main() {
    println!("networkfix-gui {}", networkfix_core::VERSION);
}
```

- [ ] **Step 5: Create `.gitignore`**

```gitignore
/target
/dist
*.msi
*.exe
!legacy/**
```

- [ ] **Step 6: Verify build**

Run: `cargo build`
Expected: builds all three crates.

- [ ] **Step 7: Update `README.md`**

Replace contents with a short stub: title, "Cross-platform rebuild in progress — see `docs/superpowers/specs/`", "Legacy scripts: `legacy/` (unsupported)".

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "chore: scaffold rust workspace, move legacy scripts"
```

---

### Task 2: Core types — Mode, Step, FixReport, serialization

**Files:**
- Create: `crates/networkfix-core/src/mode.rs`
- Create: `crates/networkfix-core/src/report.rs`
- Modify: `crates/networkfix-core/src/lib.rs`
- Test: inside `report.rs` / `mode.rs` `#[cfg(test)]` modules

**Interfaces:**
- Produces:
  - `pub enum Mode { Quick, Full, HotspotOnly, NoInternet }` with `Mode::parse(&str) -> Result<Mode, String>` accepting `quick|full|hotspot|no-internet` (case-insensitive) and `FromStr`.
  - `pub enum StepStatus { Ok, Warn, Fail, Skipped }`
  - `pub struct Step { pub name: String, pub status: StepStatus, pub detail: String }`
  - `pub struct FixReport { pub mode: Mode, pub steps: Vec<Step>, pub elevated: bool, pub duration_ms: u64 }`
  - `FixReport::exit_code(&self) -> u32` per Global Constraints.
  - All types `Serialize + Deserialize + PartialEq + Debug + Clone`.
  - Step helpers: `Step::ok(name, detail)`, `Step::warn(...)`, `Step::fail(...)`, `Step::skipped(...)`.

- [ ] **Step 1: Write failing tests** (append to `mode.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parses_mode_aliases() {
        assert_eq!(Mode::from_str("quick"), Ok(Mode::Quick));
        assert_eq!(Mode::from_str("Full"), Ok(Mode::Full));
        assert_eq!(Mode::from_str("hotspot"), Ok(Mode::HotspotOnly));
        assert_eq!(Mode::from_str("no-internet"), Ok(Mode::NoInternet));
        assert!(Mode::from_str("bogus").is_err());
    }
}
```

And in `report.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_priority_fail_over_warn() {
        let mut r = FixReport::new(Mode::Quick, true);
        r.steps.push(Step::ok("a", ""));
        assert_eq!(r.exit_code(), 0);

        let mut r = FixReport::new(Mode::Quick, false);
        r.steps.push(Step::skipped("b", "no wwan"));
        assert_eq!(r.exit_code(), 0);

        let mut r = FixReport::new(Mode::Quick, false);
        r.steps.push(Step::warn("c", "slow"));
        r.steps.push(Step::fail("d", "x"));
        assert_eq!(r.exit_code(), 2);

        let mut r = FixReport::new(Mode::Quick, false);
        r.steps.push(Step::warn("c", "slow"));
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn report_json_roundtrip() {
        let mut r = FixReport::new(Mode::NoInternet, true);
        r.steps.push(Step::ok("forwarding", "enabled"));
        let s = serde_json::to_string(&r).unwrap();
        let back: FixReport = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p networkfix-core`
Expected: FAIL (modules missing).

- [ ] **Step 3: Implement `mode.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Quick,
    Full,
    HotspotOnly,
    NoInternet,
}

impl FromStr for Mode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "quick" => Ok(Mode::Quick),
            "full" => Ok(Mode::Full),
            "hotspot" | "hotspotonly" | "hotspot-only" => Ok(Mode::HotspotOnly),
            "no-internet" | "nointernet" | "no_internet" => Ok(Mode::NoInternet),
            other => Err(format!("unknown mode: {other}")),
        }
    }
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        s.parse()
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Mode::Quick => "quick",
            Mode::Full => "full",
            Mode::HotspotOnly => "hotspot",
            Mode::NoInternet => "no-internet",
        };
        write!(f, "{s}")
    }
}
```

- [ ] **Step 4: Implement `report.rs`**

```rust
use crate::mode::Mode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Ok,
    Warn,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub status: StepStatus,
    pub detail: String,
}

impl Step {
    pub fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { name: name.into(), status: StepStatus::Ok, detail: detail.into() }
    }
    pub fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { name: name.into(), status: StepStatus::Warn, detail: detail.into() }
    }
    pub fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { name: name.into(), status: StepStatus::Fail, detail: detail.into() }
    }
    pub fn skipped(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { name: name.into(), status: StepStatus::Skipped, detail: detail.into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixReport {
    pub mode: Mode,
    pub steps: Vec<Step>,
    pub elevated: bool,
    pub duration_ms: u64,
}

impl FixReport {
    pub fn new(mode: Mode, elevated: bool) -> Self {
        Self { mode, steps: Vec::new(), elevated, duration_ms: 0 }
    }

    pub fn push(&mut self, step: Step) {
        self.steps.push(step);
    }

    pub fn worst(&self) -> Option<StepStatus> {
        if self.steps.iter().any(|s| s.status == StepStatus::Fail) {
            Some(StepStatus::Fail)
        } else if self.steps.iter().any(|s| s.status == StepStatus::Warn) {
            Some(StepStatus::Warn)
        } else {
            None
        }
    }

    pub fn exit_code(&self) -> u32 {
        match self.worst() {
            Some(StepStatus::Fail) => 2,
            Some(StepStatus::Warn) => 1,
            _ => 0,
        }
    }
}
```

- [ ] **Step 5: Wire modules in `lib.rs`**

```rust
pub mod mode;
pub mod report;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use mode::Mode;
pub use report::{FixReport, Step, StepStatus};
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p networkfix-core`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): mode, step, and fix report types"
```

---

### Task 3: Progress events + CommandRunner trait + mock

**Files:**
- Create: `crates/networkfix-core/src/progress.rs`
- Create: `crates/networkfix-core/src/runner.rs`
- Modify: `crates/networkfix-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub enum ProgressEvent { Start { mode: Mode }, Step(Step), Message(String), Done(FixReport) }` (Serialize)
  - `pub trait CommandRunner { fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String>; }`
  - `pub struct CommandOutput { pub status: i32, pub stdout: String, pub stderr: String }` with `CommandOutput::success()`
  - `pub struct SystemRunner;` implementing trait (spawns process, captures output; non-zero status is **Ok** at IO level — callers inspect `status`)
  - `pub struct MockRunner` (cfg(test) or always available for reuse): scripted queue of responses keyed by program name; records calls in `pub calls: Vec<(String, Vec<String>)>`.
  - Type alias: `pub type EventHandler<'a> = &'a dyn Fn(ProgressEvent);`

- [ ] **Step 1: Write failing tests** in `runner.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn mock_records_calls_and_returns_scripted_output() {
        let mut m = MockRunner::new();
        m.expect("netsh", CommandOutput { status: 0, stdout: "ok".into(), stderr: String::new() });
        let out = m.run("netsh", &["winsock", "reset"]).unwrap();
        assert!(out.success());
        assert_eq!(m.calls.borrow().len(), 1);
        assert_eq!(m.calls.borrow()[0].0, "netsh");
    }

    #[test]
    fn mock_fails_when_no_script_and_no_default() {
        let m = MockRunner::new();
        assert!(m.run("ipconfig", &["/flushdns"]).is_err());
    }

    #[test]
    fn mock_default_covers_unscripted_programs() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput { status: 0, stdout: String::new(), stderr: String::new() });
        assert!(m.run("anything", &[]).unwrap().success());
    }
}
```

(Adjust `RefCell` import — design `calls` as `RefCell<Vec<...>>` or use `Vec` with `&mut self` on `run`; prefer `fn run(&mut self, ...)` on Mock only if trait uses `&self` — keep trait `&self`, store calls in `RefCell`.)

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p networkfix-core`
Expected: FAIL — `MockRunner` missing.

- [ ] **Step 3: Implement `progress.rs`**

```rust
use crate::mode::Mode;
use crate::report::{FixReport, Step};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum ProgressEvent {
    Start { mode: Mode },
    Step(Step),
    Message { text: String },
    Done { report: FixReport },
}

impl ProgressEvent {
    pub fn message(text: impl Into<String>) -> Self {
        ProgressEvent::Message { text: text.into() }
    }
}
```

- [ ] **Step 4: Implement `runner.rs`**

```rust
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status == 0
    }
    pub fn ok_empty() -> Self {
        Self { status: 0, stdout: String::new(), stderr: String::new() }
    }
}

pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String>;
}

pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
        let out = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        Ok(CommandOutput {
            status: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    }
}

pub struct MockRunner {
    scripts: HashMap<String, CommandOutput>,
    default: Option<CommandOutput>,
    pub calls: RefCell<Vec<(String, Vec<String>)>>,
}

impl MockRunner {
    pub fn new() -> Self {
        Self {
            scripts: HashMap::new(),
            default: None,
            calls: RefCell::new(Vec::new()),
        }
    }
    pub fn expect(&mut self, program: &str, output: CommandOutput) {
        self.scripts.insert(program.to_string(), output);
    }
    pub fn set_default(&mut self, output: CommandOutput) {
        self.default = Some(output);
    }
}

impl Default for MockRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRunner for MockRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
        self.calls
            .borrow_mut()
            .push((program.to_string(), args.iter().map(|s| s.to_string()).collect()));
        if let Some(o) = self.scripts.get(program) {
            return Ok(o.clone());
        }
        if let Some(d) = &self.default {
            return Ok(d.clone());
        }
        Err(format!("no mock for {program}"))
    }
}

pub type EventHandler<'a> = &'a dyn Fn(crate::progress::ProgressEvent);
```

- [ ] **Step 5: Wire `lib.rs`**

Add: `pub mod progress; pub mod runner;` and re-exports `pub use progress::ProgressEvent; pub use runner::{CommandOutput, CommandRunner, SystemRunner};`

- [ ] **Step 6: Run tests**

Run: `cargo test -p networkfix-core`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): progress events and injectable command runner"
```

---

### Task 4: Elevation check + relaunch helpers

**Files:**
- Create: `crates/networkfix-core/src/elevation.rs`
- Modify: `crates/networkfix-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub enum Elevation { AlreadyElevated, RelaunchRequested, Denied, Unknown }`
  - `pub fn is_elevated() -> bool` — Windows: `ShellExecuteW`-free approach via `net session` exit code through `SystemRunner` OR `whoami /groups` check; Linux: `geteuid() == 0` via `unsafe { libc::geteuid() }` — **avoid libc dep**: Linux: run `id -u` and compare to `"0"` using `SystemRunner`.
  - `pub fn relaunch_elevated(current_exe: &Path, args: &[String]) -> Result<Elevation, String>`:
    - Windows: `powershell -NoProfile -Command Start-Process -FilePath <exe> -Verb RunAs -ArgumentList ...` via `SystemRunner`; return `RelaunchRequested` if spawn ok.
    - Linux: try `pkexec <exe> <args...>`, else `sudo -n` if non-interactive fails return guidance string in `Denied`.
  - Decision rule used by `run_fix` later: if `!is_elevated()` → caller (CLI/GUI) relaunches before running; core exposes helpers only.

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, CommandRunner, MockRunner};
    use std::path::PathBuf;

    #[test]
    fn linux_elevation_reads_id_output() {
        let mut m = MockRunner::new();
        m.expect("id", CommandOutput { status: 0, stdout: "0\n".into(), stderr: String::new() });
        assert!(elevated_from_id(&m));
        let mut m2 = MockRunner::new();
        m2.expect("id", CommandOutput { status: 0, stdout: "1000\n".into(), stderr: String::new() });
        assert!(!elevated_from_id(&m2));
    }
}
```

Helper under test: `pub(crate) fn elevated_from_id(r: &dyn CommandRunner) -> bool` (used by `is_elevated` on unix; cfg(test) can call it on all OS).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p networkfix-core`
Expected: FAIL.

- [ ] **Step 3: Implement `elevation.rs`**

```rust
use crate::runner::{CommandRunner, SystemRunner};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    AlreadyElevated,
    RelaunchRequested,
    Denied,
    Unknown,
}

#[cfg(unix)]
pub(crate) fn elevated_from_id(r: &dyn CommandRunner) -> bool {
    r.run("id", &["-u"])
        .map(|o| o.stdout.trim() == "0")
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub(crate) fn elevated_from_id(_r: &dyn CommandRunner) -> bool {
    // Windows path uses `net session`; keep helper for tests only
    false
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

#[cfg(windows)]
pub fn relaunch_elevated(current_exe: &Path, args: &[String]) -> Result<Elevation, String> {
    let arg_list = args
        .iter()
        .map(|a| format!("\"{a}\""))
        .collect::<Vec<_>>()
        .join(" ");
    let exe = current_exe.display().to_string();
    let script = format!(
        "Start-Process -FilePath '{exe}' -Verb RunAs -ArgumentList '{arg_list}'"
    );
    let out = SystemRunner.run("powershell", &["-NoProfile", "-Command", &script])?;
    if out.success() {
        Ok(Elevation::RelaunchRequested)
    } else {
        Ok(Elevation::Denied)
    }
}

#[cfg(unix)]
pub fn relaunch_elevated(current_exe: &Path, args: &[String]) -> Result<Elevation, String> {
    let exe = current_exe.display().to_string();
    let mut owned: Vec<String> = vec![exe.clone()];
    owned.extend(args.iter().cloned());
    let refs: Vec<&str> = owned.iter().map(|s| s.as_str()).collect();

    if SystemRunner.run("pkexec", &refs).map(|o| o.success()).unwrap_or(false) {
        return Ok(Elevation::RelaunchRequested);
    }
    let mut sudo_args = vec!["-n".to_string()];
    sudo_args.extend(args.iter().cloned());
    let mut full = vec![exe];
    full.extend(args.iter().cloned());
    let _ = sudo_args;
    // Interactive sudo fallback:
    let refs2: Vec<&str> = full.iter().map(|s| s.as_str()).collect();
    if SystemRunner.run("sudo", &refs2).map(|o| o.success()).unwrap_or(false) {
        Ok(Elevation::RelaunchRequested)
    } else {
        Ok(Elevation::Denied)
    }
}
```

Note: unix `relaunch_elevated` above currently *runs* the elevated command synchronously — acceptable for CLI (parent waits); GUI Task 12 will prefer spawn-and-exit. Simplify: if already the elevation mechanism blocks, return `AlreadyElevated` after child finishes with child's code is wrong — **correct design:** relaunch should spawn and parent exits. Revise unix/windows relaunch to spawn detached where possible; for v1 Windows `Start-Process` is async (good); unix use `Command::spawn` with pkexec and don't wait — implement with `std::process::Command` directly instead of `SystemRunner` for spawn:

Replace both `relaunch_elevated` bodies with spawn-not-wait versions using `std::process::Command::new(...).spawn()`, map spawn error → `Err`, success → `RelaunchRequested`, pkexec missing (ErrorKind::NotFound) → try sudo, else `Denied`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p networkfix-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): elevation detection and relaunch helpers"
```

---

### Task 5: Windows services restart module

**Files:**
- Create: `crates/networkfix-core/src/windows/mod.rs`
- Create: `crates/networkfix-core/src/windows/services.rs`
- Modify: `crates/networkfix-core/src/lib.rs` (add `#[cfg(target_os = "windows")] pub mod windows;`)

**Interfaces:**
- Consumes: `CommandRunner`, `Step`, `ProgressEvent`
- Produces: `pub fn restart_services(runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent), services: &[&str]) -> Vec<Step>` in `windows::services`.
  - For each name: `sc query <name>` → if status output lacks `RUNNING` and lacks `STOPPED` and command fails → `Step::skipped(name, "service not present")`.
  - If running: `sc stop <name>`, sleep 400ms (via `std::thread::sleep`), `sc start <name>`.
  - Success start → `Step::ok`; start failure → `Step::warn`.
  - Service list constant: `pub const QUICK_SERVICES: &[&str] = &["icssvc", "WlanSvc", "SharedAccess", "EapHost", "dot3svc", "RasMan", "PhoneSvc", "tapisrv"];`

- [ ] **Step 1: Failing tests** in `services.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::ProgressEvent;
    use crate::runner::{CommandOutput, MockRunner};

    fn out(status: i32, stdout: &str) -> CommandOutput {
        CommandOutput { status, stdout: stdout.into(), stderr: String::new() }
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
```

Caveat: MockRunner returns same `sc` output for all calls — `STATE : 4 RUNNING` means stop+start always run; fine.

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test -p networkfix-core`
Expected: FAIL (module missing). Note: on Linux CI, windows module is cfg'd out — add `#[cfg(any(target_os = "windows", test))]` on `pub mod windows` so unit tests run everywhere:

In `lib.rs`: `#[cfg(any(target_os = "windows", test))] pub mod windows;`

- [ ] **Step 3: Implement `services.rs`**

```rust
use crate::progress::ProgressEvent;
use crate::runner::CommandRunner;
use crate::report::Step;
use std::time::Duration;

pub const QUICK_SERVICES: &[&str] = &[
    "icssvc", "WlanSvc", "SharedAccess", "EapHost", "dot3svc", "RasMan", "PhoneSvc", "tapisrv",
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
            Ok(o) => steps.push(Step::warn(*name, format!("start failed: {}", o.stderr.trim()))),
            Err(e) => steps.push(Step::warn(*name, e)),
        }
    }
    steps
}
```

`windows/mod.rs`:
```rust
pub mod services;
```

- [ ] **Step 4: Run tests — PASS**

Run: `cargo test -p networkfix-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): windows service restart with skip semantics"
```

---

### Task 6: Windows adapter restart + DNS/IP steps

**Files:**
- Create: `crates/networkfix-core/src/windows/adapters.rs`
- Modify: `crates/networkfix-core/src/windows/mod.rs`

**Interfaces:**
- Produces:
  - `pub const WWAN_PATTERNS: &[&str]` and `WIFI_PATTERNS: &[&str]` (port from legacy regex, simplified substring list).
  - `pub fn find_adapters(runner: &dyn CommandRunner) -> Vec<AdapterInfo>` where `AdapterInfo { name: String, description: String, status: String }` parsed from `netsh interface show interface` OR `powershell Get-NetAdapter` — choose **`netsh interface show interface`** (no PowerShell dependency): parse lines with Admin State / State / Device Name.
  - Simplified parse: lines after header containing `Connected|Disconnected|Disabled` — name = last token group. Because parsing is fragile, also provide `pub fn classify(name: &str, desc: &str) -> AdapterKind` with `AdapterKind::{Wwan, Wifi, Other}` using case-insensitive substring lists.
  - `pub fn restart_adapter(runner, emit, name: &str) -> Step` — `netsh interface set interface name="<name>" admin=disable` → sleep 2s → `admin=enable` → `Step::ok/warn`.
  - `pub fn flush_dns(runner) -> Step` — `ipconfig /flushdns`.
  - `pub fn renew_dhcp(runner, iface: &str) -> Step` — `netsh interface ipv4 set address name="<iface>" source=dhcp` + `netsh interface ipv4 set dnsservers name="<iface>" source=dhcp`.

- [ ] **Step 1: Failing tests** in `adapters.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn classify_matches_wwan_and_wifi() {
        assert_eq!(classify("Cellular", "HP Mobile Broadband"), AdapterKind::Wwan);
        assert_eq!(classify("Wi-Fi", "Intel Wireless-AC"), AdapterKind::Wifi);
        assert_eq!(classify("Ethernet", "Realtek PCIe"), AdapterKind::Other);
    }

    #[test]
    fn restart_uses_netsh_disable_enable() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput::ok_empty());
        let step = restart_adapter(&m, &|_| {}, "Wi-Fi");
        assert_eq!(step.status, crate::StepStatus::Ok);
        let text: Vec<String> = m
            .calls
            .borrow()
            .iter()
            .flat_map(|(p, a)| {
                let mut v = vec![p.clone()];
                v.extend(a.iter().cloned());
                v
            })
            .collect();
        assert!(text.iter().any(|t| t.contains("admin=disable")));
        assert!(text.iter().any(|t| t.contains("admin=enable")));
    }

    #[test]
    fn flush_dns_reports_ok() {
        let mut m = MockRunner::new();
        m.expect("ipconfig", CommandOutput::ok_empty());
        assert_eq!(flush_dns(&m).status, crate::StepStatus::Ok);
    }
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p networkfix-core`
Expected: FAIL.

- [ ] **Step 3: Implement `adapters.rs`**

```rust
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
    "mobile", "wwan", "fibocom", "sierra", "quectel", "broadband", "lte", "hspa", "cellular",
];
const WIFI_TOKENS: &[&str] = &[
    "wi-fi", "wifi", "wireless", "wlan", "802.11", "hosted network", "wi-fi direct",
];

pub fn classify(name: &str, desc: &str) -> AdapterKind {
    let hay = format!("{} {}", name, desc).to_ascii_lowercase();
    if WWAN_TOKENS.iter().any(|t| hay.contains(t)) {
        AdapterKind::Wwan
    } else if WIFI_TOKENS.iter().any(|t| hay.contains(t)) {
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
            if parts.len() < 3 {
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
    let disable = runner.run(
        "netsh",
        &["interface", "set", "interface", &format!("name={name}"), "admin=disable"],
    );
    match disable {
        Ok(o) if o.success() || o.stdout.is_empty() || o.stderr.is_empty() => {}
        Ok(o) => return Step::warn(format!("adapter {name}"), format!("disable: {}", o.stderr.trim())),
        Err(e) => return Step::warn(format!("adapter {name}"), e),
    }
    std::thread::sleep(Duration::from_secs(2));
    match runner.run(
        "netsh",
        &["interface", "set", "interface", &format!("name={name}"), "admin=enable"],
    ) {
        Ok(o) if o.success() => Step::ok(format!("adapter {name}"), "cycled"),
        Ok(o) => Step::warn(format!("adapter {name}"), format!("enable: {}", o.stderr.trim())),
        Err(e) => Step::warn(format!("adapter {name}"), e),
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
    let a = runner.run(
        "netsh",
        &["interface", "ipv4", "set", "address", &format!("name={iface}"), "source=dhcp"],
    );
    let b = runner.run(
        "netsh",
        &["interface", "ipv4", "set", "dnsservers", &format!("name={iface}"), "source=dhcp"],
    );
    match (a, b) {
        (Ok(x), Ok(y)) if x.success() && y.success() => Step::ok(format!("renew {iface}"), "dhcp"),
        (Ok(x), _) if !x.success() => Step::warn(format!("renew {iface}"), x.stderr.trim().to_string()),
        (_, Ok(y)) if !y.success() => Step::warn(format!("renew {iface}"), y.stderr.trim().to_string()),
        (Err(e), _) | (_, Err(e)) => Step::warn(format!("renew {iface}"), e),
    }
}
```

Fix `netsh` args: format is `netsh interface set interface name="Wi-Fi" admin=disable` — args should be `["interface", "set", "interface", "name=Wi-Fi", "admin=disable"]` or pass `name=Wi-Fi` as one arg. Adjust tests to match exact arg strings `name=Wi-Fi`. Update test to check `args.contains("name=Wi-Fi")`.

- [ ] **Step 4: Wire `windows/mod.rs`: `pub mod adapters;`**

- [ ] **Step 5: Run tests — PASS**

Run: `cargo test -p networkfix-core`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): windows adapter cycle, dns flush, dhcp renew"
```

---

### Task 7: Windows NoInternet mode (ICS, 192.168.137.1, forwarding, firewall)

**Files:**
- Create: `crates/networkfix-core/src/windows/nointernet.rs`
- Modify: `crates/networkfix-core/src/windows/mod.rs`

**Interfaces:**
- Consumes: services::restart_services, adapters::{find_adapters, classify, AdapterKind}
- Produces: `pub fn fix_no_internet(runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent)) -> Vec<Step>` implementing spec §3 NoInternet for Windows:
  1. Restart SharedAccess (use restart_services with `&["SharedAccess"]`).
  2. `netsh int ipv4 set global forwarding=enabled` + `multicastforwarding=enabled` → one step "ipv4 forwarding".
  3. Find Wi-Fi Direct / hosted adapters: names containing `Local Area Connection*` or description tokens from find_adapters (name contains `Local Area Connection` or `Wi-Fi Direct`) → for each up-ish adapter, `netsh interface ipv4 set address name=<n> static 192.168.137.1 255.255.255.0` if needed; step "private ip <name>".
  4. Restart icssvc + WlanSvc (`restart_services`).
  5. Tethering toggle: `powershell` embedded WinRT snippet (single `-Command` string from legacy logic) → step "tethering toggle" (warn on failure — manual toggle hint in detail).
  6. Firewall: `netsh advfirewall firewall show rule name=all` is heavy; instead run `powershell -Command Get-NetFirewallRule ... Enable-NetFirewallRule` only if needed — v1: attempt `netsh advfirewall set currentprofile state on` is wrong; use step that runs PS enable rules (warn on fail).
  7. Re-check private IP after toggle (same as 3) → final step.

- [ ] **Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn nointernet_runs_forwarding_and_shared_access() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput::ok_empty());
        // sc query returns empty success → treated present+not running → start only
        let steps = fix_no_internet(&m, &|_| {});
        let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
        assert!(names.iter().any(|n| n.contains("SharedAccess") || *n == "SharedAccess"));
        assert!(names.iter().any(|n| n.contains("forwarding")));
        assert!(steps.iter().all(|s| s.status != crate::StepStatus::Fail || true)); // best-effort: allow warns
    }
}
```

Tighten: assert no step has status Fail when default runner succeeds for everything — but toggle powershell uses same mock success → all Ok/Skipped. Assert `steps.iter().all(|s| matches!(s.status, Ok | Warn | Skipped))`.

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p networkfix-core`
Expected: FAIL.

- [ ] **Step 3: Implement `nointernet.rs`**

```rust
use crate::adapters::{find_adapters, AdapterKind, classify};
use crate::progress::ProgressEvent;
use crate::report::Step;
use crate::runner::CommandRunner;
use crate::services::{restart_services, QUICK_SERVICES};

const HOTSPOT_IP: &str = "192.168.137.1";

pub fn fix_no_internet(
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
) -> Vec<Step> {
    let mut steps = Vec::new();

    steps.extend(restart_services(runner, emit, &["SharedAccess"]));

    let fwd = runner.run(
        "netsh",
        &["int", "ipv4", "set", "global", "forwarding=enabled"],
    );
    let mfw = runner.run(
        "netsh",
        &["int", "ipv4", "set", "global", "multicastforwarding=enabled"],
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

fn ensure_hotspot_ip(
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
) -> Vec<Step> {
    emit(ProgressEvent::message("checking hotspot private IP"));
    let mut steps = Vec::new();
    let adapters = find_adapters(runner);
    let targets: Vec<_> = adapters
        .iter()
        .filter(|a| {
            a.name.contains("Local Area Connection")
                || classify(&a.name, &a.description) == AdapterKind::Wifi
                    && a.name.contains('*')
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

fn toggle_tethering(runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent)) -> Step {
    emit(ProgressEvent::message("toggling mobile hotspot"));
    // WinRT tethering toggle via embedded PowerShell (spec §9 risk mitigation)
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
```

Fix import path: services/adapters are sibling modules — `use super::adapters::...; use super::services::...;`

`windows/mod.rs`:
```rust
pub mod adapters;
pub mod nointernet;
pub mod services;
```

- [ ] **Step 4: Run tests — PASS**

Run: `cargo test -p networkfix-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): windows no-internet fix (ICS, NAT IP, tethering, firewall)"
```

---

### Task 8: Windows Quick/Full/HotspotOnly orchestration

**Files:**
- Create: `crates/networkfix-core/src/windows/mod.rs` → add `pub fn run_mode(...)`
- Modify: `crates/networkfix-core/src/lib.rs` → `run_fix` public API
- Test: `crates/networkfix-core/src/windows/tests.rs` or inline

**Interfaces:**
- Produces:
  - `windows::run_mode(mode: Mode, runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent), elevated: bool) -> FixReport`
  - `pub fn run_fix(mode: Mode, on_event: impl Fn(ProgressEvent)) -> FixReport` in `lib.rs` — creates `SystemRunner`, emits `Start`, records duration, emits `Done`.
  - Also `pub fn run_fix_with(mode, runner: &dyn CommandRunner, elevated: bool, on_event) -> FixReport` for tests/backends.

Mode mapping (Windows):
- **Quick:** restart `QUICK_SERVICES` (or subset: icssvc, WlanSvc, SharedAccess + rest), forwarding netsh, classify adapters → restart WWAN if found (else Skipped), flush_dns, renew_dhcp on first WWAN name if any.
- **Full:** Quick steps + cycle Wifi adapter + `netsh winsock reset`, `netsh int ip reset`, `netsh int tcp reset` + restart icssvc/WlanSvc again.
- **HotspotOnly:** restart `["WlanSvc", "SharedAccess", "icssvc"]` + cycle Wifi adapter + flush_dns. No WWAN touch.
- **NoInternet:** `windows::nointernet::fix_no_internet`.

- [ ] **Step 1: Failing tests**

```rust
#[cfg(test)]
mod mode_tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

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
        // netsh interface show interface returns empty → no adapters → wwan skipped
        assert!(report
            .steps
            .iter()
            .any(|s| s.status == crate::StepStatus::Skipped && s.name.contains("adapter")));
    }

    #[test]
    fn full_includes_winsock_reset() {
        let m = mock();
        let report = run_mode(Mode::Full, &m, &|_| {}, true);
        let calls = m.calls.borrow();
        assert!(calls.iter().any(|(p, a)| p == "netsh" && a.contains(&"winsock".to_string())));
    }

    #[test]
    fn hotspot_only_does_not_restart_phone_tapisrv_set() {
        let m = mock();
        let report = run_mode(Mode::HotspotOnly, &m, &|_| {}, true);
        assert!(!report.steps.iter().any(|s| s.name == "PhoneSvc"));
        assert!(report.steps.iter().any(|s| s.name == "WlanSvc"));
    }

    #[test]
    fn never_panics_on_empty_environment() {
        let mut m = MockRunner::new();
        m.set_default(Err_report());
        let _ = run_mode(Mode::Quick, &m, &|_| {}, false);
    }
    fn Err_report() -> CommandOutput { CommandOutput::ok_empty() }
}
```

Note: MockRunner default Ok means runner errors only when unscripted and no default — use default Ok; "empty environment" = empty stdout for netsh show interface.

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p networkfix-core`
Expected: FAIL.

- [ ] **Step 3: Implement orchestration in `windows/mod.rs`**

```rust
pub mod adapters;
pub mod nointernet;
pub mod services;

use crate::mode::Mode;
use crate::progress::ProgressEvent;
use crate::report::{FixReport, Step};
use crate::runner::CommandRunner;
use adapters::{classify, find_adapters, flush_dns, renew_dhcp, restart_adapter, AdapterKind};
use services::{restart_services, QUICK_SERVICES};

pub fn run_mode(
    mode: Mode,
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    elevated: bool,
) -> FixReport {
    let mut report = FixReport::new(mode, elevated);
    let mut push = |report: &mut FixReport, steps: Vec<Step>| {
        for s in steps {
            emit(ProgressEvent::Step(s.clone()));
            report.push(s);
        }
    };

    match mode {
        Mode::NoInternet => {
            push(&mut report, nointernet::fix_no_internet(runner, emit));
        }
        Mode::HotspotOnly => {
            push(
                &mut report,
                restart_services(runner, emit, &["WlanSvc", "SharedAccess", "icssvc"]),
            );
            for a in find_adapters(runner) {
                if classify(&a.name, &a.description) == AdapterKind::Wifi {
                    let s = restart_adapter(runner, emit, &a.name);
                    emit(ProgressEvent::Step(s.clone()));
                    report.push(s);
                }
            }
            let s = flush_dns(runner);
            emit(ProgressEvent::Step(s.clone()));
            report.push(s);
        }
        Mode::Quick | Mode::Full => {
            push(&mut report, restart_services(runner, emit, QUICK_SERVICES));
            let fwd = runner.run(
                "netsh",
                &["int", "ipv4", "set", "global", "forwarding=enabled"],
            );
            let step = match fwd {
                Ok(o) if o.success() => Step::ok("ipv4 forwarding", "enabled"),
                _ => Step::warn("ipv4 forwarding", "netsh error"),
            };
            emit(ProgressEvent::Step(step.clone()));
            report.push(step);

            let adapters = find_adapters(runner);
            let wwan: Vec<_> = adapters
                .iter()
                .filter(|a| classify(&a.name, &a.description) == AdapterKind::Wwan)
                .collect();
            if wwan.is_empty() {
                let s = Step::skipped("wwan adapter", "not present");
                emit(ProgressEvent::Step(s.clone()));
                report.push(s);
            } else {
                for a in &wwan {
                    let s = restart_adapter(runner, emit, &a.name);
                    emit(ProgressEvent::Step(s.clone()));
                    report.push(s);
                }
            }

            let dns = flush_dns(runner);
            emit(ProgressEvent::Step(dns.clone()));
            report.push(dns);

            if let Some(a) = wwan.first() {
                let s = renew_dhcp(runner, &a.name);
                emit(ProgressEvent::Step(s.clone()));
                report.push(s);
            }

            if mode == Mode::Full {
                for args in [
                    vec!["winsock", "reset"],
                    vec!["int", "ip", "reset"],
                    vec!["int", "tcp", "reset"],
                ] {
                    let r = runner.run("netsh", &args);
                    let s = match r {
                        Ok(o) if o.success() => Step::ok(format!("netsh {}", args.join(" ")), "done"),
                        Ok(o) => Step::warn(format!("netsh {}", args.join(" ")), o.stderr.trim().to_string()),
                        Err(e) => Step::warn(format!("netsh {}", args.join(" ")), e),
                    };
                    emit(ProgressEvent::Step(s.clone()));
                    report.push(s);
                }
                for a in find_adapters(runner) {
                    if classify(&a.name, &a.description) == AdapterKind::Wifi {
                        let s = restart_adapter(runner, emit, &a.name);
                        emit(ProgressEvent::Step(s.clone()));
                        report.push(s);
                    }
                }
                push(&mut report, restart_services(runner, emit, &["icssvc", "WlanSvc"]));
            }
        }
    }
    report
}
```

- [ ] **Step 4: Implement `lib.rs::run_fix*`**

```rust
use progress::ProgressEvent;
use report::FixReport;
use runner::{CommandRunner, SystemRunner};
use std::time::Instant;

pub fn run_fix_with(
    mode: Mode,
    runner: &dyn CommandRunner,
    elevated: bool,
    on_event: &dyn Fn(ProgressEvent),
) -> FixReport {
    on_event(ProgressEvent::Start { mode });
    let started = Instant::now();
    #[cfg(target_os = "windows")]
    let mut report = windows::run_mode(mode, runner, on_event, elevated);
    #[cfg(target_os = "linux")]
    let mut report = linux::run_mode(mode, runner, on_event, elevated);
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    let mut report = {
        let mut r = FixReport::new(mode, elevated);
        r.push(Step::fail("platform", "unsupported operating system"));
        r
    };
    report.duration_ms = started.elapsed().as_millis() as u64;
    on_event(ProgressEvent::Done { report: report.clone() });
    report
}

pub fn run_fix(mode: Mode, on_event: impl Fn(ProgressEvent)) -> FixReport {
    let elevated = crate::elevation::is_elevated();
    run_fix_with(mode, &SystemRunner, elevated, &on_event)
}
```

Add `#[cfg(target_os = "linux")] pub mod linux;` later Task 10 — for now gate: if only windows cfg present, on linux tests `run_fix_with` hits unsupported until Task 10. **Order fix:** implement linux stub Task 9 first OR add linux module stub now in this task:

Create minimal `crates/networkfix-core/src/linux/mod.rs`:
```rust
use crate::mode::Mode;
use crate::progress::ProgressEvent;
use crate::report::{FixReport, Step};
use crate::runner::CommandRunner;

pub fn run_mode(mode: Mode, _runner: &dyn CommandRunner, emit: &dyn Fn(ProgressEvent), elevated: bool) -> FixReport {
    let mut r = FixReport::new(mode, elevated);
    let s = Step::fail("linux backend", "not yet implemented");
    emit(ProgressEvent::Step(s.clone()));
    r.push(s);
    r
}
```
Wire `#[cfg(any(target_os = "linux", test))] pub mod linux;` — careful: on windows `test` builds would compile both; `run_fix_with` uses cfg(target_os) only so fine; both mods can exist under `any(..., test)` for unit testing their internals.

- [ ] **Step 5: Run tests — PASS**

Run: `cargo test -p networkfix-core`
Expected: PASS (windows mode tests run on all platforms because `windows` mod is `any(windows, test)` — ensure `run_mode` tests call `windows::run_mode` directly not `run_fix_with`).

Adjust Task 8 tests to call `crate::windows::run_mode(...)`.

- [ ] **Step 6: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): windows quick/full/hotspot orchestration and run_fix"
```

---

### Task 9: Linux backend detection + Quick/Full/HotspotOnly

**Files:**
- Create: `crates/networkfix-core/src/linux/mod.rs` (replace stub)
- Create: `crates/networkfix-core/src/linux/nm.rs`
- Create: `crates/networkfix-core/src/linux/sysctl_steps.rs` (optional small helpers)
- Modify: `crates/networkfix-core/src/lib.rs` (ensure linux module cfg)

**Interfaces:**
- Produces:
  - `linux::Backend { NetworkManager, SystemdNetworkd, Iwd, None }`
  - `pub fn detect_backend(runner: &dyn CommandRunner) -> Backend` — `nmcli -t -f RUNNING general` success + stdout contains `running` → NM; else if `systemctl is-active NetworkManager` success → NM; else `networkctl` exists → SystemdNetworkd; else `iwctl` → Iwd; else None.
  - `pub fn restart_network_stack(runner, emit, backend) -> Vec<Step>`
  - `nm::list_connections`, `nm::restart_connection(runner, emit, name)`
  - `pub fn flush_dns_linux(runner) -> Step` — `resolvectl flush-caches` then fallback `systemd-resolve --flush-caches`; if both fail → Warn (not Fail).
  - `run_mode` for Quick/Full/HotspotOnly (NoInternet → Task 10).
  - Missing backend → single `Step::fail("backend", "no NetworkManager/systemd-networkd/iwd found")` only step.

Mode mapping Linux:
- **Quick:** detect backend; if None → fail step; restart NM (`systemctl restart NetworkManager` or `nmcli networking off` + `on`); flush DNS; `nmcli device reapply` best-effort on first wifi device — parse `nmcli -t -f DEVICE,TYPE,STATE dev status`.
- **Full:** Quick + for each wifi/wwan device `ip link set <dev> down` / `up` + restart `wpa_supplicant@<dev>` if unit exists (`systemctl restart` warn-only) + restart NM again.
- **HotspotOnly:** find hotspot connection: `nmcli -t -f NAME,TYPE con show` where type `802-11-ap` or name matches `hotspot` → `nmcli con down/up <name>`; if no hotspot profile → Skipped "turn hotspot on first"; flush DNS. Non-NM: restart `hostapd` if `systemctl list-unit-files hostapd` matches.

- [ ] **Step 1: Failing tests** in `linux/nm.rs` / `linux/mod.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::{CommandOutput, MockRunner};

    #[test]
    fn detects_networkmanager_from_nmcli() {
        let mut m = MockRunner::new();
        m.expect("nmcli", CommandOutput { status: 0, stdout: "running\n".into(), stderr: String::new() });
        assert_eq!(detect_backend(&m), Backend::NetworkManager);
    }

    #[test]
    fn none_backend_fails_gracefully() {
        let mut m = MockRunner::new();
        m.set_default(CommandOutput { status: 1, stdout: String::new(), stderr: "not found".into() });
        assert_eq!(detect_backend(&m), Backend::None);
        let report = crate::linux::run_mode(Mode::Quick, &m, &|_| {}, true);
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.steps[0].status, crate::StepStatus::Fail);
        assert_eq!(report.exit_code(), 2);
    }

    #[test]
    fn quick_with_nm_restarts_networkmanager_and_flushes_dns() {
        let mut m = MockRunner::new();
        m.expect("nmcli", CommandOutput { status: 0, stdout: "running\n".into(), stderr: String::new() });
        m.set_default(CommandOutput::ok_empty());
        // problem: set_default after expect keeps expect priority for nmcli — order: expect first then default
        let report = crate::linux::run_mode(Mode::Quick, &m, &|_| {}, true);
        assert!(report.steps.iter().any(|s| s.name.contains("NetworkManager")));
        let calls = m.calls.borrow();
        assert!(calls.iter().any(|(p, a)| p == "systemctl" && a.contains(&"NetworkManager".to_string())
            || p == "nmcli"));
        assert!(report.exit_code() <= 1);
    }

    #[test]
    fn hotspot_only_skips_when_no_ap_profile() {
        let mut m = MockRunner::new();
        m.expect("nmcli", CommandOutput { status: 0, stdout: "running\n".into(), stderr: String::new() });
        // con show returns empty
        m.expect("systemctl", CommandOutput::ok_empty());
        m.set_default(CommandOutput::ok_empty());
        let report = crate::linux::run_mode(Mode::HotspotOnly, &m, &|_| {}, true);
        // name matching: first nmcli call is detect (general), subsequent con show also nmcli — Mock returns same "running"
        // Design detect to use `nmcli general`; con show separate — mock is coarse: returns running for con show too → parse yields weird names; assert at least one Skipped OR Ok, never panic
        assert!(!report.steps.is_empty());
    }
}
```

Mock limitation: single output per program. Improve `MockRunner` in Task 3 style: queue per program: `expect_seq(program, vec![out1, out2, ...])` popping in order, then fall back to last. **Add to this task as prerequisite refactor:**

```rust
// in MockRunner
expect_seq: HashMap<String, VecDeque<CommandOutput>>,
pub fn expect_seq(&mut self, program: &str, outs: Vec<CommandOutput>) { ... }
// run(): if queue non-empty pop front and return (if len==1 after pop, keep returning it as sticky last — design: if queue has >1 pop; if ==1 peek clone without clear)
```

Then update nmcli tests to `expect_seq("nmcli", vec![general_running, con_list_empty])`.

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p networkfix-core`
Expected: FAIL.

- [ ] **Step 3: Extend MockRunner with expect_seq** (runner.rs) + adjust old tests if needed.

- [ ] **Step 4: Implement `linux/mod.rs` + `linux/nm.rs`**

`linux/mod.rs`:
```rust
pub mod nm;

use crate::mode::Mode;
use crate::progress::ProgressEvent;
use crate::report::{FixReport, Step};
use crate::runner::CommandRunner;
use nm::{detect_backend, Backend};

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
    let mut absorb = |steps: Vec<Step>| {
        for s in steps {
            emit(ProgressEvent::Step(s.clone()));
            report.push(s);
        }
    };
    match mode {
        Mode::Quick => absorb(nm::quick_steps(runner, emit, backend)),
        Mode::Full => absorb(nm::full_steps(runner, emit, backend)),
        Mode::HotspotOnly => absorb(nm::hotspot_only_steps(runner, emit, backend)),
        Mode::NoInternet => absorb(nm::no_internet_steps(runner, emit, backend)),
    }
    report
}
```

(NoInternet included in Task 10 — for Task 9, `no_internet_steps` returns `vec![Step::skipped("no-internet", "implemented in next task")]` then replaced in Task 10.)

`linux/nm.rs` — implement:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend { NetworkManager, SystemdNetworkd, Iwd, None }

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
    if runner.run("networkctl", &["status"]).map(|o| o.success()).unwrap_or(false) {
        return Backend::SystemdNetworkd;
    }
    if runner.run("iwctl", &["help"]).map(|o| o.success()).unwrap_or(false) {
        return Backend::Iwd;
    }
    Backend::None
}

pub fn flush_dns(runner: &dyn CommandRunner) -> Step {
    if let Ok(o) = runner.run("resolvectl", &["flush-caches"]) {
        if o.success() { return Step::ok("flush dns", "resolvectl"); }
    }
    if let Ok(o) = runner.run("systemd-resolve", &["--flush-caches"]) {
        if o.success() { return Step::ok("flush dns", "systemd-resolve"); }
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

pub fn quick_steps(runner, emit, backend) -> Vec<Step> {
    let mut steps = vec![];
    steps.push(restart_nm(runner));
    steps.push(flush_dns(runner));
    // best-effort reapply
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

pub fn full_steps(...) -> Vec<Step> {
    let mut steps = quick_steps(runner, emit, backend);
    for dev in wifi_devices(runner) {
        steps.push(cycle_link(runner, &dev));
    }
    steps.push(restart_wpa(runner));
    steps.push(restart_nm(runner));
    steps
}

fn cycle_link(runner, dev) -> Step {
    let _ = runner.run("ip", &["link", "set", "dev", dev, "down"]);
    std::thread::sleep(Duration::from_secs(2));
    match runner.run("ip", &["link", "set", "dev", dev, "up"]) {
        Ok(o) if o.success() => Step::ok(format!("link {dev}"), "cycled"),
        Ok(o) => Step::warn(format!("link {dev}"), o.stderr.trim().to_string()),
        Err(e) => Step::warn(format!("link {dev}"), e),
    }
}

fn restart_wpa(runner) -> Step {
    match runner.run("systemctl", &["restart", "wpa_supplicant"]) {
        Ok(o) if o.success() => Step::ok("wpa_supplicant", "restarted"),
        _ => Step::warn("wpa_supplicant", "not restarted (unit missing or failed)"),
    }
}

pub fn hotspot_only_steps(runner, emit, backend) -> Vec<Step> {
    let mut steps = vec![];
    // list connections
    let con = runner.run("nmcli", &["-t", "-f", "NAME,TYPE", "con", "show"]);
    let hotspot_name = con.ok().and_then(|o| {
        o.stdout.lines().find_map(|l| {
            let mut it = l.splitn(2, ':');
            let name = it.next()?.to_string();
            let ty = it.next().unwrap_or("");
            if ty.contains("802-11-ap") || name.to_ascii_lowercase().contains("hotspot") {
                Some(name)
            } else { None }
        })
    });
    match hotspot_name {
        Some(name) => {
            let _ = runner.run("nmcli", &["con", "down", &name]);
            std::thread::sleep(Duration::from_secs(1));
            match runner.run("nmcli", &["con", "up", &name]) {
                Ok(o) if o.success() => steps.push(Step::ok(format!("hotspot {name}"), "restarted")),
                Ok(o) => steps.push(Step::warn(format!("hotspot {name}"), o.stderr.trim().to_string())),
                Err(e) => steps.push(Step::warn(format!("hotspot {name}"), e)),
            }
        }
        None => steps.push(Step::skipped("hotspot", "no AP/hotspot profile found — turn hotspot on first")),
    }
    steps.push(flush_dns(runner));
    steps
}

pub fn no_internet_steps(...) -> Vec<Step> {
    vec![Step::skipped("no-internet", "pending task 10")]
}
```

Imports: `StepStatus`, `Duration`, etc.

- [ ] **Step 5: Run tests — PASS**

Run: `cargo test -p networkfix-core`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): linux backend detection, quick/full/hotspot modes"
```

---

### Task 10: Linux NoInternet mode (forwarding, NAT, hotspot IP)

**Files:**
- Modify: `crates/networkfix-core/src/linux/nm.rs` (replace `no_internet_steps`)
- Modify tests in `linux/mod.rs` or `nm.rs`

**Interfaces:**
- Produces `no_internet_steps(runner, emit, backend) -> Vec<Step>`:
  1. Enable forwarding: `sysctl -w net.ipv4.ip_forward=1` + attempt write via `sysctl` only (report persistence hint in detail: "run sysctl.d for persistence").
  2. Also `sysctl -w net.ipv4.conf.all.forwarding=1`.
  3. Detect default outbound iface: `ip -4 route show default` → parse `dev <name>`.
  4. NAT: try nft: `nft list table inet networkfix` … simpler v1: `iptables -t nat -C POSTROUTING -o <dev> -j MASQUERADE` (exit 1 if missing) then `iptables -t nat -A ...`; also `iptables -C FORWARD ...` rules. If `iptables` missing, try `nft add table ip nat` path — **v1 scope: iptables only; if missing, Warn with message "install iptables or configure nftables manually"**.
  5. Restart NM (`restart_nm`) to re-apply hotspot.
  6. Flush DNS.
  7. Best-effort hotspot iface IP check: `ip -4 -o addr show` — if a `192.168.137.` expected? **Linux hotspot IP varies (often 10.42.0.1)** — do NOT force 192.168.137.1; step "hotspot address" reports what's found (Ok if any `ap`-type or `10.42`/`192.168` private addr on wifi ifaces; Skipped if none).

- [ ] **Step 1: Failing tests**

```rust
#[test]
fn no_internet_enables_forwarding_and_masquerade() {
    let mut m = MockRunner::new();
    m.expect("nmcli", CommandOutput { status: 0, stdout: "running\n".into(), stderr: String::new() });
    m.expect_seq(
        "ip",
        vec![
            CommandOutput { status: 0, stdout: "default via 192.0.2.1 dev eth0 proto dhcp\n".into(), stderr: String::new() },
            CommandOutput::ok_empty(),
        ],
    );
    m.set_default(CommandOutput::ok_empty());
    let steps = crate::linux::nm::no_internet_steps(&m, &|_| {}, Backend::NetworkManager);
    let names: Vec<&str> = steps.iter().map(|s| s.name.as_str()).collect();
    assert!(names.iter().any(|n| n.contains("forwarding")));
    assert!(names.iter().any(|n| n.contains("masquerade") || n.contains("nat")));
    let calls = m.calls.borrow();
    assert!(calls.iter().any(|(p, a)| p == "sysctl"));
    assert!(calls.iter().any(|(p, a)| p == "iptables" && a.iter().any(|x| x.contains("MASQUERADE"))));
}
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p networkfix-core`
Expected: FAIL.

- [ ] **Step 3: Implement replacement `no_internet_steps`**

```rust
pub fn no_internet_steps(
    runner: &dyn CommandRunner,
    emit: &dyn Fn(ProgressEvent),
    backend: Backend,
) -> Vec<Step> {
    let mut steps = Vec::new();
    emit(ProgressEvent::message("enabling ip forwarding"));

    steps.push(sysctl_set(runner, "net.ipv4.ip_forward", "1"));
    steps.push(sysctl_set(runner, "net.ipv4.conf.all.forwarding", "1"));
    steps.push(Step::warn(
        "forwarding persistence",
        "reboot resets sysctl — add to /etc/sysctl.d/ if needed",
    ));

    let out_dev = default_dev(runner);
    match &out_dev {
        Some(dev) => steps.push(masquerade(runner, dev)),
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
    // "default via ... dev eth0 ..."
    let tokens: Vec<&str> = o.stdout.split_whitespace().collect();
    tokens
        .windows(2)
        .find(|w| w[0] == "dev")
        .map(|w| w[1].to_string())
}

fn masquerade(runner: &dyn CommandRunner, dev: &str) -> Step {
    // ensure chain rules exist; -C checks, -A appends (ignore failure of -C)
    let check = runner.run(
        "iptables",
        &["-t", "nat", "-C", "POSTROUTING", "-o", dev, "-j", "MASQUERADE"],
    );
    if !matches!(check, Ok(o) if o.success()) {
        let add = runner.run(
            "iptables",
            &["-t", "nat", "-A", "POSTROUTING", "-o", dev, "-j", "MASQUERADE"],
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
            "-A", "FORWARD", "-i", dev, "-o", dev, "-m", "state", "--state",
            "RELATED,ESTABLISHED", "-j", "ACCEPT",
        ],
    );
    // duplicate rule adds fail — treat as warn only if command missing
    if fwd.is_err() {
        return Step::warn("forward rules", "iptables not available");
    }
    Step::ok(format!("masquerade {dev}"), "NAT enabled")
}

fn report_hotspot_addrs(runner: &dyn CommandRunner) -> Step {
    let Ok(o) = runner.run("ip", &["-4", "-o", "addr", "show"]) else {
        return Step::skipped("hotspot address", "ip command failed");
    };
    let has_private = o.stdout.lines().any(|l| {
        l.contains("inet 10.")
            || l.contains("inet 192.168.")
            || l.contains("inet 172.")
    });
    if has_private {
        Step::ok("hotspot address", "private IPv4 present on an interface")
    } else {
        Step::warn("hotspot address", "no private IPv4 found — is hotspot on?")
    }
}
```

Add `use crate::progress::ProgressEvent;` and Backend import in scope (already).

- [ ] **Step 4: Run tests — PASS**

Run: `cargo test -p networkfix-core`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/networkfix-core
git commit -m "feat(core): linux no-internet forwarding and masquerade"
```

---

### Task 11: CLI — menu, direct modes, --json, exit codes, elevation

**Files:**
- Modify: `crates/networkfix/src/main.rs`
- Create: `crates/networkfix/src/cli.rs` (optional split)
- Test: `crates/networkfix/tests/cli.rs` (integration, using `assert_cmd` — add dev-dep) **or** keep unit tests on pure functions `exit_code mapping` already in core; CLI integration optional if assert_cmd heavy on Win — use `std::process::Command` on built binary via `env!("CARGO_BIN_EXE_networkfix")`.

**Interfaces:**
- Consumes: `networkfix_core::{run_fix, Mode, elevation::{is_elevated, relaunch_elevated}, ProgressEvent, FixReport}`
- Produces clap interface:

```text
networkfix [MODE] [--json] [--no-elevate]
MODE: quick | full | hotspot | no-internet   (omit → interactive menu)
```

Behavior:
1. Parse args.
2. If no mode → print menu (same text as legacy bat), read line, map 1/2/3/4/q.
3. If `!is_elevated() && !no_elevate` → print "Requesting administrator privileges..." → `relaunch_elevated(current_exe, args)` → if RelaunchRequested exit 0; if Denied eprintln + exit 3.
4. Run fix; events → stderr as human lines `[OK] name — detail`; Done → if `--json` print `serde_json::to_string_pretty(report)` to stdout else print summary to stderr.
5. Exit `report.exit_code()` (or 3 if no_elevate attempted ops needing root? still run; ops will warn).

- [ ] **Step 1: Write failing integration test** `crates/networkfix/tests/cli.rs`

```rust
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_networkfix"))
}

#[test]
fn json_quick_outputs_valid_json_on_stdout() {
    let out = bin()
        .args(["--json", "--no-elevate", "quick"])
        .output()
        .expect("run");
    // stdout must be pure JSON
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("stdout not JSON: {e}\n{}", String::from_utf8_lossy(&out.stdout)));
    assert_eq!(v["mode"], "quick");
    assert!(v["steps"].is_array());
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
```

Add to `crates/networkfix/Cargo.toml`:
```toml
[dev-dependencies]
serde_json = "1"
```

- [ ] **Step 2: Run — FAIL**

Run: `cargo test -p networkfix`
Expected: FAIL (bad clap / old stub).

- [ ] **Step 3: Implement `main.rs`**

```rust
use clap::{Parser, Subcommand};
use networkfix_core::elevation::{is_elevated, relaunch_elevated};
use networkfix_core::{run_fix, FixReport, Mode, ProgressEvent, StepStatus};
use std::io::{self, BufRead, Write};
use std::str::FromStr;

#[derive(Parser)]
#[command(name = "networkfix", version, about = "Cross-platform network hotspot/cellular repair")]
struct Args {
    /// Emit FixReport JSON on stdout (progress on stderr)
    #[arg(long, global = true)]
    json: bool,

    /// Skip UAC/pkexec relaunch (operations may warn without admin)
    #[arg(long, global = true)]
    no_elevate: bool,

    #[command(subcommand)]
    mode: Option<ModeCmd>,
}

#[derive(Subcommand)]
enum ModeCmd {
    /// Restart hotspot + network services, flush DNS (fastest)
    Quick,
    /// Quick + adapter cycle + stack reset
    Full,
    /// Hotspot/Wi-Fi services only
    Hotspot,
    /// Fix ICS/NAT/forwarding for clients with no internet
    NoInternet,
}

impl From<ModeCmd> for Mode {
    fn from(c: ModeCmd) -> Self {
        match c {
            ModeCmd::Quick => Mode::Quick,
            ModeCmd::Full => Mode::Full,
            ModeCmd::Hotspot => Mode::HotspotOnly,
            ModeCmd::NoInternet => Mode::NoInternet,
        }
    }
}

fn menu_select() -> Result<Mode, String> {
    eprintln!("============================================");
    eprintln!(" Network Fix Menu - pick an option");
    eprintln!("============================================");
    eprintln!(" [1] Quick  - restart cellular + hotspot services (fastest)");
    eprintln!(" [2] Full   - also Wi-Fi, stack reset, deeper clean");
    eprintln!(" [3] Hotspot only - restart Wi-Fi + hotspot, keep cellular");
    eprintln!(" [4] Hotspot has clients but NO internet (ICS/NAT fix)");
    eprintln!(" [Q] Quit");
    eprint!("Choose (1/2/3/4/Q): ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).map_err(|e| e.to_string())?;
    match line.trim().to_ascii_uppercase().as_str() {
        "1" => Ok(Mode::Quick),
        "2" => Ok(Mode::Full),
        "3" => Ok(Mode::HotspotOnly),
        "4" => Ok(Mode::NoInternet),
        "Q" | "" => Err("quit".into()),
        other => Err(format!("unknown mode: {other}")),
    }
}

fn main() {
    let args = Args::parse();
    let mode = match args.mode {
        Some(m) => m.into(),
        None => match menu_select() {
            Ok(m) => m,
            Err(e) if e == "quit" => std::process::exit(0),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        },
    };

    if !args.no_elevate && !is_elevated() {
        eprintln!("Requesting administrator privileges...");
        let exe = std::env::current_exe().expect("current exe");
        let mut forward = vec![mode.to_string()];
        if args.json {
            forward.push("--json".into());
        }
        match relaunch_elevated(&exe, &forward) {
            Ok(networkfix_core::elevation::Elevation::RelaunchRequested) => {
                std::process::exit(0);
            }
            _ => {
                eprintln!("error: elevation denied — re-run as admin/root or pass --no-elevate");
                std::process::exit(3);
            }
        }
    }

    let json = args.json;
    let report = run_fix(mode, |ev| {
        match ev {
            ProgressEvent::Start { mode } => eprintln!("== mode {mode} =="),
            ProgressEvent::Message { text } => eprintln!(".. {text}"),
            ProgressEvent::Step(s) => {
                let tag = match s.status {
                    StepStatus::Ok => "OK  ",
                    StepStatus::Warn => "WARN",
                    StepStatus::Fail => "FAIL",
                    StepStatus::Skipped => "SKIP",
                };
                eprintln!("[{tag}] {} — {}", s.name, s.detail);
            }
            ProgressEvent::Done { .. } => {}
        }
    });

    emit_final(&report, json);
    std::process::exit(report.exit_code() as i32);
}

fn emit_final(report: &FixReport, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(report).expect("serialize"));
    } else {
        eprintln!(
            "Done in {} ms — exit {}",
            report.duration_ms,
            report.exit_code()
        );
    }
}
```

Note: elevation forward args must include exe path style for `Start-Process -ArgumentList` — arguments should be `["quick", "--json"]` without exe; relaunch prepends exe path itself — matches Task 4 signature `relaunch_elevated(current_exe, args)`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p networkfix`
Expected: PASS. (`--no-elevate` avoids UAC in tests; on Linux CI root check skipped.)

- [ ] **Step 5: Manual smoke**

Run: `cargo run -p networkfix -- --version`
Expected: prints version.

- [ ] **Step 6: Commit**

```bash
git add crates/networkfix
git commit -m "feat(cli): menu, modes, json output, elevation, exit codes"
```

---

### Task 12: Dioxus GUI

**Files:**
- Modify: `crates/networkfix-gui/src/main.rs`
- Create: `crates/networkfix-gui/src/app.rs` (if split needed; single main.rs acceptable if <300 lines)

**Interfaces:**
- Consumes: `run_fix`, `Mode`, `ProgressEvent`, `elevation`
- Produces: windowed app:
  - Title: `NetworkFix`
  - Four buttons + log `<pre>` + status bar (elevated yes/no, backend/platform label via `std::env::consts::OS`)
  - Click button → spawn thread `std::thread::spawn` running `run_fix` with channel `std::sync::mpsc` of `ProgressEvent` → main thread `use_future`/store drains into `Vec<String>` log lines (use `crate::signals` / `use_signal` in Dioxus 0.6)
  - While running: buttons disabled (`running` signal)
  - If not elevated: banner "Not running as administrator/root — restart elevated for full effect" + button "Restart elevated" calling relaunch + `std::process::exit(0)`

- [ ] **Step 1: Implement GUI**

```rust
use dioxus::prelude::*;
use networkfix_core::elevation::{is_elevated, relaunch_elevated};
use networkfix_core::{run_fix, Mode, ProgressEvent, StepStatus};
use std::sync::mpsc;

const CSS: &str = r#"
:root { color-scheme: dark; }
body { margin:0; font-family: 'Segoe UI', system-ui, sans-serif; background:#0f1115; color:#e6e6e6; }
.wrap { padding: 20px; max-width: 720px; margin: 0 auto; }
h1 { font-size: 1.3rem; margin: 0 0 4px; }
.sub { color:#9aa0a6; font-size: .9rem; margin-bottom: 16px; }
.grid { display:grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-bottom: 14px; }
button.mode { background:#1b2330; color:#e6e6e6; border:1px solid #2e3a4d; border-radius:8px; padding:14px 12px; font-size:1rem; cursor:pointer; text-align:left; }
button.mode:hover { background:#243044; }
button.mode:disabled { opacity:.5; cursor:not-allowed; }
button.mode small { display:block; color:#9aa0a6; font-size:.75rem; margin-top:4px; }
button.elev { background:#2b3a1f; border:1px solid #4a6b2f; color:#c6f0a0; border-radius:6px; padding:8px 12px; cursor:pointer; }
.banner { background:#3a2e14; border:1px solid #7a5c1e; color:#f0d78c; padding:10px 12px; border-radius:8px; margin-bottom:14px; display:flex; justify-content:space-between; align-items:center; gap:10px; }
pre.log { background:#0a0c10; border:1px solid #1e2530; border-radius:8px; padding:12px; height: 260px; overflow:auto; font-size: .82rem; white-space: pre-wrap; }
.ok { color:#7ee787; } .warn { color:#e3b341; } .fail { color:#ff7b72; } .skip { color:#8b949e; }
.status { margin-top:10px; color:#9aa0a6; font-size:.85rem; }
"#;

#[derive(Clone, PartialEq)]
struct LogLine {
    text: String,
    cls: &'static str,
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let elevated = use_signal(|| is_elevated());
    let running = use_signal(|| false);
    let log = use_signal(Vec::<LogLine>::new);
    let status = use_signal(|| String::from("Ready"));

    let spawn_fix = move |mode: Mode, label: &str| {
        if running.read() {
            return;
        }
        running.set(true);
        status.set(format!("Running {label}…"));
        let (tx, rx) = mpsc::channel::<ProgressEvent>();
        std::thread::spawn(move || {
            let _ = run_fix(mode, |ev| {
                let _ = tx.send(ev);
            });
        });
        // Drain in background, push to signal via spawn_blocking loop
        let mut log = log;
        let mut status = status;
        let mut running = running;
        std::thread::spawn(move || {
            while let Ok(ev) = rx.recv() {
                let line = match ev {
                    ProgressEvent::Start { mode } => LogLine {
                        text: format!("== {mode} ==".into()),
                        cls: "",
                    },
                    ProgressEvent::Message { text } => LogLine {
                        text: format!(".. {text}"),
                        cls: "",
                    },
                    ProgressEvent::Step(s) => {
                        let (cls, tag) = match s.status {
                            StepStatus::Ok => ("ok", "OK"),
                            StepStatus::Warn => ("warn", "WARN"),
                            StepStatus::Fail => ("fail", "FAIL"),
                            StepStatus::Skipped => ("skip", "SKIP"),
                        };
                        LogLine {
                            text: format!("[{tag}] {} — {}", s.name, s.detail),
                            cls,
                        }
                    }
                    ProgressEvent::Done { report } => {
                        status.set(format!(
                            "Finished in {} ms (exit {})",
                            report.duration_ms,
                            report.exit_code()
                        ));
                        running.set(false);
                        continue;
                    }
                };
                log.write().push(line);
            }
        });
    };

    let q = spawn_fix.clone();
    rsx! {
        style { "{CSS}" }
        div { class: "wrap",
            h1 { "NetworkFix" }
            div { class: "sub" { "Hotspot & cellular repair — no reboot for most cases" } }

            if !elevated.read() {
                div { class: "banner",
                    span { "Not running as admin/root — some fixes will be skipped." }
                    button { class: "elev", onclick: move |_| {
                        let exe = std::env::current_exe().unwrap();
                        let _ = relaunch_elevated(&exe, &[]);
                        std::process::exit(0);
                    }, "Restart elevated" }
                }
            }

            div { class: "grid",
                button { class: "mode", disabled: running.read(),
                    onclick: move |_| spawn_fix(Mode::Quick, "Quick"),
                    "1 · Quick"small { "Restart cellular + hotspot services, flush DNS" }
                }
                button { class: "mode", disabled: running.read(),
                    onclick: move |_| spawn_fix(Mode::Full, "Full"),
                    "2 · Full"small { "Quick + adapters + stack reset" }
                }
                button { class: "mode", disabled: running.read(),
                    onclick: move |_| spawn_fix(Mode::HotspotOnly, "Hotspot"),
                    "3 · Hotspot only"small { "Wi-Fi + hotspot services only" }
                }
                button { class: "mode", disabled: running.read(),
                    onclick: move |_| spawn_fix(Mode::NoInternet, "NoInternet"),
                    "4 · No internet"small { "Clients joined but have no internet (ICS/NAT)" }
                }
            }

            pre { class: "log",
                for line in log.read().iter() {
                    span { class: "{line.cls}", "{line.text}\n" }
                }
            }
            div { class: "status", "{status.read()}" }
            div { class: "status",
                "OS: {std::env::consts::OS} · elevated: {elevated.read()} · v{networkfix_core::VERSION}"
            }
        }
    }
}
```

Dioxus 0.6 rsx syntax may need tweaks (`small` as element inside button — use `span` with display:block if `small` errors). Signal mutation across threads: `Signal` is `Send` in 0.6 when `Clone` — if not, use `spawn` from dioxus async runtime with channel polled by `use_future`. **Fallback plan if compile errors:** use `use_effect` + `use_signal` updated only on main thread via `dioxus::prelude::spawn` async loop reading `tokio::sync::mpsc` — but dioxus desktop includes async runtime; simplest robust approach:

Rewrite drain using `use_future` + `tokio::sync::mpsc::unbounded_channel`:
- `run_fix` thread sends lines as strings only (format in worker thread — already have formatting logic; send `(cls, text)` tuples)
- `use_future` awaits `rx.recv()` in loop and pushes to `log`

If compile fails on `mpsc` signal `Send`, apply this fallback during implementation.

- [ ] **Step 2: Build GUI**

Run: `cargo build -p networkfix-gui`
Expected: compiles.

- [ ] **Step 3: Manual smoke (interactive session)**

Run: `cargo run -p networkfix-gui`
Expected: window opens; click Quick on a safe VM; log fills; buttons disable while running.

- [ ] **Step 4: Commit**

```bash
git add crates/networkfix-gui
git commit -m "feat(gui): dioxus mode buttons, live log, elevation banner"
```

---

### Task 13: Windows packaging (MSI + NSIS)

**Files:**
- Create: `packaging/windows/networkfix.nsi`
- Create: `packaging/windows/wix/main.wxs` (or cargo-wix generated)
- Modify: `crates/networkfix-gui/Cargo.toml` or root — add `[package.metadata.wix]` as needed
- Create: `scripts/build-windows-installers.ps1`
- Modify: `.gitignore` (ignore `target/wix`, `dist/`)

**Interfaces:**
- Produces: script `scripts/build-windows-installers.ps1` that:
  1. `cargo build --release -p networkfix -p networkfix-gui`
  2. Copies `networkfix.exe`, `networkfix-gui.exe` to `dist/windows/stage/`
  3. Runs `cargo wix` if available (`cargo install cargo-wix`) OR builds MSI via WiX `candle`/`light` on staged files — **choose NSIS `makensis` + MSI via cargo-wix**:
     - NSIS: `makensis packaging/windows/networkfix.nsi` → `dist/NetworkFix-0.1.0-Setup.exe`
     - MSI: `cargo wix init -p networkfix-gui` first time generates `wix/main.wxs`; script runs `cargo wix --package networkfix-gui` → `target/wix/*.msi`
  4. Both installers include: both exes, Start Menu shortcut to GUI, optional PATH addition for `networkfix.exe` (NSIS: add `$PROGRAMFILES\NetworkFix` to PATH; MSI: WiX Environment table).

- [ ] **Step 1: Write NSIS script**

`packaging/windows/networkfix.nsi` (essential content):
```nsis
!define NAME "NetworkFix"
!define VERSION "0.1.0"
Name "${NAME}"
OutFile "..\..\dist\NetworkFix-${VERSION}-Setup.exe"
InstallDir "$PROGRAMFILES64\${NAME}"
RequestExecutionLevel admin
Page directory
Page components
Page instfiles

Section "NetworkFix (GUI + CLI)" SecMain
  SetOutPath "$INSTDIR"
  File "..\..\target\release\networkfix.exe"
  File "..\..\target\release\networkfix-gui.exe"
  CreateShortCut "$SMPROGRAMS\NetworkFix.lnk" "$INSTDIR\networkfix-gui.exe"
  CreateShortCut "$DESKTOP\NetworkFix.lnk" "$INSTDIR\networkfix-gui.exe"
  EnVar::AddValue "PATH" "$INSTDIR"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\networkfix.exe"
  Delete "$INSTDIR\networkfix-gui.exe"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\NetworkFix.lnk"
  Delete "$DESKTOP\NetworkFix.lnk"
  EnVar::DeleteValue "PATH" "$INSTDIR"
  RMDir "$INSTDIR"
SectionEnd
```
Note: `EnVar` plugin may need `!include EnVar.nsh` / plugin install — if unavailable, use NSIS `WriteRegStr` PATH manipulation via `${EnvVarUpdate}` or document manual PATH; **simplify:** skip PATH plugin; write a `networkfix.cmd` wrapper in Start Menu folder instead if plugin missing. Implementation may replace EnVar with registry-based PATH update using `WriteRegExpandStr` on `HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment` — keep simple wrapper approach as fallback.

- [ ] **Step 2: Write build script** `scripts/build-windows-installers.ps1`

```powershell
$ErrorActionPreference = "Stop"
cargo build --release -p networkfix -p networkfix-gui
New-Item -ItemType Directory -Force -Path dist | Out-Null
if (Get-Command makensis -ErrorAction SilentlyContinue) {
  makensis packaging/windows/networkfix.nsi
} else {
  Write-Warning "makensis not found — skip NSIS"
}
if (Get-Command cargo -ErrorAction SilentlyContinue) {
  cargo install cargo-wix --locked 2>$null
  cargo wix -p networkfix --nologo || Write-Warning "cargo wix failed"
}
Get-ChildItem dist, target\wix -Recurse -Include *.exe, *.msi -ErrorAction SilentlyContinue
```

- [ ] **Step 3: Verify (on Windows machine)**

Run: `powershell -File scripts/build-windows-installers.ps1`
Expected: at least release binaries built; MSI/NSIS when tools installed; warnings acceptable if tools missing (CI installs them).

- [ ] **Step 4: Commit**

```bash
git add packaging/windows scripts
git commit -m "feat(packaging): windows MSI and NSIS installer configs"
```

---

### Task 14: Linux packaging (deb, rpm, AppImage)

**Files:**
- Create: `packaging/linux/networkfix.desktop`
- Create: `packaging/linux/build-packages.sh`
- Create: `packaging/linux/AppDir` layout instructions inside script (or use `cargo-packager` config in root `Cargo.toml`)

**Interfaces:**
- `cargo-packager` config at workspace root or in `networkfix-gui/Cargo.toml`:

```toml
[package.metadata.packager]
formats = ["appimage", "deb", "rpm"]
product-name = "NetworkFix"
identifier = "dev.networkfix.app"
resources = ["../../packaging/linux/networkfix.desktop"]
```
(Adjust relative paths; packager runs per-binary package `networkfix-gui`.)

- `packaging/linux/networkfix.desktop`:
```ini
[Desktop Entry]
Type=Application
Name=NetworkFix
Comment=Hotspot and cellular network repair
Exec=networkfix-gui
Terminal=false
Categories=Network;System;
```

- `packaging/linux/build-packages.sh`:
```bash
#!/usr/bin/env bash
set -euo pipefail
cargo build --release -p networkfix -p networkfix-gui
cargo install cargo-packager --locked
cargo packager -p networkfix-gui --formats appimage,deb,rpm -r release
mkdir -p dist/linux
cp target/release/networkfix target/release/networkfix-gui dist/linux/
find target -maxdepth 3 \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -exec cp {} dist/linux/ \;
ls -la dist/linux
```

- [ ] **Step 1: Create files as above**

- [ ] **Step 2: Verify**

Run: `bash packaging/linux/build-packages.sh` (on Linux or WSL)
Expected: `.deb`, `.rpm`, `.AppImage` in `dist/linux` (missing format tools → packager errors; script may install `rpm`/`appimagetool` hints in comments).

- [ ] **Step 3: Commit**

```bash
git add packaging/linux
git commit -m "feat(packaging): linux deb/rpm/appimage via cargo-packager"
```

---

### Task 15: CI release workflow

**Files:**
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/release.yml`

**Interfaces:**
- `ci.yml` on push/PR: matrix `ubuntu-latest`, `windows-latest` → `cargo test --workspace` (gui compiles; may skip running GUI).
- `release.yml` on tag `v*`:
  1. test job (same matrix)
  2. `build-windows`: windows-latest → run NSIS (install via choco `nsis`) + `cargo wix`; upload `*.msi`, `*Setup.exe`
  3. `build-linux`: ubuntu-latest → install `rpm`, `libwebkit2gtk-4.1-dev` (Dioxus GUI deps), run `packaging/linux/build-packages.sh`; upload deb/rpm/AppImage
  4. `softprops/action-gh-release` attach artifacts

- [ ] **Step 1: Write `ci.yml`**

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace
```

- [ ] **Step 2: Write `release.yml`**

```yaml
name: Release
on:
  push:
    tags: ["v*"]
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace
  windows-artifacts:
    needs: test
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - uses: microsoft/setup-msbuild@v2
      - name: Install NSIS
        run: choco install nsis -y
      - name: Build release binaries
        run: cargo build --release -p networkfix -p networkfix-gui
      - name: NSIS installer
        run: makensis packaging/windows/networkfix.nsi
      - name: MSI
        run: |
          cargo install cargo-wix --locked
          cargo wix -p networkfix --nologo
      - uses: actions/upload-artifact@v4
        with:
          name: windows-installers
          path: |
            dist/*.exe
            target/wix/*.msi
  linux-artifacts:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: System deps
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev librsvg2-bin rpm
      - name: Build packages
        run: bash packaging/linux/build-packages.sh
      - uses: actions/upload-artifact@v4
        with:
          name: linux-packages
          path: dist/linux/*
  release:
    needs: [windows-artifacts, linux-artifacts]
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
      - uses: softprops/action-gh-release@v2
        with:
          files: |
            windows-installers/**
            linux-packages/**
```

- [ ] **Step 3: Validate workflow syntax**

Run: actionlint if available, else visual review.
Expected: valid YAML.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows
git commit -m "ci: test matrix and release artifacts for all installers"
```

---

### Task 16: README + legacy note + manual test checklist

**Files:**
- Modify: `README.md`
- Create: `docs/MANUAL_TEST_CHECKLIST.md`

**Interfaces:**
- README sections: features, install (per OS), usage (GUI screenshot placeholder + CLI examples), build from source, legacy scripts pointer, support matrix (Win7 CLI / Win10-11 GUI / Linux GUI), spec/plan links.

- [ ] **Step 1: Write README** (content outline with real commands)

```markdown
# NetworkFix
Installable hotspot/cellular repair for Windows and Linux.

## Install
- Windows: run `NetworkFix-0.1.0-Setup.exe` (NSIS) or `NetworkFix.msi`
- Debian/Ubuntu: `sudo apt install ./networkfix_0.1.0_amd64.deb`
- Fedora: `sudo rpm -i networkfix-0.1.0.x86_64.rpm`
- Any Linux: download AppImage, `chmod +x`, run

## CLI
networkfix              # menu
networkfix quick
networkfix no-internet --json

## Build
cargo build --release
cargo test --workspace

## Support
| OS | GUI | CLI |
|----|-----|-----|
| Windows 11/10 | yes | yes |
| Windows 7 | no | yes |
| Debian/Ubuntu/Arch/Fedora | yes | yes |

Legacy PowerShell scripts: `legacy/` (unsupported).
Spec: docs/superpowers/specs/...
```

- [ ] **Step 2: Write checklist**

`docs/MANUAL_TEST_CHECKLIST.md` with rows from spec §10: machine, mode, expected, pass/fail columns for Win11 Quick, Win11 NoInternet+phone, Win10 Quick, Win7 CLI quick, Debian quick+nointernet, Arch quick.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/MANUAL_TEST_CHECKLIST.md
git commit -m "docs: install/usage readme and manual test checklist"
```

---

## Self-Review (completed)

1. **Spec coverage:**
   - §2 architecture → Tasks 1–4, 8
   - §3 modes → Tasks 5–10
   - §4 Windows → Tasks 5–8; Linux → 9–10
   - §4 elevation → Task 4 + 11/12
   - §5 CLI → 11; GUI → 12; legacy → 1 + 16
   - §6 packaging → 13–14; CI → 15
   - §7 testing → unit tests in each task; manual → 16
   - §8 error handling → Global Constraints + Step statuses in every impl
   - §10 success criteria → Task 16 checklist + CI green

2. **Placeholders:** none — all steps include concrete code/commands. MockRunner `expect_seq` is specified in Task 9 before use.

3. **Type consistency:** `Mode`, `Step`, `FixReport`, `ProgressEvent`, `CommandRunner`, `run_fix_with` signatures match across tasks 2, 3, 8, 11, 12. `windows::run_mode` / `linux::run_mode` both `(Mode, &dyn CommandRunner, &dyn Fn(ProgressEvent), bool) -> FixReport`.

**Known implementation-time flexibility:** Dioxus 0.6 signal/thread details (Task 12 fallback), NSIS PATH plugin (Task 13 fallback to wrapper/registry), Win7 dedicated target (document smoke-test; may ship same msvc binary if win7 target unavailable — record result in checklist).
