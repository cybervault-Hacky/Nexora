$ErrorActionPreference = "Stop"

# Pinned Windows 10/11 x64 Per-User NSIS Packaging & Checksum Script
# Usage (from repository root in PowerShell 7+ or Windows PowerShell 5.1+):
#   ./scripts/windows/build-installer.ps1

$TargetTriple = "x86_64-pc-windows-msvc"
$PinnedNodeVersion = "v22.22.3"
$PinnedRustVersion = "1.93.0"
$RepositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$BundleDir = [System.IO.Path]::GetFullPath((Join-Path $RepositoryRoot "apps\desktop\src-tauri\target\$TargetTriple\release\bundle\nsis"))
$ManifestPath = Join-Path $BundleDir "SHA256SUMS.txt"

function Invoke-CheckedNative {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    Push-Location -Path $RepositoryRoot
    try {
        & $Command @Arguments
        $CommandExitCode = $LASTEXITCODE
    } finally {
        Pop-Location
    }
    if ($CommandExitCode -ne 0) {
        throw "Command '$Command' failed with exit code $CommandExitCode."
    }
}

if ($env:OS -ne "Windows_NT") {
    throw "This script must run on a native Windows host; a Linux build is not a Windows installer."
}
if (-not [Environment]::Is64BitOperatingSystem -or -not [Environment]::Is64BitProcess) {
    throw "The supported installer target is native x64 Windows running a 64-bit PowerShell process."
}
foreach ($commandName in @("node", "pnpm", "cargo", "rustc", "rustup")) {
    if (-not (Get-Command $commandName -ErrorAction SilentlyContinue)) {
        throw "Required Windows packaging command '$commandName' is not on PATH."
    }
}

foreach ($compilerTool in @("cl.exe", "link.exe", "rc.exe")) {
    if (-not (Get-Command $compilerTool -ErrorAction SilentlyContinue)) {
        throw "Required MSVC/Windows SDK tool '$compilerTool' is not on PATH. Run from a configured Visual Studio 2022 x64 developer shell."
    }
}

Push-Location -Path $RepositoryRoot
try {
    $NodeVersion = (& node --version)
    if ($LASTEXITCODE -ne 0 -or $NodeVersion.Trim() -ne $PinnedNodeVersion) {
        throw "Windows packaging requires the repository-pinned Node.js $PinnedNodeVersion."
    }
    $PnpmVersion = (& pnpm --version)
    if ($LASTEXITCODE -ne 0 -or $PnpmVersion.Trim() -ne "9.15.4") {
        throw "Windows packaging requires the repository-pinned pnpm 9.15.4."
    }
    $RustVersion = (& rustc --version)
    if ($LASTEXITCODE -ne 0 -or $RustVersion -notmatch "^rustc $([regex]::Escape($PinnedRustVersion))(\s|$)") {
        throw "Windows packaging requires the repository-pinned Rust $PinnedRustVersion toolchain."
    }
    $CargoVersion = (& cargo --version)
    if ($LASTEXITCODE -ne 0 -or $CargoVersion -notmatch "^cargo $([regex]::Escape($PinnedRustVersion))(\s|$)") {
        throw "Cargo must match the repository-pinned Rust $PinnedRustVersion toolchain."
    }
    $RustcDetails = (& rustc -vV)
    if ($LASTEXITCODE -ne 0) {
        throw "Could not query the Rust compiler host triple."
    }
    $RustHostLine = $RustcDetails | Where-Object { $_ -match '^host: ' } | Select-Object -First 1
    $RustHost = if ($RustHostLine) { $RustHostLine -replace '^host:\s*', '' } else { "" }
    if ($RustHost -ne $TargetTriple) {
        throw "The Rust host must be $TargetTriple; ARM64 and 32-bit Windows installers are not configured."
    }
    $InstalledTargets = (& rustup target list --installed)
    if ($LASTEXITCODE -ne 0 -or $InstalledTargets -notcontains $TargetTriple) {
        throw "The pinned Rust toolchain does not have the required $TargetTriple target installed."
    }
} finally {
    Pop-Location
}

