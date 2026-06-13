[CmdletBinding()]
param(
    [string]$TagName = "",
    [switch]$SkipBuild,
    [switch]$SkipSmoke
)

$ErrorActionPreference = "Stop"

$isWindowsHost = [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
if (-not $isWindowsHost) {
    throw "Windows release packaging must run on Windows."
}

$scriptDir = Split-Path -Parent $PSCommandPath
$projectRoot = (Resolve-Path (Join-Path $scriptDir "..\..")).Path
$pyprojectPath = Join-Path $projectRoot "pyproject.toml"
$buildScript = Join-Path $projectRoot "packaging\nuitka\build.py"

function Read-AppVersion {
    $match = Select-String -Path $pyprojectPath -Pattern '^version\s*=\s*"([^"]+)"' -List
    if ($null -eq $match) {
        throw "Version not found: $pyprojectPath"
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

function Test-PrereleaseVersion {
    param([string]$Version)

    return $Version -match '(?i)(a|alpha|b|beta|rc|dev)[.\-]?\d*'
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
        throw "Tag/version mismatch: tag $Tag expects pyproject version $tagVersion, got $Version"
    }
}

function Assert-ReleaseLayout {
    param(
        [string]$ReleaseRoot,
        [string]$Version
    )

    $expectedRootName = "nte-gacha-$Version"
    if ((Split-Path -Leaf $ReleaseRoot) -ne $expectedRootName) {
        throw "Unexpected release directory name: $ReleaseRoot"
    }

    $requiredPaths = @(
        (Join-Path $ReleaseRoot "bin"),
        (Join-Path $ReleaseRoot "output"),
        (Join-Path $ReleaseRoot "resources"),
        (Join-Path $ReleaseRoot "nte-gacha.exe"),
        (Join-Path $ReleaseRoot "nte-gacha-cli.exe"),
        (Join-Path $ReleaseRoot "bin\nte-gacha-core.exe")
    )
    foreach ($path in $requiredPaths) {
        if (-not (Test-Path $path)) {
            throw "Release artifact is incomplete, missing: $path"
        }
    }

    $allowedRootNames = @("bin", "output", "resources", "nte-gacha.exe", "nte-gacha-cli.exe")
    $unexpectedRootEntries = @(Get-ChildItem -LiteralPath $ReleaseRoot -Force |
        Where-Object { $allowedRootNames -notcontains $_.Name } |
        Select-Object -ExpandProperty FullName)
    if ($unexpectedRootEntries.Count -gt 0) {
        throw "Release artifact contains unexpected root entries: $($unexpectedRootEntries -join ', ')"
    }

    $binDir = Join-Path $ReleaseRoot "bin"
    $unexpectedBinExe = @(Get-ChildItem -LiteralPath $binDir -Filter "*.exe" -File |
        Where-Object { $_.Name -ne "nte-gacha-core.exe" } |
        Select-Object -ExpandProperty FullName)
    if ($unexpectedBinExe.Count -gt 0) {
        throw "Release bin contains unexpected executables: $($unexpectedBinExe -join ', ')"
    }
}

function Invoke-Smoke {
    param(
        [string]$CliExe,
        [string]$Version
    )

    $actualVersion = (& $CliExe --version).Trim()
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
    if ($actualVersion -ne $Version) {
        throw "Smoke version mismatch: expected $Version, got $actualVersion"
    }

    & $CliExe debug maps list
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
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

$version = Read-AppVersion
$normalizedTagName = Normalize-TagName -Value $TagName
Assert-TagMatchesVersion -Tag $normalizedTagName -Version $version

$releaseRoot = Join-Path $projectRoot "dist\nte-gacha-$version"
$cliExe = Join-Path $releaseRoot "nte-gacha-cli.exe"
$zipName = "nte-gacha-v$version.zip"
$zipPath = Join-Path (Join-Path $projectRoot "dist") $zipName
$isPrerelease = Test-PrereleaseVersion -Version $version

Push-Location $projectRoot
try {
    if (-not $SkipBuild) {
        Write-Host "Build: $buildScript"
        poetry run python $buildScript
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    }

    Assert-ReleaseLayout -ReleaseRoot $releaseRoot -Version $version

    if (-not $SkipSmoke) {
        Write-Host "Smoke: $cliExe --version"
        Invoke-Smoke -CliExe $cliExe -Version $version
    }

    if (Test-Path $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }
    Compress-Archive -LiteralPath $releaseRoot -DestinationPath $zipPath -CompressionLevel Optimal
}
finally {
    Pop-Location
}

if ([string]::IsNullOrWhiteSpace($normalizedTagName)) {
    $normalizedTagName = "v$version"
}

Write-WorkflowOutput -Name "version" -Value $version
Write-WorkflowOutput -Name "tagName" -Value $normalizedTagName
Write-WorkflowOutput -Name "releaseDir" -Value $releaseRoot
Write-WorkflowOutput -Name "zipPath" -Value $zipPath
Write-WorkflowOutput -Name "assetName" -Value $zipName
Write-WorkflowOutput -Name "prerelease" -Value $isPrerelease.ToString().ToLowerInvariant()
