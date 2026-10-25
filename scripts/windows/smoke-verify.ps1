# Native Windows 10/11 x64 NSIS install, launch, reinstall, uninstall, and data-preservation smoke test.
# Usage from the repository root after build-installer.ps1:
#   ./scripts/windows/smoke-verify.ps1
# Optional test-only install destination (must be a new path under LOCALAPPDATA):
#   $testPath = Join-Path $env:LOCALAPPDATA ("Nexora Test " + [char]0x03A9)
#   ./scripts/windows/smoke-verify.ps1 -TestInstallRoot $testPath
param(
    [string]$TestInstallRoot
)

$ErrorActionPreference = "Stop"
$TargetTriple = "x86_64-pc-windows-msvc"
$UnicodeMarker = [string][char]0x03A9
$BundleDir = Join-Path $PSScriptRoot "..\..\apps\desktop\src-tauri\target\$TargetTriple\release\bundle\nsis"
$ManifestPath = Join-Path $BundleDir "SHA256SUMS.txt"
$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$TauriConfigPath = Join-Path $RepositoryRoot "apps\desktop\src-tauri\tauri.conf.json"

function Get-WebView2EvergreenVersion {
    $Wv2Keys = @(
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    )
    foreach ($key in $Wv2Keys) {
        if (Test-Path -LiteralPath $key) {
            $candidate = (Get-ItemProperty -LiteralPath $key -ErrorAction SilentlyContinue).pv
            if ($candidate -and $candidate -ne "0.0.0.0") { return $candidate }
        }
    }

    $Wv2Dirs = @(
        "${env:ProgramFiles(x86)}\Microsoft\EdgeWebView\Application",
        "$env:ProgramFiles\Microsoft\EdgeWebView\Application",
        "$env:LOCALAPPDATA\Microsoft\EdgeWebView\Application"
    )
    foreach ($dir in $Wv2Dirs) {
        if (Test-Path -LiteralPath $dir) {
            $versionDir = Get-ChildItem -LiteralPath $dir -Directory -ErrorAction SilentlyContinue |
                Where-Object { $_.Name -match '^\d+\.' } |
                Sort-Object Name -Descending |
                Select-Object -First 1
            if ($versionDir) { return $versionDir.Name }
        }
    }
    return $null
}

function Get-NexoraProcess {
    param([Parameter(Mandatory = $true)][string]$ExecutablePath)
    $expectedPath = [System.IO.Path]::GetFullPath($ExecutablePath)
    foreach ($process in @(Get-Process -Name "nexora" -ErrorAction SilentlyContinue)) {
        try {
            $processPath = $process.MainModule.FileName
            if ($processPath -and [string]::Equals(
                [System.IO.Path]::GetFullPath($processPath),
                $expectedPath,
                [System.StringComparison]::OrdinalIgnoreCase
            )) {
                return $process
            }
        } catch {
            # An inaccessible process is not treated as the smoke-test app.
        }
    }
    return $null
}

function Wait-ForNexoraWindow {
    param([Parameter(Mandatory = $true)][string]$ExecutablePath)
    $deadline = (Get-Date).AddSeconds(30)
    do {
        $process = Get-NexoraProcess -ExecutablePath $ExecutablePath
        if ($process) {
            if ($script:LaunchedProcessIds -notcontains $process.Id) {
                $script:LaunchedProcessIds += $process.Id
            }
            if ($process.MainWindowHandle -ne 0) { return $process }
        }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $deadline)
    return $null
}

function Close-NexoraWindow {
    param([Parameter(Mandatory = $true)]$Process)
    if (-not $Process.CloseMainWindow()) {
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
        throw "The installed Nexora window did not accept a normal close request."
    }
    if (-not $Process.WaitForExit(15000)) {
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
        throw "The installed Nexora process did not exit after a normal close request."
    }
    if ($Process.ExitCode -ne 0) {
        throw "The installed Nexora process exited with code $($Process.ExitCode) after a normal close."
    }
}

function Resolve-NexoraShortcut {
    param(
        [Parameter(Mandatory = $true)][string]$ShortcutRoot,
        [Parameter(Mandatory = $true)][string]$ExpectedExecutable
    )
    if (-not (Test-Path -LiteralPath $ShortcutRoot)) { return $null }
    $shell = New-Object -ComObject WScript.Shell
    $expected = [System.IO.Path]::GetFullPath($ExpectedExecutable)
    foreach ($candidate in @(Get-ChildItem -LiteralPath $ShortcutRoot -Filter "Nexora.lnk" -File -Recurse -ErrorAction SilentlyContinue)) {
        try {
            $shortcut = $shell.CreateShortcut($candidate.FullName)
            if ($shortcut.TargetPath -and [string]::Equals(
                [System.IO.Path]::GetFullPath($shortcut.TargetPath),
                $expected,
                [System.StringComparison]::OrdinalIgnoreCase
            )) {
                return $candidate.FullName
            }
        } catch {
            # Ignore malformed/unrelated links; require one valid target below.
        }
    }
    return $null
}

