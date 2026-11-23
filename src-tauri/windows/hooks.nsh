; Lampion's additions to Tauri's NSIS installer (bundle > windows > nsis > installerHooks in tauri.conf.json).
;
; - Installs into %LOCALAPPDATA%\Programs\Lampion, without administrator rights. Tauri's per-user default,
;   %LOCALAPPDATA%\Lampion, is the folder of Lampion's own log, settings caches and downloaded updates.
; - Offers the Microsoft Visual C++ 2015-2022 runtime (x64) that Apache and PHP need, when it is missing
;   (not during an update), and runs the download only when Windows confirms that Microsoft signed it.
;   Lampion's first setup offers it again if it was declined or refused here.
; - Uninstalling removes the LampionApache and LampionMariaDB Windows services, if they were installed, and
;   says that the Lampion folder (C:\Lampion: servers, websites, databases) is kept.
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
        ; Runs with administrator rights, so only a valid Authenticode signature of Microsoft Corporation with a
        ; chain that ends in a Microsoft root will do (Lampion's own setup checks the same). The path reaches
        ; PowerShell through the environment, so no character in it can break the command; any answer but 0
        ; ("error" included) counts as not signed.
        Push "$TEMP\lampion_vc_redist.x64.exe"
        System::Call 'kernel32::SetEnvironmentVariable(t "LAMPION_VC_REDIST", t s)'
        nsExec::Exec `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -Command "$$s = Get-AuthenticodeSignature -LiteralPath $$env:LAMPION_VC_REDIST; $$c = $$s.SignerCertificate; if ($$s.Status -eq 'Valid' -and $$c.GetNameInfo('SimpleName', $$false) -eq 'Microsoft Corporation') { $$ch = New-Object Security.Cryptography.X509Certificates.X509Chain; $$ch.ChainPolicy.RevocationMode = 'NoCheck'; [void]$$ch.Build($$c); if ($$ch.ChainElements[$$ch.ChainElements.Count - 1].Certificate.GetNameInfo('SimpleName', $$false) -like 'Microsoft *') { exit 0 } }; exit 1"`
        Pop $3
        ${If} $3 == "0"
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
          Delete "$TEMP\lampion_vc_redist.x64.exe"
          DetailPrint "The downloaded Visual C++ runtime installer is not validly signed by Microsoft, so it was not run."
          ${If} $PassiveMode <> 1
            MessageBox MB_YESNO|MB_ICONEXCLAMATION "The downloaded Microsoft Visual C++ runtime installer is not validly signed by Microsoft, so Lampion did not run it.$\r$\n$\r$\nApache and PHP need this runtime. Download it yourself from Microsoft's page:$\r$\nhttps://learn.microsoft.com/cpp/windows/latest-supported-vc-redist$\r$\n$\r$\nOpen the page now? (Lampion also offers the runtime again when it sets up the servers.)" /SD IDNO IDNO +2
            ExecShell "open" "https://learn.microsoft.com/cpp/windows/latest-supported-vc-redist"
          ${EndIf}
        ${EndIf}
      ${Else}
        DetailPrint "The Visual C++ runtime could not be downloaded ($1). Lampion offers it again when it sets up the servers."
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ; General › Run the servers as Windows services: the services LampionApache and LampionMariaDB start
    ; with Windows from the Lampion folder, so they go with Lampion (one Windows permission prompt; not
    ; in a silent uninstall, which nobody could answer).
    ClearErrors
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Services\LampionApache" "ImagePath"
    ${If} ${Errors}
      ClearErrors
      ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Services\LampionMariaDB" "ImagePath"
    ${EndIf}
    ${IfNot} ${Errors}
    ${AndIfNot} ${Silent}
      DetailPrint "Removing the LampionApache and LampionMariaDB services (Windows asks for permission)..."
      ExecShellWait "runas" "$SYSDIR\cmd.exe" '/d /c "sc.exe stop LampionApache & sc.exe delete LampionApache & sc.exe stop LampionMariaDB & sc.exe delete LampionMariaDB"' SW_HIDE
    ${EndIf}
    ${If} $PassiveMode <> 1
      MessageBox MB_OK|MB_ICONINFORMATION "Lampion's folder with your servers, websites and databases is kept: C:\Lampion, or the folder you moved it to.$\r$\n$\r$\nApache and MariaDB no longer start with Windows; if they are running now, they stop when you restart Windows. Delete the folder yourself when you no longer need it." /SD IDOK
    ${EndIf}
  ${EndIf}
!macroend
