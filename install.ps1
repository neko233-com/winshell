[CmdletBinding()]
param([string]$Version = 'latest', [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\WinShell'))
$ErrorActionPreference = 'Stop'
if (-not [Environment]::Is64BitOperatingSystem) { throw 'WinShell requires 64-bit Windows 10 1809 or newer.' }
if ($Version -eq 'latest') {
    $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/neko233-com/winshell/releases/latest'
    $Version = $release.tag_name
}
$Version = $Version -replace '^v', ''
if ($Version -notmatch '^\d+\.\d+\.\d+([.-][a-zA-Z0-9.-]+)?$') { throw "Invalid release version: $Version" }
$asset = "winshell-$Version-windows-x64-setup.exe"
$base = "https://github.com/neko233-com/winshell/releases/download/v$Version"
$downloadDir = Join-Path ([IO.Path]::GetTempPath()) ("winshell-install-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $downloadDir | Out-Null
try {
    Write-Host "Downloading WinShell $Version..."
    $setup = Join-Path $downloadDir $asset
    Invoke-WebRequest -Uri "$base/$asset" -OutFile $setup -UseBasicParsing
    $checksum = (Invoke-WebRequest -Uri "$base/$asset.sha256" -UseBasicParsing).Content
    if ($checksum -is [byte[]]) { $checksum = [Text.Encoding]::UTF8.GetString($checksum) }
    $expected = ($checksum.Trim() -split '\s+')[0].ToLowerInvariant()
    if ($expected -notmatch '^[a-f0-9]{64}$') { throw 'Invalid release checksum.' }
    $actual = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { throw 'SHA256 verification failed. Installation stopped.' }
    $InstallDir = [IO.Path]::GetFullPath($InstallDir)
    if ($InstallDir.Contains('"')) { throw 'Invalid installation directory.' }
    $setupProcess = Start-Process -FilePath $setup -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DIR=`"$InstallDir`"" -PassThru -Wait -WindowStyle Hidden
    if ($setupProcess.ExitCode -ne 0) { throw "Installer exited with code $($setupProcess.ExitCode)." }
    if (-not (Test-Path -LiteralPath (Join-Path $InstallDir 'winshell.exe'))) { throw 'Installed executable was not found.' }
    Write-Host "WinShell $Version installed: $InstallDir"
    Write-Host 'Open WinShell from the Start menu. Re-run this command to upgrade; settings are preserved.'
} finally {
    $resolvedDownload = [IO.Path]::GetFullPath($downloadDir)
    $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if ($resolvedDownload.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and [IO.Path]::GetFileName($resolvedDownload).StartsWith('winshell-install-')) {
        Remove-Item -LiteralPath $resolvedDownload -Recurse -Force
    }
}
