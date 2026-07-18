[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$configPath = Join-Path $repoRoot "src-tauri\tauri.conf.json"
$config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
$configuredHook = $config.bundle.windows.nsis.installerHooks
if ([string]::IsNullOrWhiteSpace($configuredHook)) {
    throw "tauri.conf.json does not configure bundle.windows.nsis.installerHooks."
}

$tauriRoot = Join-Path $env:LOCALAPPDATA "tauri\NSIS"
$makeNsis = Join-Path $tauriRoot "Bin\makensis.exe"
$includeDir = Join-Path $tauriRoot "Include"
$pluginDir = Join-Path $tauriRoot "Plugins\x86-unicode\additional"
$hookPath = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $configPath) $configuredHook))

foreach ($required in @($makeNsis, $includeDir, $pluginDir, $hookPath)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "Required NSIS test input is missing: $required"
    }
}

$fixtureRoot = Join-Path $env:TEMP ("shardx-nsis-upgrade-" + [guid]::NewGuid().ToString("N"))
$installDir = Join-Path $fixtureRoot "installed"
$scriptPath = Join-Path $fixtureRoot "fixture.nsi"
$installerPath = Join-Path $fixtureRoot "fixture-setup.exe"
$mismatchScriptPath = Join-Path $fixtureRoot "fixture-mismatch.nsi"
$mismatchInstallerPath = Join-Path $fixtureRoot "fixture-mismatch-setup.exe"
$installedExe = Join-Path $installDir "shardx-nsis-fixture.exe"
$payloadSource = Join-Path $env:WINDIR "System32\where.exe"
$payloadExe = Join-Path $fixtureRoot "payload.exe"
$oldExe = $env:ComSpec
$fixtureProcess = $null
$lockedFile = $null

