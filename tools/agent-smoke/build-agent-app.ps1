[CmdletBinding()]
param(
    [switch]$SkipInstall,
    [switch]$AllowGnuRust
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
        throw "agent app build must run on Windows."
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

function Clear-AgentAppBuildOwnedPaths {
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
    if ($leaf -ne "app-current" -or $parent -ne $ExpectedParent) {
        throw "Refusing to clear unexpected agent app path: $($item.FullName)"
    }

    $buildOwnedFiles = @(
        (Join-Path $item.FullName "nte-gacha-exporter.exe"),
        (Join-Path $item.FullName "nte-gacha-exporter-cli.exe"),
        (Join-Path $item.FullName "app\nte-gacha-exporter-desktop.exe"),
        (Join-Path $item.FullName "app\nte-gacha-exporter-updater.exe"),
        (Join-Path $item.FullName "app\release.json")
    )
    foreach ($file in $buildOwnedFiles) {
        if (Test-Path -LiteralPath $file -PathType Leaf) {
            Remove-Item -LiteralPath $file -Force
        }
    }
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

$scriptDir = Split-Path -Parent $PSCommandPath
$projectRoot = (Resolve-Path (Join-Path $scriptDir "..\..")).Path
$desktopRoot = Join-Path $projectRoot "apps\desktop"
$version = Read-WorkspaceVersion -Path (Join-Path $projectRoot "Cargo.toml")
$agentBuildTargetDir = Join-Path $projectRoot "target\agent-smoke-build"
$agentOutDir = Join-Path $projectRoot "target\agent-smoke"
$agentApp = Join-Path $agentOutDir "app-current"

Write-Step "Agent app build environment"
Write-Host "Repo: $projectRoot"
Write-Ok "version -> $version"
foreach ($name in @("bun", "cargo", "rustc", "node")) {
    Assert-Command -Name $name
}
Assert-RustHost

$previousTargetDir = $env:CARGO_TARGET_DIR
try {
    $env:CARGO_TARGET_DIR = $agentBuildTargetDir
    Write-Ok "CARGO_TARGET_DIR -> $env:CARGO_TARGET_DIR"

    if (-not $SkipInstall) {
        Invoke-External -Name "bun install" -FilePath "bun" -Arguments @("install", "--frozen-lockfile") -WorkingDirectory $desktopRoot
    }

    Invoke-External -Name "Tauri agent app build" -FilePath "bun" -Arguments @("run", "tauri", "build", "--no-bundle", "--features", "agent-smoke") -WorkingDirectory $desktopRoot
    Invoke-External -Name "Portable CLI build" -FilePath "cargo" -Arguments @("build", "--release", "-p", "nte-gacha-exporter-cli") -WorkingDirectory $projectRoot

    $targetRelease = Join-Path $agentBuildTargetDir "release"
    $launcher = Join-Path $targetRelease "nte-gacha-exporter.exe"
    $cli = Join-Path $targetRelease "nte-gacha-exporter-cli.exe"
    $desktopExe = Join-Path $targetRelease "nte-gacha-exporter-desktop.exe"
    $updater = Join-Path $targetRelease "nte-gacha-exporter-updater.exe"
    foreach ($path in @($launcher, $cli, $desktopExe, $updater)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Agent app artifact missing: $path"
        }
    }

    if (-not (Test-Path -LiteralPath $agentOutDir)) {
        New-Item -ItemType Directory -Path $agentOutDir | Out-Null
    }
    if (-not (Test-Path -LiteralPath $agentApp)) {
        New-Item -ItemType Directory -Path $agentApp | Out-Null
    }
    Clear-AgentAppBuildOwnedPaths -Path $agentApp -ExpectedParent $agentOutDir

    $appDir = Join-Path $agentApp "app"
    New-Item -ItemType Directory -Force -Path $appDir | Out-Null
    Copy-Item -LiteralPath $launcher -Destination (Join-Path $agentApp "nte-gacha-exporter.exe")
    Copy-Item -LiteralPath $cli -Destination (Join-Path $agentApp "nte-gacha-exporter-cli.exe")
    Copy-Item -LiteralPath $desktopExe -Destination (Join-Path $appDir "nte-gacha-exporter-desktop.exe")
    Copy-Item -LiteralPath $updater -Destination (Join-Path $appDir "nte-gacha-exporter-updater.exe")
    New-ReleaseJson -Path (Join-Path $appDir "release.json") -Version $version

    Write-Ok "Agent app: $agentApp"
    Write-Host "Run: cargo agent launch"
}
finally {
    $env:CARGO_TARGET_DIR = $previousTargetDir
}