$RequestedEnvironment = [Environment]::GetEnvironmentVariable("NEXORA_ENV")
if (-not [string]::IsNullOrWhiteSpace($RequestedEnvironment) -and $RequestedEnvironment.Trim() -ine "production") {
    throw "Refusing to package a non-production NEXORA_ENV build (value withheld)."
}
$RequestedLoopbackOptIn = [Environment]::GetEnvironmentVariable("NEXORA_ALLOW_LOOPBACK_HTTP")
if (-not [string]::IsNullOrWhiteSpace($RequestedLoopbackOptIn) -and $RequestedLoopbackOptIn.Trim() -ieq "true") {
    throw "Refusing a production installer build with the loopback HTTP override enabled."
}

$LegacyPfxPath = [Environment]::GetEnvironmentVariable("SIGNTOOL_PFX_PATH")
$LegacyPfxPassword = [Environment]::GetEnvironmentVariable("SIGNTOOL_PFX_PASSWORD")
if (-not [string]::IsNullOrWhiteSpace($LegacyPfxPath) -or -not [string]::IsNullOrWhiteSpace($LegacyPfxPassword)) {
    throw "PFX password-based signing is disabled to avoid exposing credentials in SignTool command lines. Use an approved certificate already in CurrentUser\My and SIGNTOOL_CERT_THUMBPRINT."
}
$SigningThumbprint = [Environment]::GetEnvironmentVariable("SIGNTOOL_CERT_THUMBPRINT")
if (-not [string]::IsNullOrWhiteSpace($SigningThumbprint)) {
    $SigningThumbprint = ($SigningThumbprint -replace '\s', '').ToUpperInvariant()
    if ($SigningThumbprint -notmatch '^[0-9A-F]{40}$') {
        throw "SIGNTOOL_CERT_THUMBPRINT must be a 40-character SHA-1 certificate thumbprint (value withheld)."
    }
    $SigningCertificate = Get-Item -LiteralPath "Cert:\CurrentUser\My\$SigningThumbprint" -ErrorAction SilentlyContinue
    if (-not $SigningCertificate -or -not $SigningCertificate.HasPrivateKey) {
        throw "The configured signing certificate is not present with a private key in the current user's Personal certificate store."
    }
    $CodeSigningExtension = $SigningCertificate.Extensions |
        Where-Object { $_.Oid.Value -eq "2.5.29.37" } |
        Select-Object -First 1
    $CodeSigningEku = $null
    if ($CodeSigningExtension) {
        $CodeSigningEku = [System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new(
            $CodeSigningExtension.RawData,
            $CodeSigningExtension.Critical
        )
    }
    $CodeSigningUsages = @()
    if ($CodeSigningEku) {
        $CodeSigningUsages = @($CodeSigningEku.EnhancedKeyUsages | Where-Object { $_.Value -eq "1.3.6.1.5.5.7.3.3" })
    }
    if (-not $CodeSigningEku -or $CodeSigningUsages.Count -eq 0) {
        throw "The configured signing certificate does not declare the Code Signing enhanced key usage."
    }
    if ($SigningCertificate.NotBefore -gt (Get-Date) -or $SigningCertificate.NotAfter -le (Get-Date)) {
        throw "The configured signing certificate is outside its current validity period."
    }
    if (-not (Get-Command "signtool.exe" -ErrorAction SilentlyContinue)) {
        throw "SignTool is required when SIGNTOOL_CERT_THUMBPRINT is configured."
    }
}

Write-Host "==> [1/5] Verifying toolchain & clean secret environment..."
$ForbiddenEnv = @(
    "STRIPE_SECRET_KEY",
    "STRIPE_WEBHOOK_SECRET",
    "RAZORPAY_KEY_SECRET",
    "RAZORPAY_WEBHOOK_SECRET",
    "NEXORA_AUTH_GOOGLE_CLIENT_SECRET",
    "GOOGLE_OAUTH_CLIENT_SECRET",
    "NEXORA_AUTH_SMTP_PASSWORD",
    "NEXORA_AUTH_DATABASE_URL",
    "NEXORA_AUTH_TEST_DATABASE_URL"
)
foreach ($varName in $ForbiddenEnv) {
    if ([Environment]::GetEnvironmentVariable($varName)) {
        throw "Refusing to build desktop installer with backend server secret '$varName' set in the build environment."
    }
}

$ConfiguredApiUrl = [Environment]::GetEnvironmentVariable("NEXORA_AUTH_API_URL")
if ([string]::IsNullOrWhiteSpace($ConfiguredApiUrl)) {
    Write-Warning "NEXORA_AUTH_API_URL is not set: building offline-first desktop binary (online account endpoints will report 'backend_unconfigured' until built with an https:// origin)."
} elseif ($ConfiguredApiUrl.StartsWith("https://", [System.StringComparison]::OrdinalIgnoreCase)) {
    Write-Host "  Backend API endpoint: compile-time HTTPS origin configured (value withheld)."
} else {
    throw "Refusing to build Windows release installer: NEXORA_AUTH_API_URL must use https:// (loopback and plaintext http:// are rejected)."
}

# Never hash a leftover installer as if it came from this invocation, and never
# replace a previous artifact/manifest. Preserve old outputs and ask the operator
# to archive them elsewhere before starting a new packaging run.
$PreexistingBundleExes = @()
if (Test-Path -LiteralPath $BundleDir) {
    $PreexistingBundleExes = @(Get-ChildItem -LiteralPath $BundleDir -Filter "*.exe" -File -Force -ErrorAction Stop)
}
if ($PreexistingBundleExes.Count -gt 0 -or (Test-Path -LiteralPath $ManifestPath)) {
    $ExistingOutputNames = @($PreexistingBundleExes | ForEach-Object { $_.Name })
    if (Test-Path -LiteralPath $ManifestPath) { $ExistingOutputNames += "SHA256SUMS.txt" }
    throw "Refusing to reuse or overwrite existing NSIS output(s): $($ExistingOutputNames -join ', '). Preserve/archive them outside $BundleDir, then retry."
}

Write-Host "==> [2/5] Installing frozen dependencies, testing/building frontend, installer contract, and native Rust..."
Invoke-CheckedNative -Command "pnpm" -Arguments @("install", "--frozen-lockfile")
Invoke-CheckedNative -Command "pnpm" -Arguments @("audit")
Invoke-CheckedNative -Command "pnpm" -Arguments @("typecheck")
Invoke-CheckedNative -Command "pnpm" -Arguments @("test")
Invoke-CheckedNative -Command "pnpm" -Arguments @("test:installer-contract")
# The native Tauri macro embeds ui/dist; build it before cargo test/clippy on a clean checkout.
Invoke-CheckedNative -Command "pnpm" -Arguments @("build")
Invoke-CheckedNative -Command "cargo" -Arguments @("--locked", "fmt", "--manifest-path", "apps/desktop/src-tauri/Cargo.toml", "--", "--check")
Invoke-CheckedNative -Command "cargo" -Arguments @("--locked", "clippy", "--manifest-path", "apps/desktop/src-tauri/Cargo.toml", "--all-targets", "--", "-D", "warnings")
Invoke-CheckedNative -Command "cargo" -Arguments @("--locked", "test", "--manifest-path", "apps/desktop/src-tauri/Cargo.toml")

Write-Host "==> [3/5] Building native Windows x64 NSIS per-user installer..."
$env:NEXORA_ENV = "production"
$env:NEXORA_ALLOW_LOOPBACK_HTTP = "false"
$TauriBuildStartedUtc = [DateTime]::UtcNow
# Tauri CLI forwards arguments after `--` to Cargo; `--locked` prevents build-time lockfile drift.
Invoke-CheckedNative -Command "pnpm" -Arguments @("--filter", "@nexora/desktop", "tauri", "build", "--target", $TargetTriple, "--bundles", "nsis", "--", "--locked")

$Installers = @(Get-ChildItem -LiteralPath $BundleDir -Filter "*-setup.exe" -File -Force -ErrorAction Stop)
if ($Installers.Count -ne 1) {
    throw "Expected exactly one NSIS setup executable in $BundleDir, found $($Installers.Count). Refusing to hash stale or ambiguous artifacts."
}
if ($Installers[0].Length -le 0 -or $Installers[0].LastWriteTimeUtc -lt $TauriBuildStartedUtc) {
    throw "The NSIS setup output is empty or predates this Tauri build invocation; refusing to treat it as a fresh installer."
}

Write-Host "==> [4/5] Optional Authenticode code signing check..."
if ([string]::IsNullOrWhiteSpace($SigningThumbprint)) {
    Write-Host "Skipping Authenticode signing (no certificate-store thumbprint configured - unsigned artifact)."
} else {
    foreach ($exe in $Installers) {
        Write-Host "Signing $($exe.Name) with the configured current-user Code Signing certificate (thumbprint withheld)..."
        & signtool.exe sign /s My /sha1 $SigningThumbprint /fd SHA256 `
            /tr "http://timestamp.digicert.com" /td SHA256 $exe.FullName
        if ($LASTEXITCODE -ne 0) {
            throw "Authenticode signing failed for $($exe.Name) with exit code $LASTEXITCODE."
        }
        $Signature = Get-AuthenticodeSignature -FilePath $exe.FullName
        $ActualSignerThumbprint = if ($Signature.SignerCertificate) {
            ($Signature.SignerCertificate.Thumbprint -replace '\s', '').ToUpperInvariant()
        } else {
            ""
        }
        if ($Signature.Status -ne "Valid" -or $ActualSignerThumbprint -ne $SigningThumbprint) {
            throw "Authenticode verification failed for $($exe.Name) (status: $($Signature.Status)); the signer must match the configured certificate-store thumbprint."
        }
    }
}
Write-Host "==> [5/5] Generating SHA-256 verification manifest..."
$Lines = @()
foreach ($exe in $Installers) {
    $Hash = (Get-FileHash -LiteralPath $exe.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $Lines += "$Hash  $($exe.Name)"
    Write-Host "  SHA-256: $Hash  $($exe.Name) ($([math]::Round($exe.Length / 1MB, 2)) MiB)"
}

# Publish the manifest with create-new semantics so a racing or pre-existing
# file is never overwritten. The temporary name is unique and lives beside it.
$ManifestTemporaryPath = "$ManifestPath.$([Guid]::NewGuid().ToString('N')).tmp"
$ManifestTemporaryCreated = $false
$ManifestStream = $null
try {
    $ManifestStream = [System.IO.File]::Open(
        $ManifestTemporaryPath,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::Read
    )
    $ManifestTemporaryCreated = $true
    $ManifestBytes = [System.Text.Encoding]::UTF8.GetBytes(($Lines -join "`r`n") + "`r`n")
    $ManifestStream.Write($ManifestBytes, 0, $ManifestBytes.Length)
    $ManifestStream.Flush()
    $ManifestStream.Dispose()
    $ManifestStream = $null
    [System.IO.File]::Move($ManifestTemporaryPath, $ManifestPath)
    $ManifestTemporaryCreated = $false
} finally {
    if ($ManifestStream) { $ManifestStream.Dispose() }
    if ($ManifestTemporaryCreated -and (Test-Path -LiteralPath $ManifestTemporaryPath)) {
        Remove-Item -LiteralPath $ManifestTemporaryPath -Force -ErrorAction SilentlyContinue
    }
}
Write-Host "==> Complete: $ManifestPath"
