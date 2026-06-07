# Vader installer for Windows (PowerShell).
#
#   irm https://raw.githubusercontent.com/MarcosSmeets/vader-langue/main/install.ps1 | iex
#
# Downloads a prebuilt vader.exe from GitHub Releases and adds it to your user PATH.
# Options (environment variables):
#   VADER_VERSION   release tag to install (default: latest), e.g. v0.6.0
#   VADER_BINDIR    install directory (default: %LOCALAPPDATA%\Vader\bin)

$ErrorActionPreference = 'Stop'

$repo    = 'MarcosSmeets/vader-langue'
$version = if ($env:VADER_VERSION) { $env:VADER_VERSION } else { 'latest' }
$bindir  = if ($env:VADER_BINDIR)  { $env:VADER_BINDIR }  else { "$env:LOCALAPPDATA\Vader\bin" }
$asset   = 'vader-windows-x86_64.exe'

$url = if ($version -eq 'latest') {
  "https://github.com/$repo/releases/latest/download/$asset"
} else {
  "https://github.com/$repo/releases/download/$version/$asset"
}

New-Item -ItemType Directory -Force -Path $bindir | Out-Null
$dest = Join-Path $bindir 'vader.exe'

Write-Host "vader-install: downloading $asset ($version)..."
Invoke-WebRequest -Uri $url -OutFile $dest

# Verify the checksum against the published <url>.sha256 (best-effort).
try {
  $expected = (((Invoke-WebRequest -Uri "$url.sha256" -UseBasicParsing).Content).Trim() -split '\s+')[0].ToLower()
} catch {
  $expected = $null
}
if ($expected) {
  $actual = (Get-FileHash -Algorithm SHA256 -Path $dest).Hash.ToLower()
  if ($expected -ne $actual) {
    Remove-Item $dest -Force
    throw "vader-install: checksum mismatch (expected $expected, got $actual)"
  }
  Write-Host "vader-install: checksum OK"
} elseif ($env:VADER_REQUIRE_CHECKSUM -eq '1') {
  Remove-Item $dest -Force
  throw "vader-install: no checksum published for this release"
} else {
  Write-Host "vader-install: checksum not published for this release - skipping verification"
}

# Add to the user PATH if it isn't there already.
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $bindir) {
  [Environment]::SetEnvironmentVariable('Path', "$bindir;$userPath", 'User')
  $env:Path = "$bindir;$env:Path"
  Write-Host "vader-install: added $bindir to your user PATH (restart terminals to pick it up)."
}

Write-Host "vader-install: installed $dest"
& $dest version

if (-not (Get-Command clang -ErrorAction SilentlyContinue)) {
  Write-Host "vader-install: note: install LLVM/clang for the native backend (vader llvm)."
}
Write-Host "vader-install: done. Try:  vader new api my-project"
