#Requires -Version 5.1
<#
  Fix: Mobile Hotspot clients CONNECTED but have NO internet
  while the PC itself has internet (cellular works).

  Targets: ICS (SharedAccess), Wi-Fi Direct virtual adapter,
  IPv4 forwarding, NAT path, tethering restart (no reboot).
#>

$ErrorActionPreference = 'SilentlyContinue'

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}
if (-not (Test-Admin)) {
    Start-Process powershell.exe -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`""
    exit
}

function Write-Step($m) { Write-Host "`n>> $m" -ForegroundColor Cyan }
function Write-Ok($m)   { Write-Host "   OK: $m" -ForegroundColor Green }
function Write-Warn($m) { Write-Host "   WARN: $m" -ForegroundColor Yellow }
function Write-Bad($m)  { Write-Host "   FAIL: $m" -ForegroundColor Red }

Write-Host "============================================" -ForegroundColor White
Write-Host " Hotspot: Connected but NO internet fix" -ForegroundColor White
Write-Host " (no reboot)" -ForegroundColor White
Write-Host "============================================" -ForegroundColor White

# ---------------------------------------------------------------
# DIAGNOSE
# ---------------------------------------------------------------
Write-Step "Diagnosis - adapters (including hidden)..."
Get-NetAdapter -IncludeHidden | Where-Object { $_.Status -ne 'Not Present' } |
    Format-Table Name, InterfaceDescription, Status -AutoSize

$cell = Get-NetAdapter -IncludeHidden | Where-Object {
    $_.InterfaceDescription -match 'lt4132|Mobile|WWAN|LTE|Fibocom|HP.*Broadband|HSPA' -or
    $_.Name -match 'Cellular|WWAN|Mobile'
} | Select-Object -First 1

$wfd = Get-NetAdapter -IncludeHidden | Where-Object {
    $_.InterfaceDescription -match 'Wi-Fi Direct|Hosted Network|Local Area Connection\*' -or
    $_.Name -match 'Local Area Connection\*'
}

Write-Step "Diagnosis - ICS (SharedAccess) service..."
$ics = Get-Service -Name SharedAccess -ErrorAction SilentlyContinue
if ($ics) { Write-Host "   SharedAccess: $($ics.Status) (StartType: $($ics.StartType))" }
else { Write-Bad "SharedAccess service missing" }

Write-Step "Diagnosis - default routes..."
Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
    Format-Table ifIndex, NextHop, RouteMetric -AutoSize

Write-Step "Diagnosis - IPv4 forwarding..."
$fw = Get-NetIPInterface -AddressFamily IPv4 -ErrorAction SilentlyContinue |
    Select-Object InterfaceAlias, Forwarding
$fw | Format-Table -AutoSize

# ---------------------------------------------------------------
# 1. Restart ICS (this is what NAT/clients need)
# ---------------------------------------------------------------
Write-Step "Restarting Internet Connection Sharing (SharedAccess)..."
try {
    if ($ics) {
        Stop-Service SharedAccess -Force -ErrorAction Stop
        Start-Sleep -Seconds 1
        Set-Service SharedAccess -StartupType Automatic -ErrorAction SilentlyContinue
        Start-Service SharedAccess -ErrorAction Stop
        Write-Ok "SharedAccess running"
    }
}
catch { Write-Bad "SharedAccess: $($_.Exception.Message)" }

# ---------------------------------------------------------------
# 2. Enable IPv4 forwarding (NAT needs this)
# ---------------------------------------------------------------
Write-Step "Enabling IPv4 forwarding / multicast..."
netsh int ipv4 set global forwarding=enabled | Out-Null
netsh int ipv4 set global multicastforwarding=enabled | Out-Null
netsh int ipv6 set global forwarding=enabled | Out-Null
Write-Ok "forwarding enabled"

# ---------------------------------------------------------------
# 3. Fix Wi-Fi Direct virtual adapter IP (must be 192.168.137.1)
# ---------------------------------------------------------------
Write-Step "Checking hotspot private adapter (Wi-Fi Direct)..."
if ($wfd) {
    foreach ($a in $wfd) {
        $n = $a.Name
        Write-Host "   Found: $n ($($a.InterfaceDescription)) [$($a.Status)]"
        if ($a.Status -eq 'Up' -or $a.Status -eq 'Disconnected') {
            $ip = Get-NetIPAddress -InterfaceAlias $n -AddressFamily IPv4 -ErrorAction SilentlyContinue |
                Select-Object -First 1 -ExpandProperty IPAddress
            if ($ip -eq '192.168.137.1') {
                Write-Ok "$n IP = $ip (correct)"
            }
            elseif ($null -eq $ip -or $ip -eq '') {
                Write-Warn "$n has no IPv4 - setting 192.168.137.1"
                netsh interface ipv4 set address name="$n" static 192.168.137.1 255.255.255.0 | Out-Null
                Write-Ok "Set 192.168.137.1 on $n"
            }
            else {
                Write-Warn "$n IP = $ip (expected 192.168.137.1) - fixing"
                netsh interface ipv4 set address name="$n" static 192.168.137.1 255.255.255.0 | Out-Null
                Write-Ok "Reset $n to 192.168.137.1"
            }
        }
    }
}
else {
    Write-Warn "No Wi-Fi Direct adapter found (hotspot may be OFF) - will restart tethering next"
}

# ---------------------------------------------------------------
# 4. Restart hotspot services, then toggle tethering OFF/ON
# ---------------------------------------------------------------
Write-Step "Restarting hotspot services..."
Restart-Service icssvc -Force -ErrorAction SilentlyContinue
Restart-Service WlanSvc -Force -ErrorAction SilentlyContinue
Restart-Service SharedAccess -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
Write-Ok "icssvc + WlanSvc + SharedAccess restarted"

Write-Step "Toggling Mobile Hotspot OFF then ON (WinRT)..."
try {
    $null = [Windows.Networking.NetworkOperators.NetworkOperatorTetheringManager, Windows.Networking.NetworkOperators, ContentType = WindowsRuntime]
    $null = [Windows.Networking.Connectivity.NetworkInformation, Windows.Networking.Connectivity, ContentType = WindowsRuntime]

    $profile = [Windows.Networking.Connectivity.NetworkInformation]::GetInternetConnectionProfile()
    if ($profile) {
        Write-Ok "Internet profile: $($profile.ProfileName)"
        $tm = [Windows.Networking.NetworkOperators.NetworkOperatorTetheringManager]::CreateFromConnectionProfile($profile)
        Write-Ok "Tethering state: $($tm.TetheringOperatorState)"

        $op = $tm.StopTetheringAsync()
        $t = 20
        while ([int]$op.Status -eq 0 -and $t -gt 0) { Start-Sleep -Milliseconds 500; $t-- }
        Write-Ok "StopTethering status: $($op.Status) (1=done)"
        Start-Sleep -Seconds 2

        $op2 = $tm.StartTetheringAsync()
        $t = 30
        while ([int]$op2.Status -eq 0 -and $t -gt 0) { Start-Sleep -Milliseconds 500; $t-- }
        Write-Ok "StartTethering status: $($op2.Status) (1=done)"
        Start-Sleep -Seconds 3

        $tm2 = [Windows.Networking.NetworkOperators.NetworkOperatorTetheringManager]::CreateFromConnectionProfile(
            [Windows.Networking.Connectivity.NetworkInformation]::GetInternetConnectionProfile()
        )
        Write-Ok "New tethering state: $($tm2.TetheringOperatorState)"
    }
    else {
        Write-Bad "No internet connection profile found"
    }
}
catch {
    Write-Warn "WinRT toggle failed: $($_.Exception.Message)"
    Write-Warn "Manually toggle Mobile hotspot OFF, wait 3s, ON in Settings"
}

# ---------------------------------------------------------------
# 5. Re-apply virtual adapter IP + services after toggle
# ---------------------------------------------------------------
Start-Sleep -Seconds 2

Write-Step "Post-toggle: services + virtual adapter IP..."
Restart-Service SharedAccess -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1

$wfd2 = Get-NetAdapter -IncludeHidden | Where-Object {
    $_.InterfaceDescription -match 'Wi-Fi Direct|Hosted Network' -or
    $_.Name -match 'Local Area Connection\*'
}
foreach ($a in $wfd2) {
    $n = $a.Name
    $ip = Get-NetIPAddress -InterfaceAlias $n -AddressFamily IPv4 -ErrorAction SilentlyContinue |
        Select-Object -First 1 -ExpandProperty IPAddress
    if ($ip -ne '192.168.137.1') {
        netsh interface ipv4 set address name="$n" static 192.168.137.1 255.255.255.0 | Out-Null
        Write-Ok "$n -> 192.168.137.1 (was: $ip)"
    }
    else {
        Write-Ok "$n = 192.168.137.1"
    }
}

# Ensure DHCP-ish DNS on private side is fine; flush
ipconfig /flushdns | Out-Null

# ---------------------------------------------------------------
# 6. Firewall - allow ICS / tethering related rules if disabled
# ---------------------------------------------------------------
Write-Step "Checking firewall rules for ICS/hotspot..."
$rules = Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object {
    $_.DisplayName -match 'Internet Connection Sharing|ICS|Mobile Hotspot|Hotspot|SharedAccess' -or
    $_.Name -match 'SharedAccess|Ics'
}
if ($rules) {
    foreach ($r in $rules) {
        if ($r.Enabled -eq 'False') {
            Enable-NetFirewallRule -Name $r.Name -ErrorAction SilentlyContinue
            Write-Ok "Enabled firewall rule: $($r.DisplayName)"
        }
    }
    Write-Ok "Firewall ICS/hotspot rules checked ($($rules.Count) matched)"
}
else {
    Write-Warn "No ICS-named firewall rules found (normal on some Win11 builds)"
}

# Also ensure firewall allows ICMP/forwarding profile for private
Get-NetFirewallProfile | Where-Object { $_.Name -in 'Private','Domain' } |
    ForEach-Object {
        if ($_.Inbound -eq 'Block' -or $_.DefaultInboundAction -eq 'Block') {
            Write-Host "   Profile $($_.Name) inbound=Block (OK for clients)" -ForegroundColor DarkGray
        }
    }

# ---------------------------------------------------------------
# FINAL STATUS
# ---------------------------------------------------------------
Write-Host "`n============================================" -ForegroundColor White
Write-Host " FINAL STATUS" -ForegroundColor White
Write-Host "============================================" -ForegroundColor White

Get-NetAdapter -IncludeHidden | Where-Object { $_.Status -eq 'Up' } |
    Format-Table Name, InterfaceDescription, Status -AutoSize

if ($cell) {
    $cst = (Get-NetAdapter -Name $cell.Name).Status
    Write-Host "Cellular: $cst" -ForegroundColor $(if ($cst -eq 'Up') { 'Green' } else { 'Red' })
}
if ($wfd2) {
    Write-Host "Hotspot virtual adapter: present" -ForegroundColor Green
}
else {
    Write-Host "Hotspot virtual adapter: missing - turn hotspot ON in Settings" -ForegroundColor Yellow
}

$icsNow = (Get-Service SharedAccess -ErrorAction SilentlyContinue).Status
Write-Host "ICS (SharedAccess): $icsNow" -ForegroundColor $(if ($icsNow -eq 'Running') { 'Green' } else { 'Red' })

Write-Host @"

Done (no reboot needed).

If clients still have no internet:
  1. Settings > Network > Mobile hotspot > turn OFF, wait 5s, turn ON
  2. On client: forget hotspot, rejoin, or renew DHCP
     (cmd on client: ipconfig /release && ipconfig /renew)
  3. On THIS pc, confirm cellular still opens websites
"@ -ForegroundColor Yellow

Read-Host "Press Enter to close"
