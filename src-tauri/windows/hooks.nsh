; Roommate NSIS hooks - register/stop RoommateNetworkService around install/uninstall.
; Note: sc.exe requires a space after '='. binPath must be ONE quoted value that includes args.

!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Stopping RoommateNetworkService (if present)..."
  nsExec::ExecToLog 'sc.exe stop RoommateNetworkService'
  Sleep 1500
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Installing RoommateNetworkService..."
  nsExec::ExecToLog 'sc.exe stop RoommateNetworkService'
  Sleep 800
  nsExec::ExecToLog 'sc.exe delete RoommateNetworkService'
  Sleep 800
  ; Prefer unquoted path when possible; NSIS $INSTDIR rarely has spaces.
  ; Full BinaryPathName must include --roommate-service as part of the same value.
  nsExec::ExecToLog 'sc.exe create RoommateNetworkService binPath= "$INSTDIR\Roommate-LAN.exe --roommate-service" start= auto DisplayName= "Roommate Network Service" obj= LocalSystem'
  Pop $0
  ${If} $0 != 0
    DetailPrint "sc create failed with code $0"
    MessageBox MB_ICONSTOP "Failed to register Roommate network service (error $0). Setup cannot continue."
    SetErrorLevel 1
    Quit
  ${EndIf}
  nsExec::ExecToLog 'sc.exe description RoommateNetworkService "Hosts Roommate private Tailscale sidecar for Steam LAN."'
  nsExec::ExecToLog 'sc.exe failure RoommateNetworkService reset= 86400 actions= restart/5000/restart/10000/restart/30000'
  nsExec::ExecToLog 'sc.exe start RoommateNetworkService'
  Pop $0
  ${If} $0 != 0
    DetailPrint "sc start failed with code $0 - service may start after reboot"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Stopping RoommateNetworkService..."
  nsExec::ExecToLog 'sc.exe stop RoommateNetworkService'
  Sleep 2000
  nsExec::ExecToLog 'sc.exe delete RoommateNetworkService'
  Sleep 800
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DetailPrint "RoommateNetworkService removed."
!macroend
