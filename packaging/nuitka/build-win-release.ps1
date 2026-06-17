[CmdletBinding()]
param(
    [switch]$ChecksOnly,
    [switch]$SkipInstall,
    [switch]$SkipSidecarBuild,
    [switch]$SkipTauriBuild,
    [switch]$SkipPortableStage,
    [switch]$SkipSmoke,
    [switch]$AllowGnuRust,
    [string]$TagName = ""
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "==> $Message" -ForegroundColor Cyan
}

function Write-Ok {
    param([string]$Message)
    Write-Host "OK  $Message" -ForegroundColor Green
}

function Write-WorkflowOutput {
    param(
        [string]$Name,
        [string]$Value
    )

    Write-Host "$Name=$Value"
    if (-not [string]::IsNullOrWhiteSpace($env:GITHUB_OUTPUT)) {
        Add-Content -LiteralPath $env:GITHUB_OUTPUT -Value "$Name=$Value" -Encoding utf8
    }
}

function Write-JsonNoBom {
    param(
        [string]$Path,
        [object]$Value
    )

    $json = ($Value | ConvertTo-Json)
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($Path, "$json`n", $encoding)
}

function Assert-WindowsHost {
    $isWindows = [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
    if (-not $isWindows) {
        throw "Windows release packaging must run on Windows."
    }
    if (-not [string]::IsNullOrWhiteSpace($env:WSL_DISTRO_NAME)) {
        throw "WSL environment detected. Run from native Windows PowerShell."
    }
}

function Invoke-External {
    param(
        [string]$Name,
        [string]$FilePath,
        [string[]]$Arguments = @(),
        [string]$WorkingDirectory = ""
    )

    Write-Step $Name
    if ([string]::IsNullOrWhiteSpace($WorkingDirectory)) {
        & $FilePath @Arguments
    }
    else {
        Push-Location $WorkingDirectory
        try {
            & $FilePath @Arguments
        }
        finally {
            Pop-Location
        }
    }
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

function Assert-Command {
    param([string]$Name)

    $command = Get-Command $Name -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        throw "Missing command: $Name"
    }
    Write-Ok "$Name -> $($command.Source)"
}

function Assert-RustHost {
    $rustInfo = (& rustc -vV) -join "`n"
    $hostLine = ($rustInfo -split "`n" | Where-Object { $_ -like "host:*" } | Select-Object -First 1)
    if ([string]::IsNullOrWhiteSpace($hostLine)) {
        throw "Cannot read rustc host from rustc -vV."
    }
    if ($hostLine -notmatch "pc-windows-msvc") {
        if ($AllowGnuRust) {
            Write-Warning "Rust host is not MSVC: $hostLine"
        }
        else {
            throw "Rust host must be x86_64-pc-windows-msvc for native Windows Tauri packaging. Current: $hostLine"
        }
    }
    Write-Ok $hostLine
}

function Read-VersionFromToml {
    param([string]$Path)

    $match = Select-String -Path $Path -Pattern '^version\s*=\s*"([^"]+)"' -List
    if ($null -eq $match) {
        throw "Version not found: $Path"
    }
    return $match.Matches[0].Groups[1].Value
}

function Normalize-TagName {
    param([string]$Value)

    $clean = $Value.Trim()
    if ($clean.StartsWith("refs/tags/")) {
        return $clean.Substring("refs/tags/".Length)
    }
    return $clean
}

function Assert-TagMatchesVersion {
    param(
        [string]$Tag,
        [string]$Version
    )

    if ([string]::IsNullOrWhiteSpace($Tag)) {
        return
    }
    if (-not $Tag.StartsWith("v")) {
        throw "Release tag must start with v: $Tag"
    }

    $tagVersion = $Tag.Substring(1)
    if ($tagVersion -ne $Version) {
        throw "Tag/version mismatch: tag $Tag expects desktop version $tagVersion, got $Version"
    }
}

function Test-PrereleaseVersion {
    param([string]$Version)

    return $Version -match '(?i)(a|alpha|b|beta|rc|dev)[.\-]?\d*'
}

function New-ReleaseJson {
    param(
        [string]$Path,
        [string]$Version
    )

    $payload = [ordered]@{
        schema = "nte-gacha-release"
        schema_version = 1
        version = $Version
    }
    Write-JsonNoBom -Path $Path -Value $payload
}

function Copy-DirectoryContents {
    param(
        [string]$Source,
        [string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source -PathType Container)) {
        throw "Directory not found: $Source"
    }
    if (Test-Path -LiteralPath $Destination) {
        throw "Portable staging destination already exists: $Destination"
    }
    New-Item -ItemType Directory -Path $Destination | Out-Null
    Get-ChildItem -LiteralPath $Source -Force | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $Destination -Recurse
    }
}

