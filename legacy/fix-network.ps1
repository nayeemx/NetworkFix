#Requires -Version 5.1
<#
  Cellular + Mobile Hotspot fix for HP EliteBook (Windows 11)
  Fixes without reboot: WWAN/Wi-Fi adapter restart, hotspot service,
  network services, DNS, Winsock, IP stack.
#>

[CmdletBinding()]
param(
    [ValidateSet('Quick', 'Full', 'HotspotOnly')]
    [string]$Mode = 'Quick'
)

$ErrorActionPreference = 'SilentlyContinue'

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-Admin)) {
    Write-Host "Requesting administrator privileges..." -ForegroundColor Yellow
    Start-Process powershell.exe -Verb RunAs -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`" -Mode $Mode"
    exit
}

function Write-Step($msg) { Write-Host "`n>> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "   OK: $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "   WARN: $msg" -ForegroundColor Yellow }

Write-Host "============================================" -ForegroundColor White
Write-Host " Network Fix Script (no reboot required)" -ForegroundColor White
Write-Host " Mode: $Mode" -ForegroundColor White
Write-Host "============================================" -ForegroundColor White

# ---------------------------------------------------------------
# 1. Restart Mobile Hotspot service (icssvc)
# ---------------------------------------------------------------
Write-Step "Restarting Mobile Hotspot service (icssvc)..."
try {
    $svc = Get-Service -Name icssvc -ErrorAction Stop
    if ($svc.Status -eq 'Running') {
        Stop-Service icssvc -Force
        Start-Sleep -Milliseconds 800
    }
    Start-Service icssvc
    Start-Sleep -Milliseconds 500
    Write-Ok "icssvc restarted (state: $((Get-Service icssvc).Status))"
}
catch { Write-Warn "icssvc: $($_.Exception.Message)" }

# ---------------------------------------------------------------
# 2. Restart WLAN / network related services
# ---------------------------------------------------------------
Write-Step "Restarting WLAN and network services..."
$services = @(
    'WlanSvc',        # WLAN AutoConfig (hotspot depends on this)
    'SharedAccess',   # ICS / NAT - required so clients get internet
    'EapHost',        # Extensible auth
    'dot3svc',        # Wired auto config (harmless)
    'RasMan',         # Remote access / dial-up (used by some WWAN)
    'PhoneSvc',       # Telephony (WWAN related)
    'tapisrv'
)
foreach ($s in $services) {
    $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
    if ($null -eq $svc) { continue }
    try {
        if ($svc.Status -eq 'Running') {
            Stop-Service $s -Force -ErrorAction Stop
            Start-Sleep -Milliseconds 400
        }
        Start-Service $s -ErrorAction Stop
        Write-Ok "$s restarted"
    }
    catch { Write-Warn "$s : $($_.Exception.Message)" }
}

netsh int ipv4 set global forwarding=enabled | Out-Null
netsh int ipv4 set global multicastforwarding=enabled | Out-Null
Write-Ok "IPv4 forwarding enabled"

# ---------------------------------------------------------------
# 3. Restart cellular (WWAN) and Wi-Fi adapters
# ---------------------------------------------------------------
Write-Step "Finding cellular / Wi-Fi adapters..."

$wwanPatterns  = 'Mobile|WWAN|WWAN Adapter|Fibocom|HP Mobile|Sierra|Quectel|MediaTek.*Mobile|Intel.*LTE|Broadband'
$wifiPatterns  = 'Wi-Fi|Wireless|WLAN|802\.11|Realtek.*Wireless|Intel.*Wireless|Qualcomm.*Wireless'

$allAdapters = Get-NetAdapter -IncludeHidden -ErrorAction SilentlyContinue |
    Where-Object { $_.Status -ne 'Not Present' }

$wwan = $allAdapters | Where-Object {
    $_.InterfaceDescription -match $wwanPatterns -or $_.Name -match 'Cellular|WWAN|Mobile'
}
$wifi = $allAdapters | Where-Object {
    $_.InterfaceDescription -match $wifiPatterns -or $_.Name -match 'Wi-Fi'
}

if (-not $wwan -and -not $wifi) {
    Write-Warn "No network adapters found - unexpected."
    Get-NetAdapter | Format-Table Name, InterfaceDescription, Status
}

function Restart-AdapterSafe($adapter, $label) {
    if ($null -eq $adapter) { return }
    $name = $adapter.Name
    Write-Step "Restarting $label adapter: $name ($($adapter.InterfaceDescription))"
    try {
        Disable-NetAdapter -Name $name -Confirm:$false -ErrorAction Stop
        Write-Ok "Disabled $name"
        Start-Sleep -Seconds 2
        Enable-NetAdapter -Name $name -Confirm:$false -ErrorAction Stop
        Write-Ok "Enabled $name"
        # wait for link
        $deadline = (Get-Date).AddSeconds(15)
        do {
            Start-Sleep -Seconds 1
            $st = (Get-NetAdapter -Name $name -ErrorAction SilentlyContinue).Status
        } while ($st -eq 'Disconnected' -and (Get-Date) -lt $deadline)
        Write-Ok "$name state: $st"
    }
    catch {
        Write-Warn "$label restart failed: $($_.Exception.Message)"
        # fallback via netsh
        netsh interface set interface name="$name" admin=disable  | Out-Null
        Start-Sleep -Seconds 2
        netsh interface set interface name="$name" admin=enable   | Out-Null
        Write-Warn "Tried netsh fallback for $name"
    }
}

# Restart WWAN always (Quick + Full)
Restart-AdapterSafe $wwan 'Cellular/WWAN'

if ($Mode -eq 'Full' -or $Mode -eq 'HotspotOnly') {
    Restart-AdapterSafe $wifi 'Wi-Fi'
}

# ---------------------------------------------------------------
# 4. Flush DNS + renew cellular IP
# ---------------------------------------------------------------
Write-Step "Flushing DNS..."
ipconfig /flushdns | Out-Null
Write-Ok "DNS cache flushed"

if ($wwan) {
    Write-Step "Renewing IP on cellular adapter..."
    $ifIndex = (Get-NetAdapter -Name $wwan.Name).ifIndex
    # release + renew only on that interface
    netsh interface ipv4 set address name="$($wwan.Name)" source=dhcp | Out-Null
    netsh interface ipv4 set dnsservers name="$($wwan.Name)" source=dhcp | Out-Null
    Start-Sleep -Seconds 3
    $ip = (Get-NetIPAddress -InterfaceIndex $ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue |
           Where-Object { $_.IPAddress -ne '127.0.0.1' } | Select-Object -First 1).IPAddress
    if ($ip) { Write-Ok "Cellular IPv4: $ip" }
    else     { Write-Warn "No IPv4 yet on cellular (may still be connecting)" }
}

# ---------------------------------------------------------------
# 5. Full mode: Winsock + IP stack reset (still no reboot needed
#    for most cases; reboot only if something is deeply broken)
# ---------------------------------------------------------------
if ($Mode -eq 'Full') {
    Write-Step "Resetting Winsock and IP stack (no reboot needed for most cases)..."
    netsh winsock reset | Out-Null
    Write-Ok "Winsock reset"
    netsh int ip reset | Out-Null
    Write-Ok "IP stack reset"
    netsh int tcp reset | Out-Null
    Write-Ok "TCP stack reset"

    # Re-apply adapter restarts after stack reset
    Restart-AdapterSafe $wwan 'Cellular/WWAN'
    Restart-AdapterSafe $wifi 'Wi-Fi'

    # restart hotspot service again
    Restart-Service icssvc -Force -ErrorAction SilentlyContinue
    Restart-Service WlanSvc  -Force -ErrorAction SilentlyContinue
    Write-Ok "Hotspot services restarted again"
}

# ---------------------------------------------------------------
# 6. Re-enable Mobile Hotspot (optional automatic)
# ---------------------------------------------------------------
Write-Step "Checking Mobile Hotspot status..."
try {
    $hotspotOn = Get-NetAdapter -IncludeHidden | Where-Object {
        $_.Name -match 'Local Area Connection\*' -or
        $_.InterfaceDescription -match 'Wi-Fi Direct|Hosted Network'
    }
    if ($hotspotOn) {
        Write-Ok "Wi-Fi Direct virtual adapter present: $($hotspotOn.Name -join ', ')"
        Write-Host "   Turn hotspot back ON in: Settings > Network > Mobile hotspot" -ForegroundColor Yellow
    }
    else {
        Write-Warn "No Wi-Fi Direct virtual adapter found - toggle hotspot ON in Settings"
    }
}
catch { }

# ---------------------------------------------------------------
# Status summary
# ---------------------------------------------------------------
Write-Host "`n============================================" -ForegroundColor White
Write-Host " STATUS" -ForegroundColor White
Write-Host "============================================" -ForegroundColor White
Get-NetAdapter -IncludeHidden | Where-Object { $_.Status -ne 'Not Present' -and $_.Status -ne 'Disabled' } |
    Format-Table Name, InterfaceDescription, Status, LinkSpeed -AutoSize

$cell = Get-NetAdapter -IncludeHidden | Where-Object { $_.InterfaceDescription -match $wwanPatterns -or $_.Name -match 'Cellular|WWAN|Mobile' }
if ($cell -and $cell.Status -eq 'Up') {
    Write-Host "Cellular: UP" -ForegroundColor Green
} else {
    Write-Host "Cellular: not Up - try Mode Full or check SIM/antenna" -ForegroundColor Red
}

Write-Host "`nDone. Usually no restart needed." -ForegroundColor Green
Write-Host "If hotspot still fails: toggle Mobile hotspot OFF/ON in Settings once." -ForegroundColor Yellow

if ($Mode -eq 'Full') {
    Write-Host "If still broken after Full mode: a reboot may help (Winsock/IP reset)." -ForegroundColor Yellow
}

Read-Host "`nPress Enter to close"
