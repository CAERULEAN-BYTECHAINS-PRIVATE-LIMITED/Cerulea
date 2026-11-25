#Requires -Version 5.1
<#
.SYNOPSIS
    Windows setup script for the Cerulea build environment via WSL2 (Ubuntu).

.DESCRIPTION
    Mirrors scripts/setup-build-env.sh for Windows:
      1. Checks whether WSL2 and an Ubuntu WSL distribution are installed and initialized.
      2. If WSL or Ubuntu is missing or incomplete, attempts to install and configure it first
         (requesting elevation if needed, and handling reboot-required states cleanly).
      3. Runs scripts/setup-build-env.sh inside the Ubuntu WSL distribution against this
         repository (safely handling Windows CRLF line endings without modifying repo files).

.PARAMETER Distro
    Optional WSL distribution name to use (e.g. "Ubuntu-24.04"). If omitted, automatically
    selects an installed Ubuntu distribution or installs "Ubuntu".

.PARAMETER Build
    If specified, also runs scripts/build-node-local.sh inside WSL after environment setup.

.PARAMETER AsRoot
    Runs the WSL setup/build commands as the Linux 'root' user (skips sudo password prompts).

.PARAMETER SkipElevation
    Internal switch used when relaunching elevated to avoid elevation loops.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .\scripts\setup-build-env.ps1
    powershell -ExecutionPolicy Bypass -File .\scripts\setup-build-env.ps1 -Build
    powershell -ExecutionPolicy Bypass -File .\scripts\setup-build-env.ps1 -AsRoot
#>

[CmdletBinding()]
param(
    [string]$Distro = "",
    [switch]$Build,
    [switch]$AsRoot,
    [switch]$SkipElevation,
    [switch]$PauseOnExit
)

$ErrorActionPreference = "Stop"

# Force wsl.exe to emit UTF-8 instead of UTF-16LE so string parsing is reliable
$env:WSL_UTF8 = "1"

function Exit-Script {
    param([int]$Code = 0)
    if ($PauseOnExit) {
        Write-Host ""
        Read-Host "Press Enter to close this window" | Out-Null
    }
    exit $Code
}

function Write-Header {
    param([string]$Text)
    Write-Host "========================================" -ForegroundColor Blue
    Write-Host "  $Text" -ForegroundColor Blue
    Write-Host "========================================" -ForegroundColor Blue
    Write-Host ""
}

function Write-Step {
    param([string]$Text)
    Write-Host $Text -ForegroundColor Yellow
}

function Write-Ok {
    param([string]$Text)
    Write-Host "[OK] $Text" -ForegroundColor Green
}

function Write-Warn {
    param([string]$Text)
    Write-Host "[WARN] $Text" -ForegroundColor Yellow
}

function Write-Err {
    param([string]$Text)
    Write-Host "[ERROR] $Text" -ForegroundColor Red
}

function Test-IsAdmin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Invoke-ElevateSelf {
    if ($SkipElevation) {
        Write-Err "Administrator privileges are required for this WSL operation, but elevation was skipped or declined."
        Exit-Script 1
    }

    Write-Step "Administrator privileges are required for this WSL operation. Requesting elevation..."
    Write-Host "Check for a Windows UAC prompt. Progress will continue in the elevated PowerShell window." -ForegroundColor Yellow
    $argList = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", "`"$PSCommandPath`"",
        "-SkipElevation",
        "-PauseOnExit"
    )
    if ($Distro) {
        $argList += @("-Distro", "`"$Distro`"")
    }
    if ($Build) {
        $argList += "-Build"
    }
    if ($AsRoot) {
        $argList += "-AsRoot"
    }

    try {
        $proc = Start-Process -FilePath "powershell.exe" -ArgumentList $argList -Verb RunAs -Wait -PassThru
        exit $proc.ExitCode
    }
    catch {
        Write-Err "Failed to elevate PowerShell: $($_.Exception.Message)"
        Write-Host "Please open PowerShell as Administrator and re-run:" -ForegroundColor Yellow
        Write-Host "  powershell -ExecutionPolicy Bypass -File `"$PSCommandPath`""
        Exit-Script 1
    }
}

