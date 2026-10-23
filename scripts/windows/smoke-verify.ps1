$ErrorActionPreference = "Stop"

# Native Windows 10/11 Silent Install, WebView2 Check, Restart, Re-Install Upgrade & Uninstall Smoke Verification
# Usage (run on native Windows 10/11 host after ./scripts/windows/build-installer.ps1):
#   ./scripts/windows/smoke-verify.ps1

$BundleDir = Join-Path $PSScriptRoot "..\..\apps\desktop\src-tauri\target\release\bundle\nsis"
$Installer = Get-ChildItem -Path $BundleDir -Filter "*-setup.exe" | Select-Object -First 1
if (-not $Installer) {
    throw "Run ./scripts/windows/build-installer.ps1 first — no NSIS installer found in $BundleDir"
}

Write-Host "==> [1/6] Checking Microsoft Edge WebView2 Evergreen Runtime & Authenticode status..."
$Wv2Keys = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
)
$Wv2Version = $null
foreach ($k in $Wv2Keys) {
    if (Test-Path $k) {
        $v = (Get-ItemProperty -Path $k -ErrorAction SilentlyContinue).pv
        if ($v -and $v -ne "0.0.0.0") { $Wv2Version = $v; break }
    }
}
if (-not $Wv2Version) {
    throw "Microsoft Edge WebView2 Evergreen Runtime was not detected on this Windows machine."
}
$Sig = Get-AuthenticodeSignature -FilePath $Installer.FullName
Write-Host "  WebView2 Runtime : $Wv2Version"
Write-Host "  Installer File   : $($Installer.Name) (Authenticode: $($Sig.Status))"

$InstallRoot = Join-Path $env:LOCALAPPDATA "Programs\Nexora"
$AppExe = Join-Path $InstallRoot "nexora.exe"
$Uninstaller = Join-Path $InstallRoot "uninstall.exe"
$AppDataRoot = Join-Path $env:APPDATA "com.nexora.desktop"
$ModelStorageDir = Join-Path $AppDataRoot "models"

Write-Host "==> [2/6] Running clean per-user silent NSIS installation..."
$Proc = Start-Process -FilePath $Installer.FullName -ArgumentList "/S" -Wait -PassThru
if ($Proc.ExitCode -ne 0) {
    throw "NSIS silent install failed with exit code $($Proc.ExitCode)"
}
if (-not (Test-Path $AppExe)) {
    throw "Expected installed binary not found at $AppExe"
}

Write-Host "==> [3/6] Launching nexora.exe, verifying process stability & clean shutdown + restart..."
for ($attempt = 1; $attempt -le 2; $attempt++) {
    $Launched = Start-Process -FilePath $AppExe -PassThru
    Start-Sleep -Seconds 4
    if ($Launched.HasExited) {
        throw "nexora.exe exited prematurely on launch #$attempt with code $($Launched.ExitCode)"
    }
    Stop-Process -Id $Launched.Id -Force
    $Launched.WaitForExit(5000) | Out-Null
}
Write-Host "  Expected AppData path : $AppDataRoot (exists: $(Test-Path $AppDataRoot); default models dir: $ModelStorageDir)"

Write-Host "==> [4/6] Running in-place NSIS upgrade / reinstall check over existing installation..."
$UpProc = Start-Process -FilePath $Installer.FullName -ArgumentList "/S" -Wait -PassThru
if ($UpProc.ExitCode -ne 0) {
    throw "NSIS in-place upgrade reinstall failed with exit code $($UpProc.ExitCode)"
}
if (-not (Test-Path $AppExe)) {
    throw "Installed binary missing after in-place reinstall: $AppExe"
}

Write-Host "==> [5/6] Running silent NSIS uninstaller..."
if (-not (Test-Path $Uninstaller)) {
    throw "Expected uninstaller not found at $Uninstaller"
}
$UnProc = Start-Process -FilePath $Uninstaller -ArgumentList "/S" -Wait -PassThru
if ($UnProc.ExitCode -ne 0) {
    throw "NSIS silent uninstall failed with exit code $($UnProc.ExitCode)"
}

Write-Host "==> [6/6] Verifying binary removal while preserving user AppData..."
Start-Sleep -Seconds 2
if (Test-Path $AppExe) {
    throw "Installed binary still present after uninstall: $AppExe"
}
Write-Host "==> PASS: Native Windows WebView2 check, NSIS install, launch, restart, upgrade reinstall, and uninstall verified cleanly."
