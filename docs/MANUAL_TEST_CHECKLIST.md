# NetworkFix Manual Test Checklist

Run before each release (spec §7 / §10). Mark Pass/Fail and add notes per row.

| Machine / OS | Mode | Steps | Expected | Pass/Fail/Notes |
|--------------|------|-------|----------|-----------------|
| Windows 11 | Quick | Install NSIS or MSI; break hotspot (toggle radio, reconnect phone); run `networkfix quick` | Hotspot recovers without reboot; steps Ok/Skipped; exit 0 (or 1 with benign warns) | Pending — needs phone + real hotspot break |
| Windows 11 | No-internet | Join phone to PC hotspot; confirm phone has no internet; run `networkfix no-internet` | Phone regains internet (ICS/NAT restored, 192.168.137.1 present); exit 0/1 | Pending — needs phone on hotspot |
| Windows 10 | Quick (CLI) | Install (NSIS or MSI); run `networkfix` from PATH (MSI) or Start Menu → NetworkFix CLI (NSIS), then `networkfix quick` | Completes without crash; services restart; exit 0/1 | Pending — no Win10 machine here |
| Windows 7 | Quick (CLI only) | Copy `networkfix.exe`; run `networkfix quick` — do not attempt GUI | CLI runs (no GUI shipped/attempted); repair steps execute or skip gracefully; exit 0/1/2 as appropriate | Pending — no Win7 machine here |
| Debian/Ubuntu VM | Quick + No-internet | Install `.deb`; run `networkfix quick`, then `networkfix no-internet` with phone on hotspot | Backend detected (NetworkManager); both modes complete; phone gets internet after no-internet mode | Pending — no Debian VM here |
| Arch VM | Quick | Install (AppImage or package); run `networkfix quick` | Completes with NetworkManager backend; exit 0/1 | Pending — no Arch VM here |
| Windows 11 | GUI smoke | Launch `networkfix-gui.exe` | Window opens (~640×480); four mode buttons; live log; elevation header; run Quick from GUI succeeds | **Pass (2026-09-24)** — NSIS reinstall to `C:\Program Files\NetworkFix`; window opens (title `NetworkFix`); WebView2 profile under `%LOCALAPPDATA%\NetworkFix\WebView2`; dry-run launch OK. Interactive button click still for human confirmation |
| Windows 10 | GUI smoke | Launch `networkfix-gui.exe` | Window opens; four mode buttons; live log; run Quick from GUI succeeds | Pending — no Win10 machine here |
| Debian | GUI smoke | Launch `networkfix-gui` (WebKitGTK installed) | Window opens; four mode buttons; live log; run a mode from GUI succeeds | Pending — no Debian VM here |
| Arch-based VM | GUI smoke (Quick) | Launch `networkfix-gui` (WebKitGTK installed); run Quick mode | Window opens; four mode buttons; Quick completes with NetworkManager backend; exit 0/1 | Pending — no Arch VM here |

## Local machine results (Windows 11 Pro, 2026-09-24)

Ran against installed binaries after clean NSIS uninstall + `NetworkFix-0.1.0-Setup.exe` reinstall (fixed WebView2 data dir).

| Check | Result |
|-------|--------|
| `networkfix --version` | **Pass** — `networkfix 0.1.0`, exit 0 |
| `NETWORKFIX_DRY_RUN=1 networkfix quick` | **Pass** — exit 0, mock steps, no real commands |
| `NETWORKFIX_DRY_RUN=1 networkfix --json full` | **Pass** — pure JSON on stdout (`mode`, `steps`, `elevated`, `duration_ms`); exit 0 |
| `networkfix --no-elevate no-internet` (dry-run) | **Pass** — exit 0 |
| Unknown mode `not-a-mode` | **Pass** — nonzero exit (clap), error on stderr |
| `networkfix.cmd --version` (Start Menu / install dir wrapper) | **Pass** |
| GUI launch from `C:\Program Files\NetworkFix` | **Pass** — process stays alive, window title `NetworkFix` |
| WebView2 profile location | **Pass** — `%LOCALAPPDATA%\NetworkFix\WebView2`; no `networkfix-gui.exe.WebView2` under Program Files |
| Start Menu shortcuts (GUI + CLI) | **Pass** |
| Installed GUI SHA256 == `target\release\networkfix-gui.exe` | **Pass** |
| `cargo test --workspace` | **Pass** — 44 tests |
| `cargo clippy --workspace --all-targets -- -D warnings` | **Pass** |
| `bash -n packaging/linux/build-packages.sh` | **Pass** |
| Hotspot recover / phone no-internet (live) | Not run — needs external hardware |
| Windows 7 / 10 / Debian / Arch OS rows | Pending — no those machines/VMs here |

## CI / release criteria (spec §10 items 5–6)

Not manual OS rows — verify via workflows:

| Criterion | Verification method | Pass/Fail/Notes |
|-----------|--------------------|-----------------|
| Core unit tests pass in CI on every push (§10.6) | Check `.github/workflows/ci.yml` run for the release commit is green (Linux + Windows matrix) | Pending — no git remote yet; CI runs after first push |
| All five installer artifacts build in CI from a tag (§10.5) | Push tag `v*`; confirm `.github/workflows/release.yml` publishes MSI, NSIS `.exe`, AppImage, `.deb`, `.rpm` as release assets | Pending — no git remote yet |

## Notes

- Win7 dedicated `x86_64-win7-windows-msvc` target: if unavailable, smoke-test the Win10-build CLI on Win7 and record the result here: ______________________
- Dry-run smoke on any machine: `NETWORKFIX_DRY_RUN=1 networkfix quick` → report produced, no elevation prompt, no real commands. **Local dry-run Pass (2026-09-24).**
- Local Windows installers: `dist/NetworkFix-0.1.0-Setup.exe` (NSIS), `dist/NetworkFix-0.1.0.msi` (WiX), built with portable NSIS 3.12 + WiX 3.11.
- Tag `v0.1.0` is local-only until a remote is added (`git remote add origin … && git push -u origin master --tags`).
