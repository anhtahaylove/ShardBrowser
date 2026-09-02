; Tauri's stock process helper terminates the process but does not wait for the
; executable handle to be released. A silent upgrade can then skip the locked
; file while still updating the uninstall registry. Delete the resolved target
; first and fail closed if the exact installed executable cannot be replaced.

!ifndef SHARDX_REPLACE_RETRY_LIMIT
  !define SHARDX_REPLACE_RETRY_LIMIT 40
!endif

!ifndef SHARDX_REPLACE_RETRY_DELAY_MS
  !define SHARDX_REPLACE_RETRY_DELAY_MS 250
!endif

!macro NSIS_HOOK_PREINSTALL
  ; Keep Tauri's normal prompt and current-user process filtering. The stock
  ; check later in installer.nsi becomes a harmless second guard.
  !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"

  !define SHARDX_PREINSTALL_ID ${__LINE__}
  StrCpy $R8 0

  shardx_wait_for_exit_${SHARDX_PREINSTALL_ID}:
    !if "${INSTALLMODE}" == "currentUser"
      nsis_tauri_utils::FindProcessCurrentUser "${MAINBINARYNAME}.exe"
    !else
      nsis_tauri_utils::FindProcess "${MAINBINARYNAME}.exe"
    !endif
    Pop $R9
    ${If} $R9 = 0
      IntOp $R8 $R8 + 1
      ${If} $R8 >= ${SHARDX_REPLACE_RETRY_LIMIT}
        DetailPrint "ShardX Launcher did not exit in time; the installed executable was not changed."
        SetErrorLevel 1
        Abort "ShardX Launcher could not be stopped. Close it and retry the upgrade."
      ${EndIf}
      Sleep ${SHARDX_REPLACE_RETRY_DELAY_MS}
      Goto shardx_wait_for_exit_${SHARDX_PREINSTALL_ID}
    ${EndIf}

  StrCpy $R8 0
  shardx_delete_old_exe_${SHARDX_PREINSTALL_ID}:
    SetFileAttributes "$INSTDIR\${MAINBINARYNAME}.exe" NORMAL
    ClearErrors
    Delete "$INSTDIR\${MAINBINARYNAME}.exe"
    ${If} ${Errors}
      IntOp $R8 $R8 + 1
      ${If} $R8 >= ${SHARDX_REPLACE_RETRY_LIMIT}
        DetailPrint "The existing executable is still locked; the installed executable was not changed."
        SetErrorLevel 1
        Abort "ShardX Launcher could not replace its installed executable. Close it and retry the upgrade."
      ${EndIf}
      Sleep ${SHARDX_REPLACE_RETRY_DELAY_MS}
      Goto shardx_delete_old_exe_${SHARDX_PREINSTALL_ID}
    ${EndIf}
  !undef SHARDX_PREINSTALL_ID
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ClearErrors
  ${GetFileVersion} "$INSTDIR\${MAINBINARYNAME}.exe" $R8
  ${If} ${Errors}
    DetailPrint "The installed executable is missing or has no readable version."
    SetErrorLevel 1
    Abort "ShardX Launcher could not verify the installed executable. Retry the upgrade."
  ${ElseIf} $R8 != "${VERSIONWITHBUILD}"
    DetailPrint "Installed executable version $R8 does not match ${VERSIONWITHBUILD}."
    SetErrorLevel 1
    Abort "ShardX Launcher installed the wrong executable version. Retry the upgrade."
  ${EndIf}
!macroend
