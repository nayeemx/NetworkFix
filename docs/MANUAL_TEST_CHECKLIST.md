# NetworkFix Manual Test Checklist

Run before each release (spec §7 / §10). Mark Pass/Fail and add notes per row.

| Machine / OS | Mode | Steps | Expected | Pass/Fail/Notes |
|--------------|------|-------|----------|-----------------|
| Windows 11 | Quick | Install NSIS or MSI; break hotspot (toggle radio, reconnect phone); run `networkfix quick` | Hotspot recovers without reboot; steps Ok/Skipped; exit 0 (or 1 with benign warns) | |
| Windows 11 | No-internet | Join phone to PC hotspot; confirm phone has no internet; run `networkfix no-internet` | Phone regains internet (ICS/NAT restored, 192.168.137.1 present); exit 0/1 | |
| Windows 10 | Quick (CLI) | Install (NSIS or MSI); run `networkfix` from PATH (MSI) or Start Menu → NetworkFix CLI (NSIS), then `networkfix quick` | Completes without crash; services restart; exit 0/1 | |
| Windows 7 | Quick (CLI only) | Copy `networkfix.exe`; run `networkfix quick` — do not attempt GUI | CLI runs (no GUI shipped/attempted); repair steps execute or skip gracefully; exit 0/1/2 as appropriate | |
| Debian/Ubuntu VM | Quick + No-internet | Install `.deb`; run `networkfix quick`, then `networkfix no-internet` with phone on hotspot | Backend detected (NetworkManager); both modes complete; phone gets internet after no-internet mode | |
| Arch VM | Quick | Install (AppImage or package); run `networkfix quick` | Completes with NetworkManager backend; exit 0/1 | |
| Windows 11 | GUI smoke | Launch `networkfix-gui.exe` | Window opens (~640×480); four mode buttons; live log; elevation header; run Quick from GUI succeeds | |
| Windows 10 | GUI smoke | Launch `networkfix-gui.exe` | Window opens; four mode buttons; live log; run Quick from GUI succeeds | |
| Debian | GUI smoke | Launch `networkfix-gui` (WebKitGTK installed) | Window opens; four mode buttons; live log; run a mode from GUI succeeds | |
| Arch-based VM | GUI smoke (Quick) | Launch `networkfix-gui` (WebKitGTK installed); run Quick mode | Window opens; four mode buttons; Quick completes with NetworkManager backend; exit 0/1 | |

## CI / release criteria (spec §10 items 5–6)

Not manual OS rows — verify via workflows:

| Criterion | Verification method | Pass/Fail/Notes |
|-----------|--------------------|-----------------|
| Core unit tests pass in CI on every push (§10.6) | Check `.github/workflows/ci.yml` run for the release commit is green (Linux + Windows matrix) | |
| All five installer artifacts build in CI from a tag (§10.5) | Push tag `v*`; confirm `.github/workflows/release.yml` publishes MSI, NSIS `.exe`, AppImage, `.deb`, `.rpm` as release assets | |

## Notes

- Win7 dedicated `x86_64-win7-windows-msvc` target: if unavailable, smoke-test the Win10-build CLI on Win7 and record the result here: ______________________
- Dry-run smoke on any machine: `NETWORKFIX_DRY_RUN=1 networkfix quick` → report produced, no elevation prompt, no real commands.
