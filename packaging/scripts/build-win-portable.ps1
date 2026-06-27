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

function Expand-BundledAssetsPack {
    param(
        [string]$AssetsPackZip,
        [string]$Destination
    )

    if ([string]::IsNullOrWhiteSpace($AssetsPackZip)) {
        throw "AssetsPackZip is required for portable stage."
    }
    $resolvedZip = Resolve-Path -LiteralPath $AssetsPackZip -ErrorAction SilentlyContinue
    if ($null -eq $resolvedZip) {
        throw "AssetsPackZip not found: $AssetsPackZip"
    }

    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem

    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    $destinationRoot = [System.IO.Path]::GetFullPath($Destination).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $rootPrefix = $destinationRoot + [System.IO.Path]::DirectorySeparatorChar
    $zip = [System.IO.Compression.ZipFile]::OpenRead($resolvedZip.Path)
    try {
        foreach ($entry in $zip.Entries) {
            $entryName = $entry.FullName.Replace("\", "/")
            if ([string]::IsNullOrWhiteSpace($entryName)) {
                continue
            }
            if ($entryName.EndsWith("/")) {
                continue
            }
            $segments = $entryName.Split("/")
            if ($segments | Where-Object { [string]::IsNullOrWhiteSpace($_) -or $_ -eq "." -or $_ -eq ".." }) {
                throw "Assets pack zip contains invalid entry path: $entryName"
            }
            if ($entryName -ne "manifest.json" -and -not ($entryName.StartsWith("assets/") -and $entryName.EndsWith(".webp"))) {
                throw "Assets pack zip contains unsupported entry: $entryName"
            }

            $targetPath = [System.IO.Path]::GetFullPath((Join-Path $destinationRoot $entryName))
            if (-not $targetPath.StartsWith($rootPrefix)) {
                throw "Assets pack zip entry escapes destination: $entryName"
            }

            $targetParent = Split-Path -Parent $targetPath
            New-Item -ItemType Directory -Force -Path $targetParent | Out-Null
            $input = $entry.Open()
            try {
                $output = [System.IO.File]::Open($targetPath, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write)
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

    $manifestPath = Join-Path $Destination "manifest.json"
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "Bundled assets pack missing manifest.json after extraction."
    }
    $assetsDir = Join-Path $Destination "assets"
    $assetCount = @(Get-ChildItem -LiteralPath $assetsDir -Filter "*.webp" -File -ErrorAction SilentlyContinue).Count
    if ($assetCount -le 0) {
        throw "Bundled assets pack contains no webp assets."
    }
    Write-Ok "Bundled assets pack -> $Destination ($assetCount assets)"
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
        schema = "nte-gacha-exporter-update"
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

function Clear-PortableStageBuildOwnedPaths {
    param(
        [string]$ProjectRoot,
        [string]$ReleaseRoot,
        [string]$Version
    )

    if (-not (Test-Path -LiteralPath $ReleaseRoot)) {
        return
    }
    $expectedName = Get-DesktopReleaseArtifactName -Version $Version
    $item = Get-Item -LiteralPath $ReleaseRoot
    if ($item.Parent.FullName -ne (Join-Path $ProjectRoot "dist") -or $item.Name -ne $expectedName) {
        throw "Refusing to clear unexpected portable stage: $ReleaseRoot"
    }
    foreach ($name in @("nte-gacha-exporter.exe", "nte-gacha-exporter-cli.exe", "app", "sidecars", "update")) {
        $path = Join-Path $ReleaseRoot $name
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Force -Recurse
        }
    }
}

function Assert-PortableStageContent {
    param([string]$ReleaseRoot)

    $requiredPaths = @(
        (Join-Path $ReleaseRoot "nte-gacha-exporter.exe"),
        (Join-Path $ReleaseRoot "nte-gacha-exporter-cli.exe"),
        (Join-Path $ReleaseRoot "app\nte-gacha-exporter-desktop.exe"),
        (Join-Path $ReleaseRoot "app\nte-gacha-exporter-updater.exe"),
        (Join-Path $ReleaseRoot "app\release.json"),
        (Join-Path $ReleaseRoot "app\assets-pack\current\manifest.json")
    )
    foreach ($path in $requiredPaths) {
        if (-not (Test-Path -LiteralPath $path)) {
            throw "Portable stage is incomplete, missing: $path"
        }
    }

    $sidecarsPath = Join-Path $ReleaseRoot "sidecars"
    if (Test-Path -LiteralPath $sidecarsPath) {
        throw "Portable stage must not contain legacy sidecars: $sidecarsPath"
    }

    $assetsDir = Join-Path $ReleaseRoot "app\assets-pack\current\assets"
    $assetCount = @(Get-ChildItem -LiteralPath $assetsDir -Filter "*.webp" -File -ErrorAction SilentlyContinue).Count
    if ($assetCount -le 0) {
        throw "Portable stage bundled assets pack contains no webp assets."
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
        [string]$Version,
        [string]$TagName,
        [bool]$IsPrerelease,
        [string]$AssetsPackZip
    )

    $distRoot = Join-Path $ProjectRoot "dist"
    $artifactName = Get-DesktopReleaseArtifactName -Version $Version
    $releaseRoot = Join-Path $distRoot $artifactName
    $zipPath = Join-Path $distRoot "$artifactName.zip"
    $manifestPath = Join-Path $distRoot "nte-gacha-exporter-update.json"
    Clear-PortableStageBuildOwnedPaths -ProjectRoot $ProjectRoot -ReleaseRoot $releaseRoot -Version $Version
    if (-not (Test-Path -LiteralPath $distRoot)) {
        New-Item -ItemType Directory -Path $distRoot | Out-Null
    }

    $targetRelease = Join-Path $ProjectRoot "target\release"
    $launcher = Join-Path $targetRelease "nte-gacha-exporter.exe"
    $cli = Join-Path $targetRelease "nte-gacha-exporter-cli.exe"
    $desktopExe = Join-Path $targetRelease "nte-gacha-exporter-desktop.exe"
    $updater = Join-Path $targetRelease "nte-gacha-exporter-updater.exe"
    foreach ($path in @($launcher, $cli, $desktopExe, $updater)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Portable artifact missing: $path"
        }
    }

    $appDir = Join-Path $releaseRoot "app"
    New-Item -ItemType Directory -Force -Path $appDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $releaseRoot "data") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $releaseRoot "update\downloads") | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $releaseRoot "update\staging") | Out-Null

    Copy-Item -LiteralPath $launcher -Destination (Join-Path $releaseRoot "nte-gacha-exporter.exe")
    Copy-Item -LiteralPath $cli -Destination (Join-Path $releaseRoot "nte-gacha-exporter-cli.exe")
    Copy-Item -LiteralPath $desktopExe -Destination (Join-Path $appDir "nte-gacha-exporter-desktop.exe")
    Copy-Item -LiteralPath $updater -Destination (Join-Path $appDir "nte-gacha-exporter-updater.exe")
    New-ReleaseJson -Path (Join-Path $appDir "release.json") -Version $Version
    Expand-BundledAssetsPack -AssetsPackZip $AssetsPackZip -Destination (Join-Path $appDir "assets-pack\current")

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
    Write-WorkflowOutput -Name "manifestName" -Value "nte-gacha-exporter-update.json"
    Write-WorkflowOutput -Name "prerelease" -Value $IsPrerelease.ToString().ToLowerInvariant()
    return $releaseRoot
}

function Invoke-PortableRunSmoke {
    param(
        [string]$ReleaseRoot,
        [string]$Version
    )

    $cli = Join-Path $ReleaseRoot "nte-gacha-exporter-cli.exe"
    $actualVersion = Invoke-ExternalOutput -Name "Portable CLI version" -FilePath $cli -Arguments @("--version") -WorkingDirectory $ReleaseRoot
    if ($actualVersion -ne $Version) {
        throw "Portable CLI version mismatch: expected $Version, got $actualVersion"
    }
    Write-Ok "Portable CLI version -> $actualVersion"

    $launcher = Join-Path $ReleaseRoot "nte-gacha-exporter.exe"
    Write-Step "portable app launch smoke"
    $before = @(Get-Process -Name "nte-gacha-exporter-desktop" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
    $process = Start-Process -FilePath $launcher -WorkingDirectory $ReleaseRoot -PassThru
    Start-Sleep -Seconds 7
    if (-not $process.HasExited) {
        throw "root launcher should exit after spawning app"
    }
    $after = @(Get-Process -Name "nte-gacha-exporter-desktop" -ErrorAction SilentlyContinue |
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