function Test-PendingReboot {
    $rebootKeys = @(
        "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Component Based Servicing\RebootPending",
        "HKLM:\SOFTWARE\Microsoft\Windows\Windows Update\Auto Update\RebootRequired"
    )
    foreach ($key in $rebootKeys) {
        if (Test-Path $key) {
            return $true
        }
    }
    return $false
}

function Invoke-WslRaw {
    param([string[]]$Arguments)
    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & wsl.exe @Arguments 2>&1
        $code = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $prevPref
    }
    $text = ($output | ForEach-Object { $_.ToString() }) -join "`n"
    $text = $text -replace "`0", ""
    return [PSCustomObject]@{
        ExitCode = $code
        Output   = $text.Trim()
    }
}

function Get-UsableWslDistros {
    $res = Invoke-WslRaw -Arguments @("--list", "--quiet")
    if ($res.ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($res.Output)) {
        return @()
    }

    $excluded = @("docker-desktop", "docker-desktop-data")
    $distros = @()
    foreach ($line in ($res.Output -split "\r?\n")) {
        $name = ($line -replace "`0", "").Trim()
        if ([string]::IsNullOrWhiteSpace($name)) { continue }
        if ($excluded -contains $name) { continue }
        if ($name -like "podman-machine*") { continue }
        $distros += $name
    }
    return $distros
}

function Test-WslCoreReady {
    $wslCmd = Get-Command wsl.exe -ErrorAction SilentlyContinue
    if (-not $wslCmd) {
        return $false
    }

    $status = Invoke-WslRaw -Arguments @("--status")
    if ($status.Output -match "not installed|restart|reboot") {
        return $false
    }
    if ($status.ExitCode -eq 0) {
        return $true
    }

    if ($status.Output -match "no installed distributions") {
        return $true
    }

    return $false
}

function Enable-WslCore {
    if (-not (Test-IsAdmin)) {
        Invoke-ElevateSelf
    }

    Write-Step "Enabling WSL2 and Virtual Machine Platform (live output below; this may take a few minutes)..."
    $needsReboot = $false

    Write-Host "-> Running: wsl.exe --install --no-distribution" -ForegroundColor DarkGray
    & wsl.exe --install --no-distribution
    $wslInstallCode = $LASTEXITCODE

    if ($wslInstallCode -eq 3010) {
        $needsReboot = $true
    }
    elseif ($wslInstallCode -ne 0) {
        Write-Warn "wsl --install --no-distribution exited with code $wslInstallCode. Enabling features via DISM..."

        Write-Host "-> Enabling Microsoft-Windows-Subsystem-Linux..." -ForegroundColor DarkGray
        & dism.exe /online /enable-feature /featurename:Microsoft-Windows-Subsystem-Linux /all /norestart
        $code1 = $LASTEXITCODE

        Write-Host "-> Enabling VirtualMachinePlatform..." -ForegroundColor DarkGray
        & dism.exe /online /enable-feature /featurename:VirtualMachinePlatform /all /norestart
        $code2 = $LASTEXITCODE

        if ($code1 -eq 3010 -or $code2 -eq 3010) {
            $needsReboot = $true
        }
        elseif (($code1 -ne 0 -and $code1 -ne 3010) -or ($code2 -ne 0 -and $code2 -ne 3010)) {
            Write-Err "Failed to enable WSL Windows features (DISM codes: $code1, $code2)."
            Exit-Script 1
        }
    }

    if ($needsReboot -or (-not (Test-WslCoreReady) -and (Test-PendingReboot))) {
        Write-Host ""
        Write-Warn "A Windows restart is required to finish enabling WSL2."
        Write-Host "Please reboot your computer and re-run:" -ForegroundColor Yellow
        Write-Host "  powershell -ExecutionPolicy Bypass -File `"$PSCommandPath`""
        Exit-Script 2
    }

    Write-Step "Updating WSL kernel and setting default version to 2..."
    & wsl.exe --update
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "wsl --update returned exit code $LASTEXITCODE; trying --web-download..."
        & wsl.exe --update --web-download
    }

    $verRes = Invoke-WslRaw -Arguments @("--set-default-version", "2")
    if ($verRes.ExitCode -ne 0) {
        Write-Warn "Could not set default WSL version to 2: $($verRes.Output)"
        if (-not (Test-WslCoreReady)) {
            Write-Warn "WSL2 still reports not ready; a Windows restart may be required."
            Write-Host "Please reboot your computer and re-run:" -ForegroundColor Yellow
            Write-Host "  powershell -ExecutionPolicy Bypass -File `"$PSCommandPath`""
            Exit-Script 2
        }
    }

    Write-Ok "WSL2 core is enabled"
}

