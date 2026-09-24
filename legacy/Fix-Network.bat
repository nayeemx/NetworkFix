@echo off
title Network Fix - Cellular + Hotspot (No Reboot)
color 0A

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Requesting administrator privileges...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

echo ============================================
echo  Network Fix Menu - pick an option
echo ============================================
echo  [1] Quick  - restart cellular + hotspot services (fastest)
echo  [2] Full   - also Wi-Fi, Winsock/IP reset, deeper clean
echo  [3] Hotspot only - restart Wi-Fi + hotspot, keep cellular
echo  [4] Hotspot has clients but NO internet (ICS/NAT fix)
echo  [Q] Quit
echo ============================================
choice /C 1234Q /N /M "Choose (1/2/3/4/Q): "

if errorlevel 5 goto :quit
if errorlevel 4 goto :nointernet
if errorlevel 3 goto :hotspot
if errorlevel 2 goto :full
if errorlevel 1 goto :quick

:quick
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fix-network.ps1" -Mode Quick
goto :end

:full
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fix-network.ps1" -Mode Full
goto :end

:hotspot
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fix-network.ps1" -Mode HotspotOnly
goto :end

:nointernet
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fix-hotspot-internet.ps1"
goto :end

:quit
exit /b 0

:end
endlocal
