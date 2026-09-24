@echo off
title Hotspot Fix - Connected but No Internet
color 0E

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Requesting administrator privileges...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

echo ============================================
echo  Hotspot clients connected but NO internet?
echo  (PC has internet, joined devices do not)
echo ============================================
echo  This will:
echo   - restart ICS (SharedAccess) for NAT
echo   - fix virtual adapter IP 192.168.137.1
echo   - enable IPv4 forwarding
echo   - toggle Mobile hotspot OFF/ON
echo   - check firewall ICS rules
echo ============================================
pause

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0fix-hotspot-internet.ps1"
