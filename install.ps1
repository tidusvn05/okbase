# okbase installer for Windows: downloads a release binary from GitHub, checks it against the
# release's SHA256SUMS and installs it.
#
#   irm https://raw.githubusercontent.com/tidusvn05/okbase/main/install.ps1 | iex
#
# Options through environment variables: OKBASE_FULL=1 (okbase-full), OKBASE_VERSION=vX.Y.Z
# (default: the latest release), OKBASE_INSTALL_DIR (default: %LOCALAPPDATA%\okbase\bin).
$ErrorActionPreference = 'Stop'

$Repo = 'tidusvn05/okbase'
$Variant = if ($env:OKBASE_FULL -eq '1') { 'okbase-full' } else { 'okbase' }
$Version = if ($env:OKBASE_VERSION) { $env:OKBASE_VERSION } else { 'latest' }
$Dir = if ($env:OKBASE_INSTALL_DIR) { $env:OKBASE_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'okbase\bin' }

if ([Environment]::Is64BitOperatingSystem -ne $true -or $env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
    throw 'okbase-install: only x86_64 Windows builds are published'
}
$Target = 'x86_64-pc-windows-msvc'

if ($Version -eq 'latest') {
    $release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
    $Version = $release.tag_name
}
if (-not $Version.StartsWith('v')) { $Version = "v$Version" }

$Name = "$Variant-$Version-$Target"
$Base = "https://github.com/$Repo/releases/download/$Version"
$Tmp = Join-Path ([IO.Path]::GetTempPath()) ("okbase-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $Tmp | Out-Null
try {
    Write-Host "okbase-install: downloading $Name"
    Invoke-WebRequest "$Base/$Name.zip" -OutFile (Join-Path $Tmp "$Name.zip")
    Invoke-WebRequest "$Base/SHA256SUMS" -OutFile (Join-Path $Tmp 'SHA256SUMS')
    $line = Get-Content (Join-Path $Tmp 'SHA256SUMS') | Where-Object { $_ -match "\s\*?$([regex]::Escape("$Name.zip"))$" }
    if (-not $line) { throw "okbase-install: $Name.zip is not listed in SHA256SUMS" }
    $expected = ($line -split '\s+')[0].ToLower()
    $actual = (Get-FileHash (Join-Path $Tmp "$Name.zip") -Algorithm SHA256).Hash.ToLower()
    if ($expected -ne $actual) { throw "okbase-install: checksum mismatch for $Name.zip" }

    Expand-Archive (Join-Path $Tmp "$Name.zip") -DestinationPath $Tmp
    New-Item -ItemType Directory -Force -Path $Dir | Out-Null
    Copy-Item (Join-Path $Tmp "$Name\okbase.exe") (Join-Path $Dir 'okbase.exe') -Force
    Write-Host "okbase-install: installed $(& (Join-Path $Dir 'okbase.exe') --version) to $Dir (checksum verified)"

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (($userPath -split ';') -notcontains $Dir) {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$Dir", 'User')
        Write-Host "okbase-install: added $Dir to your user PATH (open a new terminal)"
    }
    Write-Host "okbase-install: next: run 'okbase onboard' in your knowledge folder (or ask your agent to set okbase up)"
}
finally {
    Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
}