function Get-ZipEntryName {
    param(
        [string]$SourceRoot,
        [string]$Path
    )

    $root = [System.IO.Path]::GetFullPath($SourceRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
    $full = [System.IO.Path]::GetFullPath($Path)
    $relative = $full.Substring($root.Length).TrimStart([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
    return $relative.Replace("\", "/")
}

function Compress-PortableZip {
    param(
        [string]$SourceRoot,
        [string]$ZipPath
    )

    if (Test-Path -LiteralPath $ZipPath) {
        Remove-Item -LiteralPath $ZipPath -Force
    }

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::Open($ZipPath, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        Get-ChildItem -LiteralPath $SourceRoot -Directory -Recurse -Force | ForEach-Object {
            $entryName = (Get-ZipEntryName -SourceRoot $SourceRoot -Path $_.FullName) + "/"
            [void]$zip.CreateEntry($entryName)
        }
        Get-ChildItem -LiteralPath $SourceRoot -File -Recurse -Force | ForEach-Object {
            $entryName = Get-ZipEntryName -SourceRoot $SourceRoot -Path $_.FullName
            $entry = $zip.CreateEntry($entryName, [System.IO.Compression.CompressionLevel]::Optimal)
            $input = [System.IO.File]::OpenRead($_.FullName)
            try {
                $output = $entry.Open()
                try {
                    $input.CopyTo($output)
                }
                finally {
                    $output.Dispose()
                }
            }
            finally {
                $input.Dispose()
            }
        }
    }
    finally {
        $zip.Dispose()
    }
}

function New-PortableManifest {
    param(
        [string]$ZipPath,
        [string]$ManifestPath,
        [string]$Version,
        [string]$TagName,
        [string]$Channel
    )

    $hash = (Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
    $size = (Get-Item -LiteralPath $ZipPath).Length
    $assetName = Split-Path -Leaf $ZipPath
    $payload = [ordered]@{
        schema = "nte-gacha-update"
        schema_version = 1
        version = $Version
        channel = $Channel
        release_url = "https://github.com/Anong0u0/nte_gacha_exporter/releases/tag/$TagName"
        asset_name = $assetName
        download_url = "https://github.com/Anong0u0/nte_gacha_exporter/releases/download/$TagName/$assetName"
        sha256 = $hash
        size = $size
    }
    Write-JsonNoBom -Path $ManifestPath -Value $payload
}

function Assert-SidecarRelease {
    param(
        [string]$ReleaseRoot,
        [string]$Version
    )

    $expectedRootName = "nte-gacha-sidecar-$Version"
    if ((Split-Path -Leaf $ReleaseRoot) -ne $expectedRootName) {
        throw "Unexpected sidecar release directory name: $ReleaseRoot"
    }

    $requiredPaths = @(
        (Join-Path $ReleaseRoot "nte-gacha-python-core.exe"),
        (Join-Path $ReleaseRoot "bin\nte-gacha-core.exe"),
        (Join-Path $ReleaseRoot "resources\maps"),
        (Join-Path $ReleaseRoot "resources\automation")
    )
    foreach ($path in $requiredPaths) {
        if (-not (Test-Path -LiteralPath $path)) {
            throw "Sidecar release artifact is incomplete, missing: $path"
        }
    }
}

function Clear-PortableStageBuildOwnedPaths {
    param(
        [string]$ProjectRoot,
        [string]$ReleaseRoot,
        [string]$Version
    )

    if (-not (Test-Path -LiteralPath $ReleaseRoot)) {
        return
    }
    $expectedName = "nte-gacha-desktop-v$Version-windows-x64"
    $item = Get-Item -LiteralPath $ReleaseRoot
    if ($item.Parent.FullName -ne (Join-Path $ProjectRoot "dist") -or $item.Name -ne $expectedName) {
        throw "Refusing to clear unexpected portable stage: $ReleaseRoot"
    }
    foreach ($name in @("nte-gacha.exe", "app", "sidecars", "update")) {
        $path = Join-Path $ReleaseRoot $name
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Force -Recurse
        }
    }
}

function Assert-PortableStageContent {
    param([string]$ReleaseRoot)

    $requiredPaths = @(
        (Join-Path $ReleaseRoot "nte-gacha.exe"),
        (Join-Path $ReleaseRoot "app\nte-gacha-desktop.exe"),
        (Join-Path $ReleaseRoot "app\nte-gacha-updater.exe"),
        (Join-Path $ReleaseRoot "app\release.json"),
        (Join-Path $ReleaseRoot "sidecars\nte-gacha-python-core.exe"),
        (Join-Path $ReleaseRoot "sidecars\bin\nte-gacha-core.exe"),
        (Join-Path $ReleaseRoot "sidecars\resources\maps"),
        (Join-Path $ReleaseRoot "sidecars\resources\automation")
    )
    foreach ($path in $requiredPaths) {
        if (-not (Test-Path -LiteralPath $path)) {
            throw "Portable stage is incomplete, missing: $path"
        }
    }

    $sidecarCmd = @(Get-ChildItem -LiteralPath (Join-Path $ReleaseRoot "sidecars") -Filter "*.cmd" -File -Recurse)
    if ($sidecarCmd.Count -gt 0) {
        throw "Portable stage must not contain development sidecar command files: $($sidecarCmd[0].FullName)"
    }

    Get-ChildItem -LiteralPath $ReleaseRoot -File -Recurse | Where-Object {
        $_.Extension.ToLowerInvariant() -in @(".bat", ".cmd", ".ps1", ".txt")
    } | ForEach-Object {
        $text = [System.IO.File]::ReadAllText($_.FullName)
        if ($text.Contains(".local")) {
            throw "Portable stage must not contain .local development paths: $($_.FullName)"
        }
    }
}

function New-PortableStage {
    param(
        [string]$ProjectRoot,
        [string]$DesktopRoot,
        [string]$Version,
        [string]$TagName,
        [bool]$IsPrerelease
    )

    $distRoot = Join-Path $ProjectRoot "dist"
    $releaseRoot = Join-Path $distRoot "nte-gacha-desktop-v$Version-windows-x64"
    $zipPath = Join-Path $distRoot "nte-gacha-desktop-v$Version-windows-x64.zip"
    $manifestPath = Join-Path $distRoot "nte-gacha-update.json"
    Clear-PortableStageBuildOwnedPaths -ProjectRoot $ProjectRoot -ReleaseRoot $releaseRoot -Version $Version
    if (-not (Test-Path -LiteralPath $distRoot)) {
        New-Item -ItemType Directory -Path $distRoot | Out-Null
    }

    $targetRelease = Join-Path $DesktopRoot "target\release"
    $launcher = Join-Path $targetRelease "nte-gacha.exe"
    $desktopExe = Join-Path $targetRelease "nte-gacha-desktop.exe"
    $updater = Join-Path $targetRelease "nte-gacha-updater.exe"
    foreach ($path in @($launcher, $desktopExe, $updater)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Portable artifact missing: $path"
        }
    }

    $sidecarRelease = Join-Path $ProjectRoot "dist\nte-gacha-sidecar-$Version"
    Assert-SidecarRelease -ReleaseRoot $sidecarRelease -Version $Version

    $appDir = Join-Path $releaseRoot "app"
    $sidecarsDir = Join-Path $releaseRoot "sidecars"
    New-Item -ItemType Directory -Force -Path $appDir | Out-Null
    New-Item -ItemType Directory -Force -Path $sidecarsDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $releaseRoot "data") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $releaseRoot "update\downloads") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $releaseRoot "update\staging") | Out-Null

    Copy-Item -LiteralPath $launcher -Destination (Join-Path $releaseRoot "nte-gacha.exe")
    Copy-Item -LiteralPath $desktopExe -Destination (Join-Path $appDir "nte-gacha-desktop.exe")
    Copy-Item -LiteralPath $updater -Destination (Join-Path $appDir "nte-gacha-updater.exe")
    New-ReleaseJson -Path (Join-Path $appDir "release.json") -Version $Version

    Copy-Item -LiteralPath (Join-Path $sidecarRelease "nte-gacha-python-core.exe") -Destination (Join-Path $sidecarsDir "nte-gacha-python-core.exe")
    Copy-DirectoryContents -Source (Join-Path $sidecarRelease "bin") -Destination (Join-Path $sidecarsDir "bin")
    Copy-DirectoryContents -Source (Join-Path $sidecarRelease "resources") -Destination (Join-Path $sidecarsDir "resources")
    Assert-PortableStageContent -ReleaseRoot $releaseRoot

    if (Test-Path -LiteralPath $manifestPath) {
        Remove-Item -LiteralPath $manifestPath -Force
    }

    Compress-PortableZip -SourceRoot $releaseRoot -ZipPath $zipPath
    $channel = if ($IsPrerelease) { "beta" } else { "stable" }
    New-PortableManifest -ZipPath $zipPath -ManifestPath $manifestPath -Version $Version -TagName $TagName -Channel $channel

    Write-Ok "Portable stage: $releaseRoot"
    Write-Ok "Portable zip: $zipPath"
    Write-Ok "Update manifest: $manifestPath"
    Write-WorkflowOutput -Name "version" -Value $Version
    Write-WorkflowOutput -Name "tagName" -Value $TagName
    Write-WorkflowOutput -Name "releaseDir" -Value $releaseRoot
    Write-WorkflowOutput -Name "zipPath" -Value $zipPath
    Write-WorkflowOutput -Name "assetName" -Value (Split-Path -Leaf $zipPath)
    Write-WorkflowOutput -Name "manifestPath" -Value $manifestPath
    Write-WorkflowOutput -Name "manifestName" -Value "nte-gacha-update.json"
    Write-WorkflowOutput -Name "prerelease" -Value $IsPrerelease.ToString().ToLowerInvariant()
    return $releaseRoot
}

