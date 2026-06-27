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

function Get-DesktopReleaseArtifactName {
    param([string]$Version)

    return "nte-gacha-exporter-$Version"
}

function Assert-WindowsHost {
    $isWindowsHost = [System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT
    if (-not $isWindowsHost) {
        throw "Windows release packaging must run on Windows."
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

function Invoke-ExternalOutput {
    param(
        [string]$Name,
        [string]$FilePath,
        [string[]]$Arguments = @(),
        [string]$WorkingDirectory = ""
    )

    Write-Step $Name
    if ([string]::IsNullOrWhiteSpace($WorkingDirectory)) {
        $output = (& $FilePath @Arguments) -join "`n"
    }
    else {
        Push-Location $WorkingDirectory
        try {
            $output = (& $FilePath @Arguments) -join "`n"
        }
        finally {
            Pop-Location
        }
    }
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
    return $output.Trim()
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
        schema = "nte-gacha-exporter-release"
        schema_version = 1
        version = $Version
    }
    Write-JsonNoBom -Path $Path -Value $payload
}
