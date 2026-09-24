; NetworkFix NSIS installer — builds dist/NetworkFix-0.1.0-Setup.exe
; Bundles networkfix.exe (CLI) + networkfix-gui.exe (GUI).
; PATH: no EnVar plugin — Start Menu folder with GUI shortcut, CLI shortcut,
; and a networkfix.cmd wrapper that invokes the CLI from the install dir.
!define NAME "NetworkFix"
!define VERSION "0.1.0"

Name "${NAME}"
OutFile "..\..\dist\NetworkFix-${VERSION}-Setup.exe"
InstallDir "$PROGRAMFILES64\${NAME}"
InstallDirRegKey HKLM "Software\${NAME}" "InstallDir"
RequestExecutionLevel admin
Page directory
Page components
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "NetworkFix (GUI + CLI)" SecMain
  SetShellVarContext all
  SetOutPath "$INSTDIR"
  File "..\..\target\release\networkfix.exe"
  File "..\..\target\release\networkfix-gui.exe"
  File "networkfix.cmd"

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "Software\${NAME}" "InstallDir" "$INSTDIR"

  CreateDirectory "$SMPROGRAMS\${NAME}"
  CreateShortCut "$SMPROGRAMS\${NAME}\NetworkFix GUI.lnk" "$INSTDIR\networkfix-gui.exe"
  CreateShortCut "$SMPROGRAMS\${NAME}\NetworkFix CLI.lnk" "$INSTDIR\networkfix.cmd"
  CreateShortCut "$DESKTOP\NetworkFix.lnk" "$INSTDIR\networkfix-gui.exe"
SectionEnd

Section "Uninstall"
  SetShellVarContext all
  Delete "$INSTDIR\networkfix.exe"
  Delete "$INSTDIR\networkfix-gui.exe"
  Delete "$INSTDIR\networkfix.cmd"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\${NAME}\NetworkFix GUI.lnk"
  Delete "$SMPROGRAMS\${NAME}\NetworkFix CLI.lnk"
  RMDir "$SMPROGRAMS\${NAME}"
  Delete "$DESKTOP\NetworkFix.lnk"
  DeleteRegKey HKLM "Software\${NAME}"
  RMDir "$INSTDIR"
SectionEnd
