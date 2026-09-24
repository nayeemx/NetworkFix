# NetworkFix

Installable hotspot/cellular network repair for Windows and Linux — Dioxus GUI + CLI sharing one Rust core.

## Install

### Windows 10/11

Run one of the release installers:

- **NSIS:** `NetworkFix-0.1.0-Setup.exe`
- **MSI:** `NetworkFix-0.1.0.msi`

Both install `networkfix.exe` (CLI) and `networkfix-gui.exe` (GUI) with Start Menu shortcuts. The GUI needs WebView2 (preinstalled on most Windows 10/11 systems).

### Debian / Ubuntu

```bash
sudo apt install ./networkfix_0.1.0_amd64.deb
```

### Fedora

```bash
sudo rpm -i networkfix-0.1.0.x86_64.rpm
```

### Any Linux (AppImage)

```bash
chmod +x networkfix-*.AppImage
./networkfix-*.AppImage
```

## CLI usage

```bash
networkfix                      # interactive menu (parity with legacy .bat)
networkfix quick                # restart services, flush DNS, renew DHCP
networkfix full                 # quick + adapter cycle + stack reset
networkfix hotspot              # hotspot/Wi-Fi services only
networkfix no-internet --json   # ICS/NAT fix; FixReport JSON on stdout
networkfix --version
```

- `--json` prints only JSON on stdout; progress lines go to stderr (safe for scripting).
- `--no-elevate` skips the UAC/pkexec relaunch (steps may warn without admin).

### Exit codes

| Code | Meaning |
|------|---------|
| `0`  | All steps Ok or Skipped |
| `1`  | At least one Warn |
| `2`  | At least one Fail |
| `3`  | Elevation denied |

### Dry-run (safe testing)

```powershell
$env:NETWORKFIX_DRY_RUN = "1"
networkfix quick
```

With `NETWORKFIX_DRY_RUN=1` no real commands run and no elevation prompt appears — steps execute against a mock runner so you can inspect the report safely.

## GUI

Desktop GUI built with **Dioxus**: four mode buttons (Quick / Full / Hotspot only / No internet), live color-coded log, elevation status in the header. Supported on Windows 10/11 and Linux (WebKitGTK required).

## Build from source

```bash
cargo build --release          # all three crates
cargo test --workspace         # unit tests (mocked commands only)
```

Installers:

```powershell
# Windows (requires NSIS + WiX v3 on PATH, or CI)
.\scripts\build-windows-installers.ps1
```

```bash
# Linux (deb + rpm + AppImage; must run on Linux/WSL)
packaging/linux/build-packages.sh
```

## Support matrix

| OS | GUI | CLI |
|----|-----|-----|
| Windows 11 / 10 | yes | yes |
| Windows 7 | no | yes |
| Debian / Ubuntu / Arch / Fedora | yes | yes |

## Modes

| Mode | Description |
|------|-------------|
| **Quick** | Restart hotspot + network services; flush DNS; renew DHCP on primary/cellular interface |
| **Full** | Quick + disable/enable relevant adapters + stack reset (Win: Winsock/IP/TCP; Linux: reload NetworkManager, flush routes) + restart hotspot services again |
| **Hotspot** | Restart hotspot/Wi-Fi services only; leave cellular/WWAN untouched |
| **No-internet** | Enable IPv4 forwarding; ensure private-side IP (`192.168.137.1` on Windows); NAT masquerade (ICS/`SharedAccess` or nftables/iptables); toggle hotspot; check/enable firewall ICS rules |

Missing optional hardware (e.g. no WWAN on a desktop) is reported as **Skipped**, never a hard failure.

## Legacy scripts

Original PowerShell/batch scripts live in `legacy/` — kept as reference only, **unsupported**.

## Docs

- Spec: [`docs/superpowers/specs/2026-09-24-networkfix-cross-platform-design.md`](docs/superpowers/specs/2026-09-24-networkfix-cross-platform-design.md)
- Plan: [`docs/superpowers/plans/2026-09-24-networkfix-cross-platform.md`](docs/superpowers/plans/2026-09-24-networkfix-cross-platform.md)
- Manual test checklist: [`docs/MANUAL_TEST_CHECKLIST.md`](docs/MANUAL_TEST_CHECKLIST.md)