function Invoke-SidecarSmoke {
    param([string]$ReleaseRoot)

    $sidecar = Join-Path $ReleaseRoot "sidecars\nte-gacha-python-core.exe"
    Write-Step "sidecar smoke"
    $request = '{"jsonrpc":"2.0","id":1,"method":"app.ping"}'
    $response = $request | & $sidecar
    if ($LASTEXITCODE -ne 0) {
        throw "sidecar smoke failed with exit code $LASTEXITCODE"
    }
    if ($response -notmatch '"ok"\s*:\s*true') {
        throw "unexpected sidecar response: $response"
    }
    Write-Ok "sidecar app.ping"
}

function Invoke-PortableRunSmoke {
    param([string]$ReleaseRoot)

    $launcher = Join-Path $ReleaseRoot "nte-gacha.exe"
    Write-Step "portable app launch smoke"
    $before = @(Get-Process -Name "nte-gacha-desktop" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    $process = Start-Process -FilePath $launcher -WorkingDirectory $ReleaseRoot -PassThru
    Start-Sleep -Seconds 7
    if (-not $process.HasExited) {
        throw "root launcher should exit after spawning app"
    }
    $after = @(Get-Process -Name "nte-gacha-desktop" -ErrorAction SilentlyContinue |
        Where-Object { $before -notcontains $_.Id })
    if ($after.Count -eq 0) {
        throw "portable app process not found after launcher smoke"
    }
    foreach ($child in $after) {
        [void]$child.CloseMainWindow()
        if (-not $child.WaitForExit(5000)) {
            Stop-Process -Id $child.Id -Force
        }
    }
    Write-Ok "portable app launched through root launcher"
}

Assert-WindowsHost

$scriptDir = Split-Path -Parent $PSCommandPath
$projectRoot = (Resolve-Path (Join-Path $scriptDir "..\..")).Path
$desktopRoot = Join-Path $projectRoot "desktop"
$desktopVersion = Read-VersionFromToml -Path (Join-Path $desktopRoot "Cargo.toml")
$pythonVersion = Read-VersionFromToml -Path (Join-Path $projectRoot "pyproject.toml")
if ($desktopVersion -ne $pythonVersion) {
    throw "Desktop and Python release versions must match: desktop=$desktopVersion python=$pythonVersion"
}
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
$requiredCommands = @("bun", "bunx", "node", "cargo", "rustc")
if (-not $SkipSidecarBuild) {
    $requiredCommands = @("poetry") + $requiredCommands
}
foreach ($name in $requiredCommands) {
    Assert-Command -Name $name
}
Assert-RustHost

if ($ChecksOnly) {
    Write-Ok "ChecksOnly complete. Native Windows release toolchain looks usable."
    return
}

if (-not $SkipSidecarBuild) {
    Invoke-External -Name "Nuitka sidecar build" -FilePath "poetry" -Arguments @("run", "python", ".\packaging\nuitka\build.py", "--target", "sidecar") -WorkingDirectory $projectRoot
}

if (-not $SkipInstall) {
    Invoke-External -Name "bun install" -FilePath "bun" -Arguments @("install", "--frozen-lockfile") -WorkingDirectory $desktopRoot
}

if (-not $SkipTauriBuild) {
    $localTauri = Join-Path $desktopRoot "node_modules\.bin\tauri.cmd"
    if (Test-Path -LiteralPath $localTauri) {
        Invoke-External -Name "Tauri build" -FilePath "bun" -Arguments @("run", "tauri", "build") -WorkingDirectory $desktopRoot
    }
    else {
        Invoke-External -Name "Tauri build via bunx" -FilePath "bunx" -Arguments @("@tauri-apps/cli", "build") -WorkingDirectory $desktopRoot
    }
    Invoke-External -Name "Portable tools build" -FilePath "cargo" -Arguments @("build", "--release", "-p", "nte_portable_tools") -WorkingDirectory $desktopRoot
}

$portableRoot = $null
if (-not $SkipPortableStage) {
    $portableRoot = New-PortableStage -ProjectRoot $projectRoot -DesktopRoot $desktopRoot -Version $desktopVersion -TagName $normalizedTagName -IsPrerelease $isPrerelease
}

if (-not $SkipSmoke) {
    if ($null -eq $portableRoot) {
        throw "Smoke requires portable stage. Remove -SkipPortableStage or pass -SkipSmoke."
    }
    Invoke-SidecarSmoke -ReleaseRoot $portableRoot
    Invoke-PortableRunSmoke -ReleaseRoot $portableRoot
}

Write-Ok "Windows release package complete."
