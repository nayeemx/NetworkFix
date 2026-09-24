# NetworkFix Cross-Platform Design

**Date:** 2026-09-24
**Status:** Approved (pending spec review)
**Supersedes:** PowerShell/batch scripts (moved to `legacy/`)

## 1. Goals

Convert NetworkFix from loose `.bat`/`.ps1` scripts into an installable, cross-platform desktop application:

- **Windows 10/11:** GUI (Dioxus) + CLI
- **Windows 7:** CLI only (no WebView2 → no GUI)
- **Linux (Debian, Arch, Fedora, Mint, etc.):** GUI + CLI
- **Installers:** MSI + NSIS (Windows); AppImage + `.deb` + `.rpm` (Linux)
- **Modes:** Quick, Full, HotspotOnly, NoInternet — same four on every OS

Non-goals: mobile OS support, GUI on Windows 7, non-NetworkManager-first exotic distros beyond graceful fallback.

## 2. Architecture

Cargo workspace, three crates, **Approach A: two binaries sharing one core**.

```
NetworkFix/
├── Cargo.toml                 # workspace
├── crates/
│   ├── networkfix-core/       # fix logic, types, platform dispatch
│   ├── networkfix/            # CLI binary (clap) — works on Win7+
│   └── networkfix-gui/        # Dioxus desktop app
├── packaging/
│   ├── windows/               # MSI (cargo-wix), NSIS (.nsi)
│   └── linux/                 # deb, rpm, AppImage recipes
├── .github/workflows/release.yml
├── legacy/                    # original .bat / .ps1 (reference only)
└── docs/superpowers/specs/
```

### 2.1 Dependency direction

```
networkfix-gui ──► networkfix-core ◄── networkfix (CLI)
                         │
              ┌──────────┴──────────┐
        windows_fix (cfg)     linux_fix (cfg)
```

- OS selected at **compile time** with `#[cfg(target_os = "...")]`.
- Both binaries must not reimplement fix steps; they only render `FixReport`.

### 2.2 Shared types (networkfix-core)

```rust
pub enum Mode { Quick, Full, HotspotOnly, NoInternet }

pub enum StepStatus { Ok, Warn, Fail, Skipped }

pub struct Step {
    pub name: String,
    pub status: StepStatus,
    pub detail: String,
}

pub struct FixReport {
    pub mode: Mode,
    pub steps: Vec<Step>,
    pub elevated: bool,
    pub duration_ms: u64,
}

pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output, io::Error>;
}
```

- `CommandRunner` is an injectable trait so unit tests mock `netsh`/`nmcli` without touching the real system.
- Progress streaming: `run_fix(mode, &dyn Fn(ProgressEvent))` so the GUI log updates live and CLI prints line-by-line.

### 2.3 Fix entry point

```rust
pub fn run_fix(mode: Mode, on_event: impl Fn(ProgressEvent)) -> FixReport
```

1. Check elevation; if missing, signal caller (`ElevationRequired`) so GUI/CLI can relaunch elevated (Windows: `ShellExecute runas`; Linux: `pkexec`/`sudo`).
2. Dispatch to platform module.
3. Collect steps; never panic — missing services/adapters → `Warn`/`Skipped`.

## 3. Mode semantics

| Mode | Steps (shared intent) |
|------|------------------------|
| **Quick** | Restart hotspot + network services; flush DNS; renew DHCP on primary/cellular iface |
| **Full** | Quick + disable/enable relevant adapters + stack reset (Win: Winsock/IP/TCP; Linux: reload NetworkManager, flush routes) + restart hotspot services again |
| **HotspotOnly** | Restart hotspot/Wi-Fi services only; leave cellular/WWAN untouched |
| **NoInternet** | Enable IPv4 forwarding; ensure private-side IP (Windows: `192.168.137.1` on Wi-Fi Direct adapter; Linux: hotspot iface CIDR); NAT masquerade (Windows ICS/`SharedAccess`; Linux `nftables`/`iptables` MASQUERADE); toggle hotspot off/on; check/enable firewall ICS rules |

**Graceful skip:** if a mode references hardware that does not exist (no WWAN on a desktop), emit `Skipped` with reason — never `Fail` the whole run for a missing optional component.

## 4. Platform backends

### 4.1 Windows (`windows_fix`)

Ports the behavior of the legacy scripts (authoritative reference: `legacy/fix-network.ps1`, `legacy/fix-hotspot-internet.ps1`):

- **Services:** restart `icssvc`, `WlanSvc`, `SharedAccess`, `EapHost`, `dot3svc`, `RasMan`, `PhoneSvc`, `tapisrv` (skip absent); set `SharedAccess` startup = Automatic in NoInternet mode.
- **Commands:** `ipconfig /flushdns`, `netsh int ipv4 set global forwarding=enabled`, `netsh winsock reset`, `netsh int ip reset`, `netsh int tcp reset` (Full only), `netsh interface set interface admin=disable/enable` fallback for adapter restart.
- **Adapters:** match WWAN/Wi-Fi via description/name patterns (same regexes as legacy script); Disable → sleep → Enable → wait for link (15s deadline).
- **NoInternet specifics:** find Wi-Fi Direct virtual adapter; ensure IPv4 = `192.168.137.1/24`; toggle tethering via WinRT `NetworkOperatorTetheringManager` (invoke through a small PowerShell bridge only if no pure-Rust path is reliable); enable disabled ICS firewall rules.
- **Elevation:** relaunch self with `runas` verb; CLI prints a clear message if user declines UAC.

Supported OS: Windows 10/11 full; Windows 7 CLI-only (avoid APIs beyond Win7 in the CLI/core path; no WebView2 dependency).

### 4.2 Linux (`linux_fix`)