function Select-TargetDistro {
    param([string[]]$InstalledDistros)

    if ($Distro) {
        if ($InstalledDistros -contains $Distro) {
            return $Distro
        }
        return $null
    }

    $preferred = @("Ubuntu", "Ubuntu-24.04", "Ubuntu-22.04", "Ubuntu-20.04")
    foreach ($candidate in $preferred) {
        if ($InstalledDistros -contains $candidate) {
            return $candidate
        }
    }

    $anyUbuntu = $InstalledDistros | Where-Object { $_ -like "Ubuntu*" } | Select-Object -First 1
    if ($anyUbuntu) {
        return $anyUbuntu
    }

    return $null
}

function Install-WslDistro {
    param([string]$Name)

    Write-Step "Installing WSL distribution '$Name' (this downloads ~500MB and may take a few minutes)..."

    $prevPref = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & wsl.exe --install -d $Name 2>&1 | Tee-Object -Variable installRaw
        $code = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $prevPref
    }
    $installText = (($installRaw | ForEach-Object { $_.ToString() }) -join "`n") -replace "`0", ""

    if ($installText -match "reboot|restart") {
        Write-Host ""
        Write-Warn "A Windows restart is required before '$Name' can complete installation."
        Write-Host "Please reboot your computer and re-run:" -ForegroundColor Yellow
        Write-Host "  powershell -ExecutionPolicy Bypass -File `"$PSCommandPath`""
        Exit-Script 2
    }

    if ($installText -match "requires elevation" -and -not (Test-IsAdmin)) {
        Invoke-ElevateSelf
    }

    $afterDistros = Get-UsableWslDistros
    $found = Select-TargetDistro -InstalledDistros $afterDistros

    if (-not $found) {
        Write-Warn "Distribution '$Name' is not registered yet (exit code $code). Retrying with --web-download..."
        $prevPref = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            & wsl.exe --install -d $Name --web-download 2>&1 | Tee-Object -Variable webRaw
            $code = $LASTEXITCODE
        }
        finally {
            $ErrorActionPreference = $prevPref
        }
        $webText = (($webRaw | ForEach-Object { $_.ToString() }) -join "`n") -replace "`0", ""

        if ($webText -match "requires elevation" -and -not (Test-IsAdmin)) {
            Invoke-ElevateSelf
        }
        if ($webText -match "reboot|restart" -or (Test-PendingReboot)) {
            Write-Host ""
            Write-Warn "A Windows restart is required before '$Name' can complete installation."
            Write-Host "Please reboot your computer and re-run:" -ForegroundColor Yellow
            Write-Host "  powershell -ExecutionPolicy Bypass -File `"$PSCommandPath`""
            Exit-Script 2
        }

        $afterDistros = Get-UsableWslDistros
        $found = Select-TargetDistro -InstalledDistros $afterDistros
    }

    if (-not $found) {
        Write-Err "WSL distribution '$Name' was not installed (exit code $code)."
        if (-not (Test-IsAdmin)) {
            Write-Host "Try running this script from an Administrator PowerShell window, or reboot Windows first if WSL was just enabled." -ForegroundColor Yellow
        }
        Exit-Script $(if ($code -ne 0) { $code } else { 1 })
    }
}

