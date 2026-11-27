# OpenHUD installer for Windows.
#
#   irm https://openhud.vercel.app/install.ps1 | iex
#
# Downloads the prebuilt binary from GitHub Releases, checks its SHA-256, puts it in
# %LOCALAPPDATA%\openhud\bin, adds that to your PATH and starts the guided setup, which
# connects your AI agents. Source: https://github.com/ultron-hq/openhud
#
# Options (environment variables): OPENHUD_INSTALL_DIR, OPENHUD_VERSION (e.g. v0.1.0),
# OPENHUD_NO_SETUP=1.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$repo = 'ultron-hq/openhud'
$dir = if ($env:OPENHUD_INSTALL_DIR) { $env:OPENHUD_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'openhud\bin' }
$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne 'AMD64' -and $arch -ne 'ARM64') { throw "This CPU ($arch) isn't supported yet." }
# Windows on ARM runs the x64 build through its built-in emulation.
$target = 'x86_64-pc-windows-msvc'
$asset = "openhud-$target.zip"
$url = if ($env:OPENHUD_VERSION) { "https://github.com/$repo/releases/download/$($env:OPENHUD_VERSION)/$asset" }
       else { "https://github.com/$repo/releases/latest/download/$asset" }

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("openhud-" + [guid]::NewGuid())
New-Item -ItemType Directory $tmp | Out-Null
try {
    Write-Host "Installing OpenHUD ($target)"
    Invoke-WebRequest $url -OutFile "$tmp\$asset" -UseBasicParsing
    Invoke-WebRequest "$url.sha256" -OutFile "$tmp\$asset.sha256" -UseBasicParsing
    $expected = (Get-Content "$tmp\$asset.sha256" -Raw).Trim().Substring(0, 64).ToLower()
    $actual = (Get-FileHash "$tmp\$asset" -Algorithm SHA256).Hash.ToLower()
    if ($expected -ne $actual) { throw 'Checksum mismatch, the download may be damaged; try again.' }
    Expand-Archive "$tmp\$asset" -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Force $dir | Out-Null
    Copy-Item "$tmp\openhud.exe" $dir -Force
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
$bin = Join-Path $dir 'openhud.exe'
# Agents set up before the renames start clickhand.exe; keep that name working.
Copy-Item $bin (Join-Path $dir 'clickhand.exe') -Force -ErrorAction SilentlyContinue
Write-Host "Installed $(& $bin --version) to $bin"

# The saved user PATH (new terminals) and this window's PATH are fixed separately, so a
# reinstall also repairs a terminal that was opened before the first install.
function Test-OnPath([string]$list, [string]$d) {
    $want = $d.TrimEnd('\')
    foreach ($p in ($list -split ';')) { if ($p -and $p.TrimEnd('\') -ieq $want) { return $true } }
    return $false
}
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not $userPath) { $userPath = '' }
if (-not (Test-OnPath $userPath $dir)) {
    [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ";$dir").TrimStart(';'), 'User')
    Write-Host "Added $dir to your PATH (new terminals will see it)"
}
if (-not (Test-OnPath $env:Path $dir)) {
    $env:Path = ($env:Path.TrimEnd(';') + ";$dir").TrimStart(';')
}

if ($env:OPENHUD_NO_SETUP) {
    Write-Host 'Run `openhud` to connect your AI agents.'
} else {
    Write-Host ''
    & $bin setup
}
