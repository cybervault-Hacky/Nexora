$ErrorActionPreference = "Stop"

# Reproducible Windows 10/11 x64 Per-User NSIS Packaging & Checksum Script
# Usage (from repository root in PowerShell 7+ or Windows PowerShell 5.1+):
#   ./scripts/windows/build-installer.ps1

Write-Host "==> [1/5] Verifying toolchain & clean secret environment..."
$ForbiddenEnv = @(
    "STRIPE_SECRET_KEY",
    "STRIPE_WEBHOOK_SECRET",
    "RAZORPAY_KEY_SECRET",
    "RAZORPAY_WEBHOOK_SECRET",
    "NEXORA_AUTH_GOOGLE_CLIENT_SECRET",
    "NEXORA_AUTH_DATABASE_URL"
)
foreach ($varName in $ForbiddenEnv) {
    if ([Environment]::GetEnvironmentVariable($varName)) {
        throw "Refusing to build desktop installer with backend server secret '$varName' set in the build environment."
    }
}

$ConfiguredApiUrl = [Environment]::GetEnvironmentVariable("NEXORA_AUTH_API_URL")
$AllowLoopbackHttp = ([Environment]::GetEnvironmentVariable("NEXORA_ALLOW_LOOPBACK_HTTP") -eq "true")
if ([string]::IsNullOrWhiteSpace($ConfiguredApiUrl)) {
    Write-Warning "NEXORA_AUTH_API_URL is not set: building offline-first desktop binary (online account endpoints will report 'backend_unconfigured' until built with an https:// origin)."
} elseif ($ConfiguredApiUrl.StartsWith("https://", [System.StringComparison]::OrdinalIgnoreCase)) {
    Write-Host "  Backend API endpoint: compile-time HTTPS origin configured (value withheld)."
} elseif ($AllowLoopbackHttp -and ($ConfiguredApiUrl -match "^http://(127\.0\.0\.1|localhost|\[::1\])(:\d+)?(/.*)?$")) {
    Write-Warning "Building installer with local loopback HTTP override enabled (NEXORA_ALLOW_LOOPBACK_HTTP=true; local QA only)."
} else {
    throw "Refusing to build Windows release installer: NEXORA_AUTH_API_URL must use https:// (plaintext http:// is rejected)."
}

Write-Host "==> [2/5] Installing frozen dependencies, typechecking, and running UI tests..."
pnpm install --frozen-lockfile
pnpm typecheck
pnpm test

Write-Host "==> [3/5] Building native Windows x64 NSIS per-user installer..."
pnpm --filter @nexora/desktop tauri build --bundles nsis

$BundleDir = Join-Path $PSScriptRoot "..\..\apps\desktop\src-tauri\target\release\bundle\nsis"
$Installers = Get-ChildItem -Path $BundleDir -Filter "*-setup.exe" -ErrorAction Stop
if ($Installers.Count -eq 0) {
    throw "No NSIS setup executable produced in $BundleDir"
}

Write-Host "==> [4/5] Optional Authenticode code signing check..."
if ($env:SIGNTOOL_PFX_PATH -and $env:SIGNTOOL_PFX_PASSWORD) {
    foreach ($exe in $Installers) {
        Write-Host "Signing $($exe.Name) with Authenticode SHA-256 + RFC3161 timestamp..."
        signtool.exe sign /f $env:SIGNTOOL_PFX_PATH /p $env:SIGNTOOL_PFX_PASSWORD `
            /fd SHA256 /tr "http://timestamp.digicert.com" /td SHA256 $exe.FullName
    }
} else {
    Write-Host "Skipping Authenticode signing (SIGNTOOL_PFX_PATH not set — unsigned artifact)."
}

Write-Host "==> [5/5] Generating SHA-256 verification manifest..."
$ManifestPath = Join-Path $BundleDir "SHA256SUMS.txt"
$Lines = @()
foreach ($exe in $Installers) {
    $Hash = (Get-FileHash -Path $exe.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $Lines += "$Hash  $($exe.Name)"
    Write-Host "  SHA-256: $Hash  $($exe.Name) ($([math]::Round($exe.Length / 1MB, 2)) MiB)"
}
Set-Content -Path $ManifestPath -Value $Lines -Encoding UTF8
Write-Host "==> Complete: $ManifestPath"
