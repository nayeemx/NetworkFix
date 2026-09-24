[CmdletBinding()]
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Write-Host "== NetworkFix Windows installers =="

if (-not $SkipBuild) {
    Write-Host "cargo build --release -p networkfix -p networkfix-gui"
    cargo build --release -p networkfix -p networkfix-gui
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release failed (exit $LASTEXITCODE)"
    }
}

$stage = Join-Path $root "dist\windows\stage"
New-Item -ItemType Directory -Force -Path (Join-Path $root "dist") | Out-Null
New-Item -ItemType Directory -Force -Path $stage | Out-Null

foreach ($exe in @("networkfix.exe", "networkfix-gui.exe")) {
    $src = Join-Path $root "target\release\$exe"
    if (-not (Test-Path $src)) {
        throw "missing release binary: $src"
    }
    Copy-Item -Force -Path $src -Destination $stage
}
$icoSrc = Join-Path $root "packaging\windows\networkfix.ico"
if (-not (Test-Path $icoSrc)) {
    throw "missing icon: $icoSrc"
}
Copy-Item -Force -Path $icoSrc -Destination $stage
Write-Host "Staged release binaries + icon in $stage"

$artifacts = @()

# --- NSIS -------------------------------------------------------------
$makensis = Get-Command makensis -ErrorAction SilentlyContinue
if ($makensis) {
    Write-Host "Running makensis..."
    $nsi = Join-Path $root "packaging\windows\networkfix.nsi"
    & $makensis.Source $nsi
    if ($LASTEXITCODE -eq 0) {
        $nsisOut = Join-Path $root "dist\NetworkFix-0.1.0-Setup.exe"
        if (Test-Path $nsisOut) {
            $artifacts += (Get-Item $nsisOut).FullName
        } else {
            Write-Warning "makensis succeeded but $nsisOut not found"
        }
    } else {
        Write-Warning "makensis exited with $LASTEXITCODE - NSIS installer not produced"
    }
} else {
    Write-Warning "makensis not found - skipping NSIS installer (install NSIS, or CI will build it)"
}

# --- MSI --------------------------------------------------------------
$candle = Get-Command candle -ErrorAction SilentlyContinue
$light = Get-Command light -ErrorAction SilentlyContinue
if ($candle -and $light) {
    Write-Host "Running WiX candle/light..."
    $wixObjDir = Join-Path $root "dist\windows\wix"
    New-Item -ItemType Directory -Force -Path $wixObjDir | Out-Null
    $wxs = Join-Path $root "packaging\windows\wix\main.wxs"
    $obj = Join-Path $wixObjDir "main.wixobj"
    $msi = Join-Path $root "dist\NetworkFix-0.1.0.msi"

    & $candle.Source -nologo -arch x64 "-dStageDir=$stage" "-dIconPath=$icoSrc" -out $obj $wxs
    if ($LASTEXITCODE -eq 0) {
        & $light.Source -nologo -out $msi $obj
        if ($LASTEXITCODE -eq 0 -and (Test-Path $msi)) {
            $artifacts += (Get-Item $msi).FullName
        } else {
            Write-Warning "light exited with $LASTEXITCODE - MSI not produced"
        }
    } else {
        Write-Warning "candle exited with $LASTEXITCODE - MSI not produced"
    }
} else {
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    cargo wix --help 2>$null | Out-Null
    $hasCargoWix = ($LASTEXITCODE -eq 0)
    $ErrorActionPreference = $prevEap

    if ($hasCargoWix) {
        Write-Host "Attempting cargo wix (packages one crate only; manual main.wxs is preferred)..."
        cargo wix -p networkfix-gui --nologo
        if ($LASTEXITCODE -eq 0) {
            $found = Get-ChildItem -Path (Join-Path $root "target\wix") -Filter *.msi -Recurse -ErrorAction SilentlyContinue
            foreach ($f in $found) { $artifacts += $f.FullName }
        } else {
            Write-Warning "cargo wix failed (exit $LASTEXITCODE) - MSI not produced"
        }
    } else {
        Write-Warning "WiX toolset (candle/light) and cargo-wix not found - skipping MSI (install WiX v3 or 'cargo install cargo-wix'; CI will build it)"
    }
}

# --- Artifacts --------------------------------------------------------
Write-Host ""
Write-Host "Release binaries:"
Get-Item (Join-Path $root "target\release\networkfix.exe"), (Join-Path $root "target\release\networkfix-gui.exe") -ErrorAction SilentlyContinue |
    ForEach-Object { Write-Host "  $($_.FullName) ($([math]::Round($_.Length/1MB,1)) MB)" }

Write-Host "Installer artifacts:"
$others = Get-ChildItem -Path (Join-Path $root "dist"), (Join-Path $root "target\wix") -Recurse -Include *.msi, *Setup.exe -ErrorAction SilentlyContinue
$all = @($artifacts) + @($others | ForEach-Object { $_.FullName }) | Sort-Object -Unique
if ($all.Count -eq 0) {
    Write-Warning "No installer artifacts produced (makensis/WiX tools missing) - release binaries built OK"
} else {
    foreach ($a in $all) { Write-Host "  $a" }
}
