[CmdletBinding()]
param(
    [switch]$SkipInstall,
    [switch]$RunSmoke,
    [switch]$AllowGnuRust,
    [int]$KeepRuns = 1,
    [switch]$KeepPortable
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
        throw "agent smoke portable build must run on Windows."
    }
    if (-not [string]::IsNullOrWhiteSpace($env:WSL_DISTRO_NAME)) {
        throw "WSL environment detected. Run from native Windows PowerShell."
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
            throw "Rust host must be x86_64-pc-windows-msvc for native Windows Tauri build. Current: $hostLine"
        }
    }
    Write-Ok $hostLine
}

function Read-WorkspaceVersion {
    param([string]$Path)

    $inWorkspacePackage = $false
    foreach ($line in [System.IO.File]::ReadLines($Path)) {
        $trimmed = $line.Trim()
        if ($trimmed -match '^\[(.+)\]$') {
            $inWorkspacePackage = $Matches[1] -eq "workspace.package"
            continue
        }
        if ($inWorkspacePackage -and $trimmed -match '^version\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }
    throw "Workspace package version not found: $Path"
}

function Clear-SmokeInput {
    param(
        [string]$Path,
        [string]$ExpectedParent
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }

    $item = Get-Item -LiteralPath $Path
    $parent = Split-Path -Parent $item.FullName
    $leaf = Split-Path -Leaf $item.FullName
    if ($leaf -ne "smoke-input-current" -or $parent -ne $ExpectedParent) {
        throw "Refusing to clear unexpected smoke input path: $($item.FullName)"
    }
    Remove-Item -LiteralPath $item.FullName -Force -Recurse
}

function New-ReleaseJson {
    param(
        [string]$Path,
        [string]$Version
    )

    $payload = [ordered]@{
        schema = "nte-gacha-exporter-release"
        schema_version = 1
        version = $Version
    }
    Write-JsonNoBom -Path $Path -Value $payload
}

Assert-WindowsHost
if ($KeepRuns -lt 1) {
    throw "KeepRuns must be at least 1."
}

$scriptDir = Split-Path -Parent $PSCommandPath
$projectRoot = (Resolve-Path (Join-Path $scriptDir "..\..")).Path
$desktopRoot = Join-Path $projectRoot "apps\desktop"
$version = Read-WorkspaceVersion -Path (Join-Path $projectRoot "Cargo.toml")
$smokeTargetDir = Join-Path $projectRoot "target\agent-smoke-build"
$agentSmokeDir = Join-Path $projectRoot "target\agent-smoke"
$smokeInput = Join-Path $agentSmokeDir "smoke-input-current"

Write-Step "Agent smoke build environment"
Write-Host "Repo: $projectRoot"
Write-Ok "version -> $version"
foreach ($name in @("bun", "cargo", "rustc", "node")) {
    Assert-Command -Name $name
}
Assert-RustHost

$previousTargetDir = $env:CARGO_TARGET_DIR
try {
    $env:CARGO_TARGET_DIR = $smokeTargetDir
    Write-Ok "CARGO_TARGET_DIR -> $env:CARGO_TARGET_DIR"

    if (-not $SkipInstall) {
        Invoke-External -Name "bun install" -FilePath "bun" -Arguments @("install", "--frozen-lockfile") -WorkingDirectory $desktopRoot
    }

    Invoke-External -Name "Tauri smoke build" -FilePath "bun" -Arguments @("run", "tauri", "build", "--no-bundle", "--features", "agent-smoke") -WorkingDirectory $desktopRoot
    Invoke-External -Name "Portable tools smoke build" -FilePath "cargo" -Arguments @("build", "--release", "-p", "nte-gacha-exporter-cli") -WorkingDirectory $projectRoot

    $targetRelease = Join-Path $smokeTargetDir "release"
    $launcher = Join-Path $targetRelease "nte-gacha-exporter.exe"
    $cli = Join-Path $targetRelease "nte-gacha-exporter-cli.exe"
    $desktopExe = Join-Path $targetRelease "nte-gacha-exporter-desktop.exe"
    $updater = Join-Path $targetRelease "nte-gacha-exporter-updater.exe"
    foreach ($path in @($launcher, $cli, $desktopExe, $updater)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Smoke artifact missing: $path"
        }
    }

    if (-not (Test-Path -LiteralPath $agentSmokeDir)) {
        New-Item -ItemType Directory -Path $agentSmokeDir | Out-Null
    }
    Clear-SmokeInput -Path $smokeInput -ExpectedParent $agentSmokeDir

    $appDir = Join-Path $smokeInput "app"
    New-Item -ItemType Directory -Force -Path $appDir | Out-Null
    Copy-Item -LiteralPath $launcher -Destination (Join-Path $smokeInput "nte-gacha-exporter.exe")
    Copy-Item -LiteralPath $cli -Destination (Join-Path $smokeInput "nte-gacha-exporter-cli.exe")
    Copy-Item -LiteralPath $desktopExe -Destination (Join-Path $appDir "nte-gacha-exporter-desktop.exe")
    Copy-Item -LiteralPath $updater -Destination (Join-Path $appDir "nte-gacha-exporter-updater.exe")
    New-ReleaseJson -Path (Join-Path $appDir "release.json") -Version $version

    Write-Ok "Smoke portable: $smokeInput"
    Write-Host "Run: cargo smoke"

    if ($RunSmoke) {
        $smokeArgs = @(
            "smoke",
            "--release-root",
            "target\agent-smoke\smoke-input-current",
            "--keep-runs",
            "$KeepRuns"
        )
        if ($KeepPortable) {
            $smokeArgs += "--keep-portable"
        }
        Invoke-External -Name "Agent smoke" -FilePath "cargo" -Arguments $smokeArgs -WorkingDirectory $projectRoot
    }
}
finally {
    $env:CARGO_TARGET_DIR = $previousTargetDir
}
