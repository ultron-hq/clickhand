# Clickhand installer for Windows.
#
#   irm https://clickhand.vercel.app/install.ps1 | iex
#
# Downloads the prebuilt binary from GitHub Releases, checks its SHA-256, puts it in
# %LOCALAPPDATA%\clickhand\bin, adds that to your PATH and starts the guided setup, which
# connects your AI agents. Source: https://github.com/ultron-hq/clickhand
#
# Options (environment variables): CLICKHAND_INSTALL_DIR, CLICKHAND_VERSION (e.g. v0.1.0),
# CLICKHAND_NO_SETUP=1.
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$repo = 'ultron-hq/clickhand'
$dir = if ($env:CLICKHAND_INSTALL_DIR) { $env:CLICKHAND_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'clickhand\bin' }
$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne 'AMD64' -and $arch -ne 'ARM64') { throw "This CPU ($arch) isn't supported yet." }
# Windows on ARM runs the x64 build through its built-in emulation.
$target = 'x86_64-pc-windows-msvc'
$asset = "clickhand-$target.zip"
$url = if ($env:CLICKHAND_VERSION) { "https://github.com/$repo/releases/download/$($env:CLICKHAND_VERSION)/$asset" }
       else { "https://github.com/$repo/releases/latest/download/$asset" }

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("clickhand-" + [guid]::NewGuid())
New-Item -ItemType Directory $tmp | Out-Null
try {
    Write-Host "Installing Clickhand ($target)"
    Invoke-WebRequest $url -OutFile "$tmp\$asset" -UseBasicParsing
    Invoke-WebRequest "$url.sha256" -OutFile "$tmp\$asset.sha256" -UseBasicParsing
    $expected = (Get-Content "$tmp\$asset.sha256" -Raw).Trim().Substring(0, 64).ToLower()
    $actual = (Get-FileHash "$tmp\$asset" -Algorithm SHA256).Hash.ToLower()
    if ($expected -ne $actual) { throw 'Checksum mismatch, the download may be damaged; try again.' }
    Expand-Archive "$tmp\$asset" -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Force $dir | Out-Null
    Copy-Item "$tmp\clickhand.exe" $dir -Force
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
$bin = Join-Path $dir 'clickhand.exe'
Write-Host "Installed $(& $bin --version) to $bin"

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (-not $userPath) { $userPath = '' }
if (($userPath -split ';') -notcontains $dir) {
    [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ";$dir").TrimStart(';'), 'User')
    $env:Path += ";$dir"
    Write-Host "Added $dir to your PATH (new terminals will see it)"
}

if ($env:CLICKHAND_NO_SETUP) {
    Write-Host 'Run `clickhand` to connect your AI agents.'
} else {
    Write-Host ''
    & $bin setup
}