try {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item -LiteralPath $oldExe -Destination $installedExe -Force
    Copy-Item -LiteralPath $payloadSource -Destination $payloadExe -Force

    $payloadVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($payloadExe)
    $expectedVersion = "{0}.{1}.{2}.{3}" -f $payloadVersion.FileMajorPart,
        $payloadVersion.FileMinorPart,
        $payloadVersion.FileBuildPart,
        $payloadVersion.FilePrivatePart

    $escaped = {
        param([string]$Value)
        return $Value.Replace("$", "$$").Replace('"', '$\"')
    }

    $nsi = @"
Unicode true
SilentInstall silent
RequestExecutionLevel user

!include "LogicLib.nsh"
!include "FileFunc.nsh"

!define INSTALLMODE "currentUser"
!define MAINBINARYNAME "shardx-nsis-fixture"
!define PRODUCTNAME "ShardX NSIS Upgrade Fixture"
!define VERSIONWITHBUILD "$expectedVersion"
!define SHARDX_REPLACE_RETRY_LIMIT 20
!define SHARDX_REPLACE_RETRY_DELAY_MS 50

!addplugindir "$(& $escaped $pluginDir)"

; Minimal copy of the stock Tauri silent process stop used by this fixture.
!macro CheckIfAppIsRunning executableName productName
  nsis_tauri_utils::FindProcessCurrentUser "`$`{executableName}"
  Pop `$R0
  `$`{If} `$R0 = 0
    nsis_tauri_utils::KillProcessCurrentUser "`$`{executableName}"
    Pop `$R0
    `$`{If} `$R0 != 0
    `$`{AndIf} `$R0 != 2
      SetErrorLevel 1
      Abort "Could not stop `$`{productName}."
    `$`{EndIf}
    Sleep 500
  `$`{EndIf}
!macroend

!include "$(& $escaped $hookPath)"

OutFile "$(& $escaped $installerPath)"
InstallDir "$(& $escaped $installDir)"

Section
  SetOutPath `$INSTDIR
  !insertmacro NSIS_HOOK_PREINSTALL
  !insertmacro CheckIfAppIsRunning "`$`{MAINBINARYNAME}.exe" "`$`{PRODUCTNAME}"
  File "/oname=`$`{MAINBINARYNAME}.exe" "$(& $escaped $payloadExe)"
  !insertmacro NSIS_HOOK_POSTINSTALL
SectionEnd
"@
    Set-Content -LiteralPath $scriptPath -Value $nsi -Encoding utf8

    & $makeNsis "/INPUTCHARSET" "UTF8" "/WX" "/V2" "/XSetCompressor /FINAL lzma" $scriptPath
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $installerPath)) {
        throw "makensis failed to build the upgrade fixture."
    }

    $fixtureProcess = Start-Process -FilePath $installedExe -ArgumentList "/d", "/c", "ping 127.0.0.1 -t >nul" -WindowStyle Hidden -PassThru
    Start-Sleep -Milliseconds 250
    if ($fixtureProcess.HasExited) {
        throw "The locked executable fixture exited before the installer test."
    }

    $install = Start-Process -FilePath $installerPath -ArgumentList "/S" -WindowStyle Hidden -Wait -PassThru
    if ($install.ExitCode -ne 0) {
        throw "The replacement fixture failed with exit code $($install.ExitCode)."
    }

    $fixtureProcess.Refresh()
    if (-not $fixtureProcess.HasExited) {
        throw "The installer did not stop the old executable."
    }

    $expectedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $payloadExe).Hash
    $installedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedExe).Hash
    if ($installedHash -ne $expectedHash) {
        throw "The installer did not replace the old executable with its payload."
    }

    $installedVersionInfo = [Diagnostics.FileVersionInfo]::GetVersionInfo($installedExe)
    $installedVersion = "{0}.{1}.{2}.{3}" -f $installedVersionInfo.FileMajorPart,
        $installedVersionInfo.FileMinorPart,
        $installedVersionInfo.FileBuildPart,
        $installedVersionInfo.FilePrivatePart
    if ($installedVersion -ne $expectedVersion) {
        throw "Installed fixture version $installedVersion does not match $expectedVersion."
    }

    $mismatchNsi = $nsi.Replace(
        "!define VERSIONWITHBUILD `"$expectedVersion`"",
        "!define VERSIONWITHBUILD `"99.99.99.99`""
    ).Replace(
        "OutFile `"$(& $escaped $installerPath)`"",
        "OutFile `"$(& $escaped $mismatchInstallerPath)`""
    )
    Set-Content -LiteralPath $mismatchScriptPath -Value $mismatchNsi -Encoding utf8
    & $makeNsis "/INPUTCHARSET" "UTF8" "/WX" "/V2" "/XSetCompressor /FINAL lzma" $mismatchScriptPath
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $mismatchInstallerPath)) {
        throw "makensis failed to build the version-mismatch fixture."
    }
    $mismatchInstall = Start-Process -FilePath $mismatchInstallerPath -ArgumentList "/S" -WindowStyle Hidden -Wait -PassThru
    if ($mismatchInstall.ExitCode -eq 0) {
        throw "The installer reported success after installing an unexpected executable version."
    }

    Copy-Item -LiteralPath $oldExe -Destination $installedExe -Force
    $oldHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedExe).Hash
    $lockedFile = [IO.File]::Open($installedExe, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $blockedInstall = Start-Process -FilePath $installerPath -ArgumentList "/S" -WindowStyle Hidden -Wait -PassThru
    if ($blockedInstall.ExitCode -eq 0) {
        throw "The installer reported success while the target executable was locked."
    }
    $lockedFile.Dispose()
    $lockedFile = $null

    $blockedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedExe).Hash
    if ($blockedHash -ne $oldHash) {
        throw "The fail-closed fixture unexpectedly changed the locked executable."
    }

    Write-Output "[OK] NSIS upgrade replaces the target payload and fails closed on lock or version mismatch."
}
finally {
    if ($lockedFile) {
        $lockedFile.Dispose()
    }
    if ($fixtureProcess -and -not $fixtureProcess.HasExited) {
        Stop-Process -Id $fixtureProcess.Id -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path -LiteralPath $fixtureRoot) {
        $resolvedFixture = [IO.Path]::GetFullPath($fixtureRoot)
        $resolvedTemp = [IO.Path]::GetFullPath($env:TEMP).TrimEnd("\") + "\"
        if (-not $resolvedFixture.StartsWith($resolvedTemp, [StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove fixture outside the temp directory: $resolvedFixture"
        }
        Remove-Item -LiteralPath $resolvedFixture -Recurse -Force
    }
}