function Test-DistroInitialized {
    param([string]$Name)
    $probe = Invoke-WslRaw -Arguments @("-d", $Name, "--", "bash", "-lc", "id -un >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1 && echo CERULEA_WSL_READY")
    return ($probe.ExitCode -eq 0 -and $probe.Output -match "CERULEA_WSL_READY")
}

function Initialize-WslDistro {
    param([string]$Name)

    if (Test-DistroInitialized -Name $Name) {
        return
    }

    Write-Warn "WSL distribution '$Name' has not completed first-time setup (UNIX user creation)."
    Write-Step "Launching '$Name' so first-time setup can complete. If prompted, enter a UNIX username and password, then type 'exit' when you reach the shell prompt."

    & wsl.exe -d $Name
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "Initial launch returned exit code $LASTEXITCODE; verifying whether '$Name' is now ready..."
    }

    if (-not (Test-DistroInitialized -Name $Name)) {
        Write-Err "WSL distribution '$Name' is still not ready (missing bash/apt-get or incomplete user setup)."
        Write-Host "Try launching '$Name' manually from the Start Menu to finish setup, then re-run this script." -ForegroundColor Yellow
        Exit-Script 1
    }
}

function Convert-ToWslPath {
    param(
        [string]$DistroName,
        [string]$WindowsPath
    )

    $normalizedWin = $WindowsPath -replace "\\", "/"
    $res = Invoke-WslRaw -Arguments @("-d", $DistroName, "--", "wslpath", "-a", "-u", $normalizedWin)
    if ($res.ExitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($res.Output)) {
        return ($res.Output -split "\r?\n")[0].Trim()
    }

    if ($WindowsPath -match "^([A-Za-z]):\\(.*)$") {
        $drive = $Matches[1].ToLowerInvariant()
        $rest = $Matches[2] -replace "\\", "/"
        return "/mnt/$drive/$rest"
    }

    throw "Unable to translate Windows path '$WindowsPath' to a WSL path."
}

