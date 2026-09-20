<#
.SYNOPSIS
    Install the Marslang interpreter on Windows.

.DESCRIPTION
    Downloads a released marslang.exe, verifies its checksum, puts it in a
    per-user directory, and adds that directory to the user's PATH. Nothing is
    compiled, so no Rust toolchain is needed, and nothing is written outside the
    user's profile, so no administrator rights are needed.

.EXAMPLE
    irm https://marslang.kevin-z.com/install.ps1 | iex

.EXAMPLE
    .\install.ps1 -Version rs-0.9.0 -Use std,ext
#>
[CmdletBinding()]
param(
    # Release tag to install, or "latest".
    [string] $Version = "latest",
    # Package sets to install: std is built into the interpreter; ext is the
    # extension collection, which is published separately.
    [string[]] $Use = @("std"),
    # Where the interpreter goes; its bin\ directory is added to PATH.
    [string] $InstallDir = "$env:LOCALAPPDATA\Programs\marslang",
    # Where packages you install live, importable from every program.
    [string] $PkgDir = "$env:USERPROFILE\marslang_pkgs",
    # Leave PATH alone.
    [switch] $NoPath
)

$ErrorActionPreference = "Stop"
$repo = "marslang-project/marslang"
$known = @("std", "ext")

function Step([string] $message) { Write-Host "==> $message" }

foreach ($set in $Use) {
    if ($known -notcontains $set) {
        throw "unknown package set '$set'; use any of: $($known -join ', ')"
    }
}

# TLS 1.2 is not the default in Windows PowerShell 5.1, and GitHub requires it.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$architecture = switch ($env:PROCESSOR_ARCHITECTURE) {
    "AMD64" { "x86_64-pc-windows-msvc" }
    "ARM64" { "aarch64-pc-windows-msvc" }
    default { throw "unsupported processor architecture '$env:PROCESSOR_ARCHITECTURE'" }
}

if ($Version -eq "latest") {
    Step "Looking up the latest release"
    try {
        $release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -Headers @{ "User-Agent" = "marslang-installer" }
        $Version = $release.tag_name
    } catch {
        throw "could not read the latest release of $repo ($($_.Exception.Message)). Pass -Version rs-X.Y.Z, or check that the repository is public."
    }
}

$archive = "marslang-$Version-$architecture.zip"
# A mirror or a local copy can be used instead of the GitHub release.
$base = if ($env:MARSLANG_DOWNLOAD_BASE) { $env:MARSLANG_DOWNLOAD_BASE } else { "https://github.com/$repo/releases/download/$Version" }
$staging = Join-Path ([IO.Path]::GetTempPath()) "marslang-install-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $staging | Out-Null

try {
    Step "Downloading $archive"
    $zip = Join-Path $staging $archive
    Invoke-WebRequest "$base/$archive" -OutFile $zip -UseBasicParsing

    Step "Verifying the checksum"
    $sums = Join-Path $staging "SHA256SUMS"
    Invoke-WebRequest "$base/SHA256SUMS" -OutFile $sums -UseBasicParsing
    $line = Get-Content $sums | Where-Object { $_ -match "\s\*?$([regex]::Escape($archive))$" } | Select-Object -First 1
    $expected = ($line -split '\s+') | Select-Object -First 1
    if (-not $expected) { throw "SHA256SUMS does not list $archive" }
    $actual = (Get-FileHash $zip -Algorithm SHA256).Hash
    if ($actual -ne $expected.ToUpper()) {
        throw "checksum mismatch for ${archive}: expected $expected, got $actual"
    }

    Step "Installing to $InstallDir"
    $bin = Join-Path $InstallDir "bin"
    New-Item -ItemType Directory -Path $bin -Force | Out-Null
    Expand-Archive -Path $zip -DestinationPath $staging -Force
    $exe = Get-ChildItem -Path $staging -Filter "marslang.exe" -Recurse | Select-Object -First 1
    if (-not $exe) { throw "$archive does not contain marslang.exe" }
    Copy-Item $exe.FullName (Join-Path $bin "marslang.exe") -Force

    New-Item -ItemType Directory -Path $PkgDir -Force | Out-Null
    if ($PkgDir -ne "$env:USERPROFILE\marslang_pkgs") {
        [Environment]::SetEnvironmentVariable("MARSLANG_PKGS", $PkgDir, "User")
        $env:MARSLANG_PKGS = $PkgDir
    }

    if ($Use -contains "ext") {
        Write-Warning "extension packages are not published yet; only the standard library was installed"
    }

    if (-not $NoPath) {
        $path = [Environment]::GetEnvironmentVariable("PATH", "User")
        if (($path -split ';') -notcontains $bin) {
            Step "Adding $bin to your PATH"
            [Environment]::SetEnvironmentVariable("PATH", "$path;$bin".Trim(';'), "User")
        }
        $env:PATH = "$env:PATH;$bin"
    }

    $installed = & (Join-Path $bin "marslang.exe") --version
    Write-Host ""
    Write-Host "$installed is installed." -ForegroundColor Green
    Write-Host "  interpreter: $(Join-Path $bin 'marslang.exe')"
    Write-Host "  packages:    $PkgDir"
    Write-Host ""
    Write-Host "Open a new terminal, then run:  marslang hello.mars"
    Write-Host "Documentation: https://marslang.kevin-z.com"
} finally {
    Remove-Item $staging -Recurse -Force -ErrorAction SilentlyContinue
}