function Get-PeMachine {
    param([Parameter(Mandatory = $true)][string]$Path)
    $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::Read)
    $reader = New-Object System.IO.BinaryReader($stream)
    try {
        if ($stream.Length -lt 64) { throw "The executable is too small to contain a PE header." }
        $stream.Position = 0x3c
        $peOffset = $reader.ReadInt32()
        if ($peOffset -lt 64 -or $peOffset -gt ($stream.Length - 6)) { throw "The executable has an invalid PE-header offset." }
        $stream.Position = $peOffset
        if ([System.Text.Encoding]::ASCII.GetString($reader.ReadBytes(4)) -ne "PE`0`0") {
            throw "The executable is missing the PE signature."
        }
        return $reader.ReadUInt16()
    } finally {
        $reader.Dispose()
        $stream.Dispose()
    }
}

function Test-PathEqualOrWithin {
    param(
        [Parameter(Mandatory = $true)][string]$Candidate,
        [Parameter(Mandatory = $true)][string]$Parent
    )
    $candidateFull = [System.IO.Path]::GetFullPath($Candidate).TrimEnd([char[]]@([char]0x5c, [char]0x2f))
    $parentFull = [System.IO.Path]::GetFullPath($Parent).TrimEnd([char[]]@([char]0x5c, [char]0x2f))
    if ([string]::Equals($candidateFull, $parentFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $true
    }
    $parentPrefix = if ($parentFull.Length -eq 0) {
        [System.IO.Path]::DirectorySeparatorChar.ToString()
    } else {
        $parentFull + [System.IO.Path]::DirectorySeparatorChar
    }
    return $candidateFull.StartsWith($parentPrefix, [System.StringComparison]::OrdinalIgnoreCase)
}

if ($env:OS -ne "Windows_NT" -or -not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
    throw "This smoke test requires native x64 Windows running 64-bit PowerShell; Linux packaging is not Windows acceptance."
}
$OperatingSystem = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction Stop
Write-Host "Windows host: $($OperatingSystem.Caption), version $($OperatingSystem.Version), build $($OperatingSystem.BuildNumber), product type $($OperatingSystem.ProductType)."
if ($OperatingSystem.ProductType -ne 1) {
    Write-Warning "This is not a Windows client OS (ProductType 1); this package smoke result is not Windows 10/11 user acceptance."
}
foreach ($requiredVariable in @("LOCALAPPDATA", "APPDATA", "TEMP")) {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($requiredVariable))) {
        throw "The current Windows profile is missing the required $requiredVariable path."
    }
}
if (-not (Test-Path -LiteralPath $TauriConfigPath)) {
    throw "The repository Tauri configuration is missing: $TauriConfigPath"
}
$ExpectedVersion = (Get-Content -LiteralPath $TauriConfigPath -Raw | ConvertFrom-Json).version
if ([string]::IsNullOrWhiteSpace($ExpectedVersion)) {
    throw "The Tauri configuration does not declare an application version."
}