function Invoke-RepoScriptInWsl {
    param(
        [string]$DistroName,
        [string]$WslRepoPath,
        [string]$RelativeScriptPath,
        [switch]$RunAsRoot
    )

    $escapedRepo = $WslRepoPath.Replace("'", "'`"'`"'")
    $escapedScript = $RelativeScriptPath.Replace("'", "'`"'`"'")
    $tmpFile = "/tmp/cerulea-$([guid]::NewGuid().ToString('N')).sh"

    # Strip CRLF into a temporary script in /tmp so bash never fails on \r
    # while keeping stdin attached to the terminal for sudo/read prompts.
    # Avoid '$', double quotes, and newlines so PowerShell and wsl.exe pass the command cleanly.
    $bashCmd = "set -e; if [ -f ~/.cargo/env ]; then . ~/.cargo/env; fi; cd '$escapedRepo'; trap 'rm -f $tmpFile' EXIT; tr -d '\r' < '$escapedScript' > '$tmpFile'; chmod +x '$tmpFile'; bash '$tmpFile'"

    # Do not capture/assign output from this function so wsl.exe stdout/stdin stay attached
    # to the console and $LASTEXITCODE propagates cleanly as an int.
    if ($RunAsRoot) {
        & wsl.exe -d $DistroName -u root -e bash -lc $bashCmd
    }
    else {
        & wsl.exe -d $DistroName -e bash -lc $bashCmd
    }
}

# ── Main ───────────────────────────────────────────────────────────────────────
Write-Header "Cerulea Windows Build Setup (WSL2)"

$RepoRoot = Split-Path -Parent $PSScriptRoot
$SetupScript = Join-Path $RepoRoot "scripts\setup-build-env.sh"
$CargoConfig = Join-Path $RepoRoot ".cargo\config.toml"

if (-not (Test-Path $SetupScript)) {
    Write-Err "Could not find $SetupScript"
    exit 1
}
if (-not (Test-Path $CargoConfig)) {
    Write-Err "Could not find $CargoConfig"
    exit 1
}

# 1. Check and set up WSL core
Write-Step "[1/4] Checking WSL2 installation..."
if (-not (Test-WslCoreReady)) {
    Write-Warn "WSL2 is not installed or not fully configured."
    Enable-WslCore
}
else {
    Write-Ok "WSL2 is installed"
    $null = Invoke-WslRaw -Arguments @("--set-default-version", "2")
}

# 2. Check and set up Ubuntu WSL distribution
Write-Step "[2/4] Checking Ubuntu WSL distribution..."
$installedDistros = Get-UsableWslDistros
$targetDistro = Select-TargetDistro -InstalledDistros $installedDistros

if (-not $targetDistro) {
    $distroToInstall = if ($Distro) { $Distro } else { "Ubuntu" }
    Write-Warn "Target WSL distribution '$distroToInstall' is not installed."
    Install-WslDistro -Name $distroToInstall

    $installedDistros = Get-UsableWslDistros
    $targetDistro = Select-TargetDistro -InstalledDistros $installedDistros
    if (-not $targetDistro) {
        $targetDistro = $distroToInstall
    }
}

Initialize-WslDistro -Name $targetDistro
Write-Ok "Using WSL distribution: $targetDistro"

# 3. Resolve repository path inside WSL
Write-Step "[3/4] Mapping repository path to WSL..."
$wslRepoPath = Convert-ToWslPath -DistroName $targetDistro -WindowsPath $RepoRoot
Write-Ok "WSL repository path: $wslRepoPath"

# 4. Run Ubuntu setup-build-env.sh inside WSL
$userNote = if ($AsRoot) { "$targetDistro as root" } else { $targetDistro }
Write-Step "[4/4] Running scripts/setup-build-env.sh inside WSL ($userNote)..."
Write-Host ""
Invoke-RepoScriptInWsl -DistroName $targetDistro -WslRepoPath $wslRepoPath -RelativeScriptPath "scripts/setup-build-env.sh" -RunAsRoot:$AsRoot
$setupExit = [int]$LASTEXITCODE
if ($setupExit -ne 0) {
    Write-Err "scripts/setup-build-env.sh failed inside WSL with exit code $setupExit."
    if (-not $AsRoot) {
        Write-Host ""
        Write-Host "If this failed due to a forgotten WSL sudo password, you can either:" -ForegroundColor Yellow
        Write-Host "  1. Reset your WSL user password (no old password needed):" -ForegroundColor Yellow
        Write-Host "       wsl -d $targetDistro -u root passwd `$(wsl -d $targetDistro whoami)"
        Write-Host "  2. Or run setup directly as root (skips sudo password prompts):" -ForegroundColor Yellow
        Write-Host "       powershell -ExecutionPolicy Bypass -File .\scripts\setup-build-env.ps1 -AsRoot"
    }
    Exit-Script $setupExit
}

if ($Build) {
    Write-Host ""
    Write-Step "Running scripts/build-node-local.sh inside WSL ($userNote)..."
    Invoke-RepoScriptInWsl -DistroName $targetDistro -WslRepoPath $wslRepoPath -RelativeScriptPath "scripts/build-node-local.sh" -RunAsRoot:$AsRoot
    $buildExit = [int]$LASTEXITCODE
    if ($buildExit -ne 0) {
        Write-Err "scripts/build-node-local.sh failed inside WSL with exit code $buildExit."
        Exit-Script $buildExit
    }
}
else {
    $rootFlag = if ($AsRoot) { " -AsRoot" } else { "" }
    $wslUserArg = if ($AsRoot) { " -u root" } else { "" }
    Write-Host ""
    Write-Ok "Windows + WSL2 build environment is ready!"
    Write-Host "To build the node inside WSL now, run:" -ForegroundColor Yellow
    Write-Host "  powershell -ExecutionPolicy Bypass -File .\scripts\setup-build-env.ps1$rootFlag -Build"
    Write-Host "Or open WSL manually and run:" -ForegroundColor Yellow
    Write-Host "  wsl -d $targetDistro$wslUserArg -- bash -lc `"cd '$wslRepoPath' && sed -i 's/\r$//' scripts/build-node-local.sh && ./scripts/build-node-local.sh`""
}

Exit-Script 0
