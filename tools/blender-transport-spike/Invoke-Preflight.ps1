[CmdletBinding()]
param(
    [string]$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..")),
    [string]$BlenderPath,
    [string]$OutputPath,
    [ValidateRange(5, 60)]
    [int]$ProcessTimeoutSeconds = 20
)

$ErrorActionPreference = "Stop"

if ($env:OS -ne "Windows_NT") {
    Write-Error "BLOCKED - this preflight requires Windows PowerShell on a real Windows host. No Windows, Rust, Blender, pipe, or handshake test was run." -ErrorAction Continue
    exit 2
}

function Get-ApplicationCommand {
    param([Parameter(Mandatory = $true)][string]$Name)
    $command = Get-Command -Name $Name -CommandType Application -ErrorAction SilentlyContinue |
        Select-Object -First 1
    return $command
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][int]$TimeoutSeconds
    )

    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("nexora-blender-preflight-" + [Guid]::NewGuid().ToString("N"))
    $stdoutPath = Join-Path $tempRoot "stdout.txt"
    $stderrPath = Join-Path $tempRoot "stderr.txt"
    $process = $null
    $timedOut = $false
    $exitCode = $null
    $startError = $false

    try {
        [void](New-Item -ItemType Directory -Path $tempRoot -ErrorAction Stop)
        # All callers pass fixed, whitespace-free arguments; no user-supplied text is executed.
        $argumentString = $Arguments -join " "
        $process = Start-Process -FilePath $FilePath -ArgumentList $argumentString -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -ErrorAction Stop
        if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
            $timedOut = $true
            try { $process.Kill() } catch { }
            try { [void]$process.WaitForExit(5000) } catch { }
        } else {
            $process.Refresh()
            $exitCode = $process.ExitCode
        }

        $stdout = if (Test-Path -LiteralPath $stdoutPath) { [System.IO.File]::ReadAllText($stdoutPath) } else { "" }
        $stderr = if (Test-Path -LiteralPath $stderrPath) { [System.IO.File]::ReadAllText($stderrPath) } else { "" }
        return [pscustomobject]@{
            TimedOut = $timedOut
            ExitCode = $exitCode
            StdOut = $stdout
            StdErr = $stderr
            StartError = $startError
        }
    } catch {
        # Do not copy raw exception text into the report: it may contain a local path.
        $startError = $true
        return [pscustomobject]@{
            TimedOut = $false
            ExitCode = $null
            StdOut = ""
            StdErr = ""
            StartError = $startError
        }
    } finally {
        if ($process -and -not $process.HasExited) {
            try { $process.Kill() } catch { }
        }
        if (Test-Path -LiteralPath $tempRoot) {
            Remove-Item -LiteralPath $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

function Get-ToolObservation {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [string[]]$VersionArguments = @("--version"),
        [bool]$RunVersion = $true
    )

    $command = Get-ApplicationCommand -Name $Name
    if (-not $command) {
        return [pscustomobject]@{ Available = $false; Version = $null; VersionQuery = "NOT_AVAILABLE" }
    }
    if (-not $RunVersion) {
        return [pscustomobject]@{ Available = $true; Version = $null; VersionQuery = "NOT_QUERIED" }
    }

    $result = Invoke-CapturedProcess -FilePath $command.Source -Arguments $VersionArguments -TimeoutSeconds 10
    if ($result.TimedOut) {
        return [pscustomobject]@{ Available = $true; Version = $null; VersionQuery = "TIMEOUT" }
    }
    if ($result.StartError -or $result.ExitCode -ne 0) {
        return [pscustomobject]@{ Available = $true; Version = $null; VersionQuery = "FAILED" }
    }
    $line = ($result.StdOut -split "`r?`n" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -First 1)
    return [pscustomobject]@{ Available = $true; Version = $line; VersionQuery = "OK" }
}

$resolvedRoot = [System.IO.Path]::GetFullPath($RepositoryRoot)
$requiredRelativeFiles = @(
    "tools/blender-transport-spike/README.md",
    "tools/blender-transport-spike/Invoke-Preflight.ps1",
    "tools/blender-transport-spike/TEST_PLAN.md",
    "tools/blender-transport-spike/result-template.json",
    "tools/blender-transport-spike/check_static.py",
    "docs/nexora/BLENDER_INTEGRATION_PHASE_5_IMPLEMENTATION.md",
    "docs/nexora/BLENDER_INTEGRATION_PHASE_5A_TRANSPORT_SPIKE.md",
    "docs/nexora/APP_CONNECTOR_CONTRACT.md",
    "apps/desktop/src-tauri/Cargo.toml",
    "apps/desktop/src-tauri/src/integrations/commands.rs",
    "apps/desktop/src-tauri/src/integrations/detector.rs"
)
$fileChecks = @()
foreach ($relativePath in $requiredRelativeFiles) {
    $fileChecks += [pscustomobject]@{
        File = $relativePath
        Present = Test-Path -LiteralPath (Join-Path $resolvedRoot ($relativePath -replace '/', '\')) -PathType Leaf
    }
}
$repositoryFilesPresent = @($fileChecks | Where-Object { -not $_.Present }).Count -eq 0

$rustc = Get-ToolObservation -Name "rustc"
$cargo = Get-ToolObservation -Name "cargo"
$rustup = Get-ToolObservation -Name "rustup"
$git = Get-ToolObservation -Name "git"
$node = Get-ToolObservation -Name "node"
$pnpm = Get-ToolObservation -Name "pnpm"
$msvcCl = Get-ToolObservation -Name "cl.exe" -RunVersion $false
$msvcLink = Get-ToolObservation -Name "link.exe" -RunVersion $false
$windowsSdkRc = Get-ToolObservation -Name "rc.exe" -RunVersion $false

$rustHost = $null
$rustTargetObservation = "NOT_CHECKED"
$rustcCommand = Get-ApplicationCommand -Name "rustc"
$rustupCommand = Get-ApplicationCommand -Name "rustup"
if ($rustc.Available -and $rustcCommand) {
    $hostResult = Invoke-CapturedProcess -FilePath $rustcCommand.Source -Arguments @("-vV") -TimeoutSeconds 10
    if (-not $hostResult.TimedOut -and -not $hostResult.StartError -and $hostResult.ExitCode -eq 0) {
        $hostLine = $hostResult.StdOut -split "`r?`n" | Where-Object { $_ -match '^host:\s*' } | Select-Object -First 1
        if ($hostLine) { $rustHost = ($hostLine -replace '^host:\s*', '').Trim() }
    }
}
if ($rustup.Available -and $rustupCommand) {
    $targets = Invoke-CapturedProcess -FilePath $rustupCommand.Source -Arguments @("target", "list", "--installed") -TimeoutSeconds 10
    if (-not $targets.TimedOut -and -not $targets.StartError -and $targets.ExitCode -eq 0) {
        $rustTargetObservation = if (($targets.StdOut -split "`r?`n") -contains "x86_64-pc-windows-msvc") { "PRESENT" } else { "MISSING" }
    } else {
        $rustTargetObservation = "QUERY_FAILED"
    }
}

$blenderCommand = $null
$blenderSource = "NONE"
if (-not [string]::IsNullOrWhiteSpace($BlenderPath)) {
    $candidate = [Environment]::ExpandEnvironmentVariables($BlenderPath)
    if (Test-Path -LiteralPath $candidate -PathType Leaf) {
        $blenderCommand = [pscustomobject]@{ Source = [System.IO.Path]::GetFullPath($candidate) }
        $blenderSource = "EXPLICIT_PATH"
    } else {
        $blenderSource = "EXPLICIT_PATH_NOT_FOUND"
    }
} else {
    $foundBlender = Get-ApplicationCommand -Name "blender.exe"
    if (-not $foundBlender) { $foundBlender = Get-ApplicationCommand -Name "blender" }
    if ($foundBlender) {
        $blenderCommand = $foundBlender
        $blenderSource = "PATH"
    }
}

$blenderVersion = $null
$blenderVersionStatus = "NOT_AVAILABLE"
$embeddedPythonVersion = $null
$embeddedPythonStatus = "NOT_PROBED"
if ($blenderCommand) {
    $configTempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("nexora-blender-probe-" + [Guid]::NewGuid().ToString("N"))
    $temporaryBlenderEnvironment = @("BLENDER_USER_CONFIG", "BLENDER_USER_SCRIPTS", "BLENDER_USER_DATAFILES")
    $previousEnvironment = @{}
    try {
        [void](New-Item -ItemType Directory -Path $configTempRoot -ErrorAction Stop)
        foreach ($name in $temporaryBlenderEnvironment) {
            $previousEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, [EnvironmentVariableTarget]::Process)
            $isolatedPath = Join-Path $configTempRoot $name
            [void](New-Item -ItemType Directory -Path $isolatedPath -Force -ErrorAction Stop)
            [Environment]::SetEnvironmentVariable($name, $isolatedPath, [EnvironmentVariableTarget]::Process)
        }

        $versionResult = Invoke-CapturedProcess -FilePath $blenderCommand.Source -Arguments @("--version") -TimeoutSeconds $ProcessTimeoutSeconds
        if ($versionResult.TimedOut) {
            $blenderVersionStatus = "TIMEOUT"
        } elseif ($versionResult.StartError -or $versionResult.ExitCode -ne 0) {
            $blenderVersionStatus = "FAILED"
        } else {
            $versionMatch = [regex]::Match($versionResult.StdOut, '(?m)^Blender\s+([^\s]+)')
            if ($versionMatch.Success) {
                $blenderVersion = $versionMatch.Groups[1].Value
                $blenderVersionStatus = "OK"
            } else {
                $blenderVersionStatus = "UNRECOGNIZED_OUTPUT"
            }
        }

        if ($blenderVersionStatus -eq "OK") {
            # Fixed, read-only version probe. No project file, user startup script,
            # scene operation, add-on install, or arbitrary caller-supplied Python.
            $fixedExpression = "import sys,bpy;print('NEXORA_SPIKE_PY='+'.'.join(map(str,sys.version_info[:3])));print('NEXORA_SPIKE_BLENDER='+bpy.app.version_string)"
            $pythonResult = Invoke-CapturedProcess -FilePath $blenderCommand.Source -Arguments @(
                "--background", "--factory-startup", "--disable-autoexec", "--python-expr", $fixedExpression
            ) -TimeoutSeconds $ProcessTimeoutSeconds
            if ($pythonResult.TimedOut) {
                $embeddedPythonStatus = "TIMEOUT"
            } elseif ($pythonResult.StartError -or $pythonResult.ExitCode -ne 0) {
                $embeddedPythonStatus = "FAILED"
            } else {
                $pythonMatch = [regex]::Match($pythonResult.StdOut, '(?m)^NEXORA_SPIKE_PY=([^\r\n]+)')
                if ($pythonMatch.Success) {
                    $embeddedPythonVersion = $pythonMatch.Groups[1].Value.Trim()
                    $embeddedPythonStatus = "OK"
                } else {
                    $embeddedPythonStatus = "VERSION_NOT_REPORTED"
                }
            }
        } else {
            $embeddedPythonStatus = "BLOCKED_BLENDER_VERSION_PROBE"
        }
    } catch {
        # Keep local paths and raw exception text out of the structured report.
        if ($blenderVersionStatus -eq "NOT_AVAILABLE") { $blenderVersionStatus = "PROBE_ERROR" }
        if ($embeddedPythonStatus -eq "NOT_PROBED") { $embeddedPythonStatus = "PROBE_ERROR" }
    } finally {
        foreach ($name in $temporaryBlenderEnvironment) {
            if ($previousEnvironment.ContainsKey($name)) {
                [Environment]::SetEnvironmentVariable($name, $previousEnvironment[$name], [EnvironmentVariableTarget]::Process)
            }
        }
        if (Test-Path -LiteralPath $configTempRoot) {
            Remove-Item -LiteralPath $configTempRoot -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

$osInfo = $null
try {
    $os = Get-CimInstance -ClassName Win32_OperatingSystem -ErrorAction Stop
    $osInfo = [pscustomobject]@{
        ProductName = ($os.Caption -replace '^\s*', '').Trim()
        Version = $os.Version
        Build = $os.BuildNumber
        Architecture = $os.OSArchitecture
        Is64BitOperatingSystem = [Environment]::Is64BitOperatingSystem
        PowerShellProcessIs64Bit = [Environment]::Is64BitProcess
    }
} catch {
    $osInfo = [pscustomobject]@{
        ProductName = "UNKNOWN"
        Version = [Environment]::OSVersion.Version.ToString()
        Build = [Environment]::OSVersion.Version.Build.ToString()
        Architecture = $env:PROCESSOR_ARCHITECTURE
        Is64BitOperatingSystem = [Environment]::Is64BitOperatingSystem
        PowerShellProcessIs64Bit = [Environment]::Is64BitProcess
    }
}

$missing = @()
if (-not $repositoryFilesPresent) { $missing += "Required Nexora repository files" }
if (-not $rustc.Available -or $rustc.VersionQuery -ne "OK") { $missing += "rustc" }
if (-not $cargo.Available -or $cargo.VersionQuery -ne "OK") { $missing += "Cargo" }
if (-not $msvcLink.Available) { $missing += "MSVC linker (link.exe)" }
if (-not $blenderCommand -or $blenderVersionStatus -ne "OK") { $missing += "Blender executable/version probe" }
if ($embeddedPythonStatus -ne "OK") { $missing += "Blender embedded-Python probe" }
if (-not $rustHost -or $rustHost -ne "x86_64-pc-windows-msvc") { $missing += "Rust host target x86_64-pc-windows-msvc" }
if ($rustTargetObservation -eq "MISSING") { $missing += "Rust target x86_64-pc-windows-msvc" }

$preflightStatus = if ($missing.Count -eq 0) { "PREREQUISITES_DETECTED_NOT_TESTED" } else { "BLOCKED_MISSING_PREREQUISITES" }
$report = [ordered]@{
    schemaVersion = "1.0"
    generatedUtc = [DateTime]::UtcNow.ToString("o")
    preflightStatus = $preflightStatus
    note = "Prerequisite detection is not a test pass. The experimental transport harness is not included; no pipe, peer identity, Blender compatibility, or health-check test ran."
    host = [ordered]@{
        isWindows = $true
        windows = $osInfo
        powershellVersion = $PSVersionTable.PSVersion.ToString()
    }
    repository = [ordered]@{
        rootAccepted = (Test-Path -LiteralPath $resolvedRoot -PathType Container)
        requiredFilesPresent = $repositoryFilesPresent
        requiredFiles = $fileChecks
        git = $git
    }
    buildTools = [ordered]@{
        rustc = $rustc
        cargo = $cargo
        rustHost = $rustHost
        rustTargetX64Msvc = $rustTargetObservation
        rustup = $rustup
        msvcCompiler = $msvcCl
        msvcLinker = $msvcLink
        windowsSdkResourceCompiler = $windowsSdkRc
        node = $node
        pnpm = $pnpm
    }
    blender = [ordered]@{
        available = [bool]$blenderCommand
        source = $blenderSource
        version = $blenderVersion
        versionProbe = $blenderVersionStatus
        embeddedPythonVersion = $embeddedPythonVersion
        embeddedPythonProbe = $embeddedPythonStatus
    }
    testEligibility = @(
        [pscustomobject]@{ test = "Preflight inventory"; status = "RAN_BY_THIS_SCRIPT_ONLY" },
        [pscustomobject]@{ test = "Portable kit static consistency"; status = "AVAILABLE_SEPARATE_COMMAND" },
        [pscustomobject]@{ test = "Static documentation review"; status = "MANUAL_REVIEW" },
        [pscustomobject]@{ test = "Windows named-pipe ACL / identity / protocol"; status = "BLOCKED_NO_VALIDATION_HARNESS" },
        [pscustomobject]@{ test = "Real Blender health-check handshake"; status = "BLOCKED_NO_VALIDATION_HARNESS" }
    )
    missingPrerequisites = $missing
}

$json = $report | ConvertTo-Json -Depth 8
Write-Output $json
if (-not [string]::IsNullOrWhiteSpace($OutputPath)) {
    $outputFullPath = if ([System.IO.Path]::IsPathRooted($OutputPath)) {
        [System.IO.Path]::GetFullPath($OutputPath)
    } else {
        [System.IO.Path]::GetFullPath((Join-Path (Get-Location).Path $OutputPath))
    }
    $outputParent = Split-Path -Parent $outputFullPath
    if (-not (Test-Path -LiteralPath $outputParent -PathType Container)) {
        Write-Error "Output directory does not exist; preflight report was not saved." -ErrorAction Continue
        exit 2
    }
    Set-Content -LiteralPath $outputFullPath -Value $json -Encoding UTF8
}

if ($missing.Count -gt 0) {
    Write-Error ("BLOCKED - missing required prerequisites: " + ($missing -join "; ") + ". No transport test was run.") -ErrorAction Continue
    exit 2
}
exit 0