$BundleExecutables = @(Get-ChildItem -LiteralPath $BundleDir -Filter "*.exe" -File -Force -ErrorAction SilentlyContinue)
$InstallerCandidates = @($BundleExecutables | Where-Object { $_.Name -like "*-setup.exe" })
if ($BundleExecutables.Count -ne 1 -or $InstallerCandidates.Count -ne 1) {
    throw "Expected exactly one x64 NSIS setup executable and no other EXE files in $BundleDir; found $($BundleExecutables.Count) EXE(s). Run build-installer.ps1 first."
}
$Installer = $InstallerCandidates[0]
if ($Installer.Length -le 0) { throw "The NSIS setup executable is empty: $($Installer.FullName)" }
if (-not (Test-Path -LiteralPath $ManifestPath)) {
    throw "The build SHA-256 manifest is missing: $ManifestPath"
}
$ManifestFile = Get-Item -LiteralPath $ManifestPath -ErrorAction Stop
if ($ManifestFile.LastWriteTimeUtc -lt $Installer.LastWriteTimeUtc) {
    throw "The SHA-256 manifest predates the installer and may be stale. Run build-installer.ps1 against an empty output directory."
}
$InstallerHash = (Get-FileHash -LiteralPath $Installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
$ManifestMatches = @()
foreach ($line in Get-Content -LiteralPath $ManifestPath) {
    $cleanLine = $line.TrimStart([char]0xFEFF)
    if ($cleanLine -match '^(?<hash>[0-9a-fA-F]{64})\s{2}(?<name>.+)$' -and
        [string]::Equals($Matches.name.Trim(), $Installer.Name, [System.StringComparison]::OrdinalIgnoreCase)) {
        $ManifestMatches += $Matches.hash.ToLowerInvariant()
    }
}
if ($ManifestMatches.Count -ne 1 -or $ManifestMatches[0] -ne $InstallerHash) {
    throw "The installer SHA-256 does not match exactly one entry in SHA256SUMS.txt."
}
$Signature = Get-AuthenticodeSignature -FilePath $Installer.FullName
if ($Signature.Status -notin @("Valid", "NotSigned")) {
    throw "The installer has an invalid or untrusted Authenticode status: $($Signature.Status)."
}
Write-Host "==> Verified installer artifact: $($Installer.Name), $([Math]::Round($Installer.Length / 1MB, 2)) MiB, SHA-256 $InstallerHash, Authenticode $($Signature.Status)."

$DefaultInstallRoot = Join-Path $env:LOCALAPPDATA "Nexora"
$UninstallRegistryPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Nexora"
$StartMenuRoot = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$DesktopRoot = [Environment]::GetFolderPath([System.Environment+SpecialFolder]::DesktopDirectory)
$ExistingStartMenuShortcuts = @(Get-ChildItem -LiteralPath $StartMenuRoot -Filter "Nexora.lnk" -File -Recurse -ErrorAction SilentlyContinue)
$DesktopShortcutPath = Join-Path $DesktopRoot "Nexora.lnk"
if ((Test-Path -LiteralPath $DefaultInstallRoot) -or
    (Test-Path -LiteralPath $UninstallRegistryPath) -or
    $ExistingStartMenuShortcuts.Count -gt 0 -or
    (Test-Path -LiteralPath $DesktopShortcutPath)) {
    throw "Refusing to overwrite an existing Nexora installation or product shortcut. Run this smoke test with a clean Windows test profile."
}

if ([string]::IsNullOrWhiteSpace($TestInstallRoot)) {
    $TestInstallRoot = Join-Path $env:LOCALAPPDATA ("Nexora smoke " + $UnicodeMarker + " " + [Guid]::NewGuid().ToString("N"))
}
$InstallRoot = [System.IO.Path]::GetFullPath($TestInstallRoot)
$LocalAppDataPrefix = [System.IO.Path]::GetFullPath($env:LOCALAPPDATA).TrimEnd([char[]]@([char]0x5c, [char]0x2f)) + [System.IO.Path]::DirectorySeparatorChar
if (-not $InstallRoot.StartsWith($LocalAppDataPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "The test install destination must be a new user-writable path below LOCALAPPDATA."
}
if ($InstallRoot.Contains('"') -or (Test-Path -LiteralPath $InstallRoot)) {
    throw "The test install destination must be a new path without quote characters: $InstallRoot"
}

$AppDataRoot = Join-Path $env:APPDATA "com.nexora.desktop"
$LocalAppDataRoot = Join-Path $env:LOCALAPPDATA "com.nexora.desktop"
foreach ($ProtectedRoot in @($DefaultInstallRoot, $AppDataRoot, $LocalAppDataRoot)) {
    if ((Test-PathEqualOrWithin -Candidate $InstallRoot -Parent $ProtectedRoot) -or
        (Test-PathEqualOrWithin -Candidate $ProtectedRoot -Parent $InstallRoot)) {
        throw "The disposable test install path must not overlap Nexora's default install or user-data roots."
    }
}

$InstallRootInitiallyPresent = Test-Path -LiteralPath $InstallRoot
$ModelsRoot = Join-Path $AppDataRoot "models"
$HistoryFile = Join-Path $AppDataRoot "conversations.json"
$SettingsFile = Join-Path $AppDataRoot "inference-settings.json"
$DataMarker = Join-Path $AppDataRoot ".nexora-installer-smoke-preserve-marker"
$ModelMarker = Join-Path $ModelsRoot ".nexora-installer-smoke-model-storage-marker"
$LocalDataMarker = Join-Path $LocalAppDataRoot ".nexora-installer-smoke-preserve-marker"
$AppDataRootInitiallyPresent = Test-Path -LiteralPath $AppDataRoot
$ModelsRootInitiallyPresent = Test-Path -LiteralPath $ModelsRoot
$LocalAppDataRootInitiallyPresent = Test-Path -LiteralPath $LocalAppDataRoot
foreach ($directory in @($AppDataRoot, $LocalAppDataRoot)) {
    if (Test-Path -LiteralPath $directory) {
        $existing = @(Get-ChildItem -LiteralPath $directory -Force -ErrorAction Stop)
        if ($existing.Count -gt 0) {
            throw "Refusing to run the preservation test over non-empty user data at $directory. Use a clean Windows test profile."
        }
    }
}

$PackageTestRoot = Join-Path $env:TEMP ("Nexora installer path " + $UnicodeMarker + " " + [Guid]::NewGuid().ToString("N"))
if (Test-Path -LiteralPath $PackageTestRoot) {
    throw "The unique installer-copy test directory unexpectedly already exists; refusing to reuse it."
}
$InstallerCopy = Join-Path $PackageTestRoot $Installer.Name
$PackageTestRootCreated = $false
$InstallAttempted = $false
$UninstallCompleted = $false
$DataMarkerContent = "Nexora user-data smoke sentinel $([Guid]::NewGuid().ToString('N'))"
$LocalDataMarkerContent = "Nexora local-data smoke sentinel $([Guid]::NewGuid().ToString('N'))"
$HistoryContent = '{"version":1,"conversations":[]}'
$SettingsContent = '{"modelPath":null,"runtimePath":null,"contextSize":32768,"modelId":null}'
$ModelMarkerContent = "This is only a storage-preservation sentinel, not a model or verification record. $([Guid]::NewGuid().ToString('N'))"
$PreservationSentinels = @(
    @{ Path = $DataMarker; Expected = $DataMarkerContent },
    @{ Path = $LocalDataMarker; Expected = $LocalDataMarkerContent },
    @{ Path = $HistoryFile; Expected = $HistoryContent },
    @{ Path = $SettingsFile; Expected = $SettingsContent },
    @{ Path = $ModelMarker; Expected = $ModelMarkerContent }
)
$LaunchedProcessIds = @()
$Wv2VersionBefore = Get-WebView2EvergreenVersion

try {
    Write-Host "==> [1/8] Preparing Unicode/space paths and non-destructive data sentinels..."
    New-Item -ItemType Directory -Path $PackageTestRoot -ErrorAction Stop | Out-Null
    $PackageTestRootCreated = $true
    Copy-Item -LiteralPath $Installer.FullName -Destination $InstallerCopy -ErrorAction Stop
    $CopiedHash = (Get-FileHash -LiteralPath $InstallerCopy -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($CopiedHash -ne $InstallerHash) { throw "The Unicode/space-path installer copy failed its SHA-256 check." }

    if (-not $AppDataRootInitiallyPresent) { New-Item -ItemType Directory -Path $AppDataRoot -Force | Out-Null }
    if (-not $LocalAppDataRootInitiallyPresent) { New-Item -ItemType Directory -Path $LocalAppDataRoot -Force | Out-Null }
    New-Item -ItemType Directory -Path $ModelsRoot -Force | Out-Null
    [System.IO.File]::WriteAllText($DataMarker, $DataMarkerContent)
    [System.IO.File]::WriteAllText($LocalDataMarker, $LocalDataMarkerContent)
    [System.IO.File]::WriteAllText($HistoryFile, $HistoryContent)
    [System.IO.File]::WriteAllText($SettingsFile, $SettingsContent)
    [System.IO.File]::WriteAllText($ModelMarker, $ModelMarkerContent)

    Write-Host "==> [2/8] Running a fresh per-user NSIS install from the copied path..."
    Write-Host "  Test install path: $InstallRoot"
    Write-Host "  WebView2 before install: $(if ($Wv2VersionBefore) { $Wv2VersionBefore } else { 'not detected' })"
    # NSIS requires /D to be the final, unquoted argument. This unique test path
    # contains spaces and a non-ASCII character and remains below LOCALAPPDATA.
    $InstallAttempted = $true
    $InstallProcess = Start-Process -FilePath $InstallerCopy -ArgumentList "/S /D=$InstallRoot" -Wait -PassThru
    if ($InstallProcess.ExitCode -ne 0) { throw "NSIS silent install failed with exit code $($InstallProcess.ExitCode)." }

    $Wv2VersionAfter = Get-WebView2EvergreenVersion
    if (-not $Wv2VersionAfter) {
        throw "The installer exited successfully but WebView2 Evergreen is still missing; check the configured bootstrapper/network prerequisite."
    }
    $AppExe = Join-Path $InstallRoot "nexora.exe"
    if (-not (Test-Path -LiteralPath $AppExe)) { throw "Expected installed x64 application was not found: $AppExe" }
    $InstalledExe = Get-Item -LiteralPath $AppExe
    if ($InstalledExe.Length -lt 1MB) { throw "Installed application payload is unexpectedly small ($($InstalledExe.Length) bytes)." }
    $InstalledExeHash = (Get-FileHash -LiteralPath $AppExe -Algorithm SHA256).Hash.ToLowerInvariant()
    $PeMachine = Get-PeMachine -Path $AppExe
    if ($PeMachine -ne 0x8664) { throw "Expected an AMD64/x64 application PE (0x8664), found machine type 0x$('{0:X4}' -f $PeMachine)." }

    if (-not (Test-Path -LiteralPath $UninstallRegistryPath)) { throw "The per-user uninstall registration was not created: $UninstallRegistryPath" }
    $UninstallEntry = Get-ItemProperty -LiteralPath $UninstallRegistryPath -ErrorAction Stop
    if ($UninstallEntry.DisplayName -ne "Nexora" -or $UninstallEntry.DisplayVersion -ne $ExpectedVersion) {
        throw "Uninstall registration has the wrong product identity or version."
    }
    if (-not $UninstallEntry.UninstallString -or -not (Test-Path -LiteralPath $UninstallEntry.UninstallString.Trim('"'))) {
        throw "Uninstall registration does not point to an existing uninstaller."
    }
    $RegisteredInstallRoot = $UninstallEntry.InstallLocation.Trim('"')
    if (-not [string]::Equals(
        [System.IO.Path]::GetFullPath($RegisteredInstallRoot).TrimEnd([char]0x5c, [char]0x2f),
        $InstallRoot.TrimEnd([char]0x5c, [char]0x2f),
        [System.StringComparison]::OrdinalIgnoreCase
    )) { throw "The uninstall registration points to an unexpected install location." }

    $StartMenuShortcut = Resolve-NexoraShortcut -ShortcutRoot $StartMenuRoot -ExpectedExecutable $AppExe
    $DesktopShortcut = Resolve-NexoraShortcut -ShortcutRoot $DesktopRoot -ExpectedExecutable $AppExe
    if (-not $StartMenuShortcut) { throw "A working Nexora Start Menu shortcut was not created." }
    if (-not $DesktopShortcut) { throw "A working Nexora desktop shortcut was not created for the silent install." }
    $SessionId = (Get-Process -Id $PID).SessionId
    if ([Environment]::UserInteractive -and $SessionId -gt 0 -and -not $env:GITHUB_ACTIONS) {
        Write-Host "==> [3/8] Launching from outside the checkout and through the installed Start Menu shortcut..."
        $DirectLaunch = Start-Process -FilePath $AppExe -WorkingDirectory $PackageTestRoot -PassThru
        $LaunchedProcessIds += $DirectLaunch.Id
        $DirectWindow = Wait-ForNexoraWindow -ExecutablePath $AppExe
        if (-not $DirectWindow) { throw "The installed application did not create a visible window when launched outside the checkout." }
        Close-NexoraWindow -Process $DirectWindow

        Start-Process -FilePath $StartMenuShortcut | Out-Null
        $ShortcutWindow = Wait-ForNexoraWindow -ExecutablePath $AppExe
        if (-not $ShortcutWindow) { throw "The installed Start Menu shortcut did not launch the installed application window." }
        $LaunchedProcessIds += $ShortcutWindow.Id
        Close-NexoraWindow -Process $ShortcutWindow
        Write-Host "  Production window launched, closed cleanly, and relaunched via Start Menu."
    } else {
        Write-Host "==> [3/8] Headless/non-interactive session detected; PE, install registration, and shortcuts checked; GUI launch not claimed."
    }

    foreach ($preservedFile in $PreservationSentinels) {
        if ([System.IO.File]::ReadAllText($preservedFile.Path) -cne $preservedFile.Expected) {
            throw "Application startup changed a pre-existing user-data/model-storage preservation sentinel."
        }
    }

    Write-Host "==> [4/8] Running same-build reinstall over the existing per-user installation..."
    Write-Host "  No previous supported installer was supplied; this is a same-build reinstall check, not a version-upgrade test."
    $ReinstallProcess = Start-Process -FilePath $InstallerCopy -ArgumentList "/S /D=$InstallRoot" -Wait -PassThru
    if ($ReinstallProcess.ExitCode -ne 0) { throw "Same-build reinstall failed with exit code $($ReinstallProcess.ExitCode)." }
    if (-not (Test-Path -LiteralPath $AppExe)) { throw "The installed application was missing after same-build reinstall." }
    if ((Get-FileHash -LiteralPath $AppExe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $InstalledExeHash) {
        throw "Same-build reinstall changed the application executable bytes unexpectedly."
    }
    if (-not (Test-Path -LiteralPath $UninstallRegistryPath)) { throw "The uninstall registration was lost after same-build reinstall." }

    Write-Host "==> [5/8] Running the registered uninstaller silently..."
    $Uninstaller = Join-Path $InstallRoot "uninstall.exe"
    if (-not (Test-Path -LiteralPath $Uninstaller)) { throw "Expected uninstaller is missing: $Uninstaller" }
    $UninstallArguments = "/S `"_?=$InstallRoot`""
    $UninstallProcess = Start-Process -FilePath $Uninstaller -ArgumentList $UninstallArguments -Wait -PassThru
    if ($UninstallProcess.ExitCode -ne 0) { throw "NSIS silent uninstall failed with exit code $($UninstallProcess.ExitCode)." }
    $UninstallCompleted = $true

    Write-Host "==> [6/8] Verifying payload, registration, shortcuts, processes, and data-preservation policy..."
    $Deadline = (Get-Date).AddSeconds(30)
    while ((Test-Path -LiteralPath $AppExe) -and (Get-Date) -lt $Deadline) { Start-Sleep -Milliseconds 500 }
    if (Test-Path -LiteralPath $AppExe) { throw "Installed application remained after uninstall: $AppExe" }
    if (Test-Path -LiteralPath $UninstallRegistryPath) { throw "Uninstall registry registration remained after uninstall." }
    if (Resolve-NexoraShortcut -ShortcutRoot $StartMenuRoot -ExpectedExecutable $AppExe) { throw "Start Menu shortcut remained after uninstall." }
    if (Resolve-NexoraShortcut -ShortcutRoot $DesktopRoot -ExpectedExecutable $AppExe) { throw "Desktop shortcut remained after uninstall." }
    if (Get-NexoraProcess -ExecutablePath $AppExe) { throw "A Nexora process remained after uninstall." }

    if (Test-Path -LiteralPath $InstallRoot) {
        $RemainingContents = @(Get-ChildItem -LiteralPath $InstallRoot -Force -ErrorAction Stop)
        $UnexpectedContents = @($RemainingContents | Where-Object { $_.Name -ne "uninstall.exe" })
        if ($UnexpectedContents.Count -gt 0) { throw "The uninstaller left application files in the test install directory." }
        if (Test-Path -LiteralPath $Uninstaller) { Remove-Item -LiteralPath $Uninstaller -Force -ErrorAction Stop }
        $RemainingContents = @(Get-ChildItem -LiteralPath $InstallRoot -Force -ErrorAction Stop)
        if ($RemainingContents.Count -gt 0) { throw "Unexpected files remained in the test install directory after removing the NSIS self-delete stub." }
        Remove-Item -LiteralPath $InstallRoot -Force -ErrorAction Stop
    }

    foreach ($preservedFile in $PreservationSentinels) {
        if (-not (Test-Path -LiteralPath $preservedFile.Path) -or
            [System.IO.File]::ReadAllText($preservedFile.Path) -cne $preservedFile.Expected) {
            throw "Uninstall removed or changed a user-data/model-storage preservation sentinel: $($preservedFile.Path)"
        }
    }

    Write-Host "==> [7/8] Reinstalling after a successful uninstall..."
    $UninstallCompleted = $false
    $PostUninstallInstall = Start-Process -FilePath $InstallerCopy -ArgumentList "/S /D=$InstallRoot" -Wait -PassThru
    if ($PostUninstallInstall.ExitCode -ne 0) { throw "Reinstall after uninstall failed with exit code $($PostUninstallInstall.ExitCode)." }
    if (-not (Test-Path -LiteralPath $AppExe) -or -not (Test-Path -LiteralPath $UninstallRegistryPath)) {
        throw "Reinstall after uninstall did not restore the application and uninstall registration."
    }
    if ((Get-FileHash -LiteralPath $AppExe -Algorithm SHA256).Hash.ToLowerInvariant() -ne $InstalledExeHash) {
        throw "Reinstall after uninstall changed the application executable bytes unexpectedly."
    }
    $PostReinstallStartMenu = Resolve-NexoraShortcut -ShortcutRoot $StartMenuRoot -ExpectedExecutable $AppExe
    $PostReinstallDesktop = Resolve-NexoraShortcut -ShortcutRoot $DesktopRoot -ExpectedExecutable $AppExe
    if (-not $PostReinstallStartMenu -or -not $PostReinstallDesktop) {
        throw "Reinstall after uninstall did not recreate both product shortcuts."
    }
    $PostUninstallUninstaller = Join-Path $InstallRoot "uninstall.exe"
    $PostUninstallArguments = "/S `"_?=$InstallRoot`""
    $FinalUninstall = Start-Process -FilePath $PostUninstallUninstaller -ArgumentList $PostUninstallArguments -Wait -PassThru
    if ($FinalUninstall.ExitCode -ne 0) { throw "Final uninstall after reinstallation failed with exit code $($FinalUninstall.ExitCode)." }
    $UninstallCompleted = $true
    $FinalRemovalDeadline = (Get-Date).AddSeconds(30)
    while ((Test-Path -LiteralPath $AppExe) -and (Get-Date) -lt $FinalRemovalDeadline) { Start-Sleep -Milliseconds 500 }
    if (Test-Path -LiteralPath $AppExe) { throw "The application remained after the final uninstall." }
    if (Test-Path -LiteralPath $UninstallRegistryPath) { throw "Uninstall registration remained after the final uninstall." }
    if (Resolve-NexoraShortcut -ShortcutRoot $StartMenuRoot -ExpectedExecutable $AppExe) { throw "Start Menu shortcut remained after the final uninstall." }
    if (Resolve-NexoraShortcut -ShortcutRoot $DesktopRoot -ExpectedExecutable $AppExe) { throw "Desktop shortcut remained after the final uninstall." }
    if (Get-NexoraProcess -ExecutablePath $AppExe) { throw "A Nexora process remained after the final uninstall." }
    $Wv2VersionAfterUninstall = Get-WebView2EvergreenVersion
    if (-not $Wv2VersionAfterUninstall) { throw "WebView2 Evergreen did not remain available after Nexora uninstall." }
    if (Test-Path -LiteralPath $InstallRoot) {
        $FinalContents = @(Get-ChildItem -LiteralPath $InstallRoot -Force -ErrorAction Stop)
        $UnexpectedFinalContents = @($FinalContents | Where-Object { $_.Name -ne "uninstall.exe" })
        if ($UnexpectedFinalContents.Count -gt 0) { throw "The final uninstaller left application files in the test install directory." }
        if (Test-Path -LiteralPath $PostUninstallUninstaller) { Remove-Item -LiteralPath $PostUninstallUninstaller -Force -ErrorAction Stop }
        if (@(Get-ChildItem -LiteralPath $InstallRoot -Force -ErrorAction Stop).Count -gt 0) {
            throw "Unexpected files remained after removing the final NSIS self-delete stub."
        }
        Remove-Item -LiteralPath $InstallRoot -Force -ErrorAction Stop
    }

    foreach ($preservedFile in $PreservationSentinels) {
        if (-not (Test-Path -LiteralPath $preservedFile.Path) -or
            [System.IO.File]::ReadAllText($preservedFile.Path) -cne $preservedFile.Expected) {
            throw "Reinstall/uninstall cycle removed or changed a user-data/model-storage sentinel."
        }
    }

    Write-Host "==> [8/8] PASS: manifest hash, per-user NSIS install, x64 PE, WebView2, shortcuts, same-build reinstall, uninstall, reinstall-after-uninstall, process cleanup, and data sentinels verified."
    Write-Host "  WebView2 before/after install/uninstall: $(if ($Wv2VersionBefore) { $Wv2VersionBefore } else { 'not detected' }) / $Wv2VersionAfter / $Wv2VersionAfterUninstall"
    Write-Host "  This script does not prove a previous-version upgrade, offline network isolation, model inference, or code signing beyond the installer Authenticode status printed above."
} finally {
    foreach ($processId in $LaunchedProcessIds) {
        $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
        if ($process) { Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue }
    }

    if ($InstallAttempted -and -not $UninstallCompleted) {
        $Uninstaller = Join-Path $InstallRoot "uninstall.exe"
        if (Test-Path -LiteralPath $Uninstaller) {
            try {
                $CleanupArgs = "/S `"_?=$InstallRoot`""
                $CleanupProcess = Start-Process -FilePath $Uninstaller -ArgumentList $CleanupArgs -Wait -PassThru
                if ($CleanupProcess.ExitCode -ne 0) { Write-Warning "Best-effort cleanup uninstaller returned $($CleanupProcess.ExitCode)." }
            } catch {
                Write-Warning "Best-effort installer cleanup failed; no user data was recursively removed."
            }
        }
        if (-not $InstallRootInitiallyPresent -and (Test-Path -LiteralPath $InstallRoot)) {
            try {
                $leftovers = @(Get-ChildItem -LiteralPath $InstallRoot -Force -ErrorAction Stop)
                if ($leftovers.Count -eq 1 -and $leftovers[0].Name -eq "uninstall.exe") {
                    Remove-Item -LiteralPath $Uninstaller -Force -ErrorAction Stop
                    $leftovers = @(Get-ChildItem -LiteralPath $InstallRoot -Force -ErrorAction Stop)
                }
                if ($leftovers.Count -eq 0) { Remove-Item -LiteralPath $InstallRoot -Force -ErrorAction SilentlyContinue }
                elseif ($leftovers.Count -gt 0) { Write-Warning "Test install files remain at $InstallRoot for manual recovery; the script did not recursively delete them." }
            } catch {
                Write-Warning "Could not complete best-effort cleanup of the test-only installation directory $InstallRoot."
            }
        }
    }

    foreach ($sentinel in $PreservationSentinels) {
        if (-not (Test-Path -LiteralPath $sentinel.Path)) { continue }
        try {
            $sentinelItem = Get-Item -LiteralPath $sentinel.Path -Force -ErrorAction Stop
            if ($sentinelItem.PSIsContainer) {
                Write-Warning "Leaving unexpected directory at test sentinel path $($sentinel.Path)."
                continue
            }
            $currentContent = [System.IO.File]::ReadAllText($sentinel.Path)
            if (-not [string]::Equals($currentContent, $sentinel.Expected, [System.StringComparison]::Ordinal)) {
                Write-Warning "Leaving changed test sentinel untouched for manual review: $($sentinel.Path)"
                continue
            }
            Remove-Item -LiteralPath $sentinel.Path -Force -ErrorAction Stop
        } catch {
            Write-Warning "Could not safely remove test sentinel $($sentinel.Path); it was left in place."
        }
    }
    foreach ($directory in @($ModelsRoot, $AppDataRoot, $LocalAppDataRoot)) {
        if (-not (Test-Path -LiteralPath $directory)) { continue }
        if ($directory -eq $ModelsRoot) {
            $wasPresent = $ModelsRootInitiallyPresent
        } elseif ($directory -eq $LocalAppDataRoot) {
            $wasPresent = $LocalAppDataRootInitiallyPresent
        } else {
            $wasPresent = $AppDataRootInitiallyPresent
        }
        $remaining = @(Get-ChildItem -LiteralPath $directory -Force -ErrorAction SilentlyContinue)
        if (-not $wasPresent -and $remaining.Count -eq 0) { Remove-Item -LiteralPath $directory -Force -ErrorAction SilentlyContinue }
    }
    if ($PackageTestRootCreated -and (Test-Path -LiteralPath $PackageTestRoot)) {
        try {
            if (Test-Path -LiteralPath $InstallerCopy) {
                $copyItem = Get-Item -LiteralPath $InstallerCopy -Force -ErrorAction Stop
                if ($copyItem.PSIsContainer) {
                    Write-Warning "Unexpected directory at installer-copy path; leaving it untouched: $InstallerCopy"
                } else {
                    $copyHash = (Get-FileHash -LiteralPath $InstallerCopy -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
                    if ($copyHash -eq $InstallerHash) {
                        Remove-Item -LiteralPath $InstallerCopy -Force -ErrorAction Stop
                    } else {
                        Write-Warning "Installer-copy bytes changed; preserving the file for manual review: $InstallerCopy"
                    }
                }
            }
            $remainingPackageFiles = @(Get-ChildItem -LiteralPath $PackageTestRoot -Force -ErrorAction Stop)
            if ($remainingPackageFiles.Count -eq 0) {
                Remove-Item -LiteralPath $PackageTestRoot -Force -ErrorAction Stop
            } else {
                Write-Warning "Leaving unexpected installer-test files in place for manual review: $PackageTestRoot"
            }
        } catch {
            Write-Warning "Could not safely clean the test-only installer-copy path $PackageTestRoot; remaining files were preserved."
        }
    }
}
