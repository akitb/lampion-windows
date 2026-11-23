; Lampion's additions to Tauri's NSIS installer (bundle > windows > nsis > installerHooks in tauri.conf.json).
;
; - Installs into %LOCALAPPDATA%\Programs\Lampion, without administrator rights. Tauri's per-user default,
;   %LOCALAPPDATA%\Lampion, is the folder of Lampion's own log, settings caches and downloaded updates.
; - Offers the Microsoft Visual C++ 2015-2022 runtime (x64) that Apache and PHP need, when it is missing
;   (not during an update). Lampion's first setup offers it again if it was declined here.
; - Uninstalling says that the Lampion folder (C:\Lampion: servers, websites, databases) is kept.
;
; The updater runs the installer as `Lampion-X.Y-setup.exe /S /UPDATE /R`: silent, no new shortcuts, and
; Tauri's template starts Lampion again when it is done (/R).

; Functions are compiled here, before the template defines PRODUCTNAME, so the names are written out.
; .onGUIInit runs after .onInit chose the folder and before the directory page shows it.
!define MUI_CUSTOMFUNCTION_GUIINIT LampionInstallFolder

Function LampionInstallFolder
  ${If} $INSTDIR == "$LOCALAPPDATA\Lampion"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\Lampion"
  ${EndIf}
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  ; A silent install has no .onGUIInit; the template already set the output folder, so set it again.
  Call LampionInstallFolder
  SetOutPath $INSTDIR

  ${IfNot} ${Silent}
  ${AndIf} $UpdateMode <> 1
    ; The template's SetContext reads the 64-bit registry view on 64-bit Windows.
    ClearErrors
    ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Installed"
    ${If} ${Errors}
    ${OrIf} $0 <> 1
      DetailPrint "Downloading the Microsoft Visual C++ runtime that Apache and PHP need..."
      NSISdl::download "https://aka.ms/vc14/vc_redist.x64.exe" "$TEMP\lampion_vc_redist.x64.exe"
      Pop $1
      ${If} $1 == "success"
        DetailPrint "Installing the Visual C++ runtime (Windows asks for permission)..."
        ExecWait '"$TEMP\lampion_vc_redist.x64.exe" /install /passive /norestart' $2
        Delete "$TEMP\lampion_vc_redist.x64.exe"
        ${If} $2 = 0
        ${OrIf} $2 = 1638
        ${OrIf} $2 = 3010
          DetailPrint "The Visual C++ runtime is installed."
        ${Else}
          DetailPrint "The Visual C++ runtime was not installed (code $2). Lampion offers it again when it sets up the servers."
        ${EndIf}
      ${Else}
        DetailPrint "The Visual C++ runtime could not be downloaded ($1). Lampion offers it again when it sets up the servers."
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
    MessageBox MB_OK|MB_ICONINFORMATION "Lampion's folder with your servers, websites and databases is kept: C:\Lampion, or the folder you moved it to.$\r$\n$\r$\nServers that are still running keep running until you restart Windows. Delete the folder yourself when you no longer need it." /SD IDOK
  ${EndIf}
!macroend