- **Backend detection (in order):** NetworkManager (`nmcli`) → systemd-networkd → `iwctl`; if none, Fail with actionable message.
- **Quick:** `nmcli networking off/on` or restart `NetworkManager.service`; flush DNS (`resolvectl flush-caches`, fallback delete `/etc/resolv.conf` cache paths is **not** done — only safe commands); renew via `nmcli device reapply` / DHCP release-renew on primary iface.
- **Full:** Quick + `ip link set down/up` cycle on matched wireless/wwan ifaces + restart `wpa_supplicant` if present.
- **HotspotOnly:** restart NM hotspot connection (`nmcli con down/up` for the hotspot profile) or `hostapd` service if used.
- **NoInternet:** `sysctl net.ipv4.ip_forward=1` (persist hint in report); nftables `masquerade` on outbound iface (iptables fallback); verify hotspot iface IP; restart `dnsmasq` if installed; `systemctl restart NetworkManager`.
- **Elevation:** re-exec via `pkexec` then `sudo` fallback.

Target: any systemd or broadly standard distro (Debian/Ubuntu/Mint, Arch/Manjaro, Fedora/openSUSE). Non-systemd (e.g., Artix, Alpine) gets best-effort CLI with explicit warnings — not a release blocker.

## 5. Interfaces

### 5.1 CLI (`networkfix`)

```bash
networkfix                 # interactive menu (parity with legacy .bat)
networkfix quick
networkfix full
networkfix hotspot
networkfix no-internet
networkfix --json quick    # machine-readable FixReport to stdout
networkfix --version
```

- Exit codes: `0` all Ok/Skipped; `1` any Warn; `2` any Fail; `3` elevation denied.
- `--json` prints only JSON (progress goes to stderr) for scripting.

### 5.2 GUI (`networkfix-gui`, Dioxus desktop)

- Single window (~640×480), dark-friendly.
- Four large mode buttons: Quick / Full / Hotspot only / No internet.
- Live log panel (timestamped steps, color: green OK, yellow WARN, red FAIL).
- Header status: elevation state, detected platform/backend (e.g., "NetworkManager").
- "Run as admin" (Windows) / "Authenticate" (Linux) affordance when not elevated.
- Disable buttons while a fix runs; show duration when done.
- No settings screen in v1 (YAGNI).

### 5.3 Legacy scripts

Move `*.bat`, `*.ps1` → `legacy/`. README documents them as unsupported reference. Do not delete (user request: keep as fallback reference).

## 6. Packaging

| Platform | Artifacts | Tooling |
|----------|-----------|---------|
| Windows | MSI + NSIS `.exe` | `cargo-wix` + NSIS script; both ship `networkfix.exe` + `networkfix-gui.exe` + Start Menu shortcut |
| Linux | AppImage, `.deb`, `.rpm` | `cargo-packager` (or equivalent); `.desktop` entry for GUI; `networkfix` on PATH |

- Installers detect nothing at install time — elevation happens at **run** time (standard for network tools).
- WebView2 Evergreen bootstrapper noted as Windows GUI prerequisite (Win10/11 usually already have Edge WebView2).
- Linux GUI runtime depends on WebKitGTK (documented in package metadata).

### CI (GitHub Actions)

On tag `v*`:
1. `test` job (Linux + Windows matrix)
2. Build matrix: `x86_64-pc-windows-msvc` (GUI+CLI), `x86_64-win7-windows-msvc` (CLI-only if practical; else document Win7 as "build with older toolchain"), `x86_64-unknown-linux-gnu` (GUI+CLI)
3. Package job → upload MSI, NSIS, AppImage, deb, rpm as release assets

## 7. Testing

- **Unit (core):** mode routing, report assembly, JSON serialization, backend detection logic — all against `MockCommandRunner`.
- **Unit (per platform module):** given mocked command outputs, assert correct step statuses (e.g., missing SharedAccess → Skipped not Fail).
- **Manual checklist (per release):**
  - Windows 11: Quick + NoInternet with a phone joined to hotspot
  - Windows 10: Quick
  - Windows 7: CLI `quick` runs, no GUI attempt
  - Debian/Ubuntu VM: Quick + NoInternet
  - Arch VM: Quick
- No automated GUI E2E in v1 (Dioxus desktop E2E is costly); manual + core tests cover logic.

## 8. Error handling principles

1. Never panic in fix paths; recover or mark Fail with message.
2. One step failing does not abort remaining steps (best-effort repair).
3. Elevation denial → clear message + exit 3, not a crash.
4. Unknown backend → explicit "unsupported backend" Fail, not silent no-op.

## 9. Open risks

| Risk | Mitigation |
|------|------------|
| WinRT tethering toggle awkward from Rust | Fallback: invoke embedded PowerShell one-liner (still shipped in binary, no external .ps1 file needed) |
| Win7 target toolchain quirks | Ship Win7 as CLI built with `x86_64-win7-windows-msvc`; if unavailable, document "CLI from Win10 build runs on Win7" only after smoke test |
| Distro variance on Linux | Backend detection order + Skipped/Warn; README support matrix |
| WebView2/WebKitGTK missing | Installer notes; GUI binary detects and prints guidance to console if webview fails to launch |

## 10. Success criteria

- [ ] `networkfix quick` fixes a broken hotspot on Windows 11 without reboot (parity with legacy script)
- [ ] `networkfix no-internet` restores internet for a joined phone (Windows + one Linux distro)
- [ ] GUI runs on Windows 10/11 and one Debian-based + one Arch-based distro
- [ ] CLI runs on Windows 7
- [ ] All five installer artifacts build in CI from a tag
- [ ] Core unit tests pass in CI on every push
