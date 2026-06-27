[CmdletBinding()]
param(
    [switch]$ChecksOnly,
    [switch]$SkipInstall,
    [switch]$SkipTauriBuild,
    [switch]$SkipPortableStage,
    [switch]$SkipSmoke,
    [switch]$AllowGnuRust,
    [string]$TagName = "",
    [string]$AssetsPackZip = ""
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$scriptDir = Split-Path -Parent $PSCommandPath
. (Join-Path $scriptDir "scripts\build-win-common.ps1")
. (Join-Path $scriptDir "scripts\build-win-portable.ps1")

Assert-WindowsHost

$projectRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
$desktopRoot = Join-Path $projectRoot "apps\desktop"
$desktopVersion = Read-WorkspaceVersion -Path (Join-Path $projectRoot "Cargo.toml")
$normalizedTagName = Normalize-TagName -Value $TagName
Assert-TagMatchesVersion -Tag $normalizedTagName -Version $desktopVersion
if ([string]::IsNullOrWhiteSpace($normalizedTagName)) {
    $normalizedTagName = "v$desktopVersion"
}
$isPrerelease = Test-PrereleaseVersion -Version $desktopVersion

Write-Step "Native Windows release environment"
Write-Host "Repo: $projectRoot"
Write-Ok "version -> $desktopVersion"
Write-Ok "tag -> $normalizedTagName"
$requiredCommands = @()
$needsBun = $ChecksOnly -or (-not $SkipInstall) -or (-not $SkipTauriBuild)
$needsRust = $ChecksOnly -or (-not $SkipTauriBuild)
if ($needsBun) {
    $requiredCommands += @("bun", "bunx", "node")
}
if ($needsRust) {
    $requiredCommands += @("cargo", "rustc")
}
foreach ($name in $requiredCommands) {
    Assert-Command -Name $name
}
if ($needsRust) {
    Assert-RustHost
}

if ($ChecksOnly) {
    Write-Ok "ChecksOnly complete. Native Windows release toolchain looks usable."
    return
}

if (-not $SkipInstall) {
    Invoke-External -Name "bun install" -FilePath "bun" -Arguments @("install", "--frozen-lockfile") -WorkingDirectory $desktopRoot
}

if (-not $SkipTauriBuild) {
    $tauriBuildArgs = @("run", "tauri", "build", "--no-bundle")
    $bunxTauriBuildArgs = @("@tauri-apps/cli", "build", "--no-bundle")

    $localTauriCandidates = @(
        (Join-Path $desktopRoot "node_modules\.bin\tauri.cmd"),
        (Join-Path $desktopRoot "node_modules\.bin\tauri.exe"),
        (Join-Path $desktopRoot "node_modules\.bin\tauri")
    )
    $hasLocalTauri = $false
    foreach ($path in $localTauriCandidates) {
        if (Test-Path -LiteralPath $path -PathType Leaf) {
            $hasLocalTauri = $true
            break
        }
    }
    if ($hasLocalTauri) {
        Invoke-External -Name "Tauri build" -FilePath "bun" -Arguments $tauriBuildArgs -WorkingDirectory $desktopRoot
    }
    else {
        Invoke-External -Name "Tauri build via bunx" -FilePath "bunx" -Arguments $bunxTauriBuildArgs -WorkingDirectory $desktopRoot
    }
    Invoke-External -Name "Portable tools build" -FilePath "cargo" -Arguments @("build", "--release", "-p", "nte-gacha-exporter-cli") -WorkingDirectory $projectRoot
}

$portableRoot = $null
if (-not $SkipPortableStage) {
    $portableRoot = New-PortableStage -ProjectRoot $projectRoot -Version $desktopVersion -TagName $normalizedTagName -IsPrerelease $isPrerelease -AssetsPackZip $AssetsPackZip
}

if (-not $SkipSmoke) {
    if ($null -eq $portableRoot) {
        throw "Smoke requires portable stage. Remove -SkipPortableStage or pass -SkipSmoke."
    }
    Invoke-PortableRunSmoke -ReleaseRoot $portableRoot -Version $desktopVersion
}

Write-Ok "Windows release package complete."
