[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$runtimePath = Join-Path $projectRoot 'runtime\git'
$version = '2.55.0.5'
$asset = "PortableGit-$version-64-bit.7z.exe"
$url = "https://github.com/git-for-windows/git/releases/download/v2.55.0.windows.5/$asset"
$sha256 = '5aa8a20f6e9abb2c755f0e73c91c687701a46b309ad84a0ca6509380fa4ae290'
$cachePath = Join-Path $projectRoot '.cache'
$archivePath = Join-Path $cachePath $asset

if (Test-Path -LiteralPath $runtimePath) {
    $markerPath = Join-Path $runtimePath 'winshell-runtime.txt'
    if ((Test-Path -LiteralPath $markerPath) -and ((Get-Content -LiteralPath $markerPath -Raw).Trim() -eq $sha256) -and (Test-Path -LiteralPath (Join-Path $runtimePath 'bin\bash.exe'))) {
        Write-Output "Verified runtime already prepared: $runtimePath"
        return
    }
    throw "A runtime already exists at $runtimePath. Choose a fresh checkout to prepare the pinned version."
}
New-Item -ItemType Directory -Path $cachePath -Force | Out-Null
if (-not (Test-Path -LiteralPath $archivePath)) {
    Write-Output "Downloading official Git for Windows $version..."
    Invoke-WebRequest -Uri $url -OutFile $archivePath
}
$actualHash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $sha256) { throw "SHA256 mismatch for $archivePath. Expected $sha256, got $actualHash" }
$stagePath = Join-Path $cachePath ('git-stage-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stagePath | Out-Null
$sevenZip = Get-Command 7z.exe -ErrorAction SilentlyContinue
if (-not $sevenZip -and (Test-Path -LiteralPath 'C:\Program Files\7-Zip\7z.exe')) {
    $sevenZip = Get-Item -LiteralPath 'C:\Program Files\7-Zip\7z.exe'
}
if ($sevenZip) {
    $extractor = if ($sevenZip.Source) { $sevenZip.Source } else { $sevenZip.FullName }
    & $extractor x $archivePath "-o$stagePath" -y | Out-Null
    if ($LASTEXITCODE -ne 0) { throw '7-Zip extraction failed' }
} else {
    $process = Start-Process -FilePath $archivePath -ArgumentList @('-y', ('-o"' + $stagePath + '"')) -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "PortableGit extraction failed: $($process.ExitCode)" }
}
if (-not (Test-Path -LiteralPath (Join-Path $stagePath 'bin\bash.exe'))) { throw 'Extracted archive does not contain Bash' }
if (-not (Test-Path -LiteralPath (Join-Path $stagePath 'cmd\git.exe'))) { throw 'Extracted archive does not contain Git' }
Set-Content -LiteralPath (Join-Path $stagePath 'winshell-runtime.txt') -Value $sha256 -Encoding ascii
Set-Content -LiteralPath (Join-Path $stagePath 'winshell-upstream.txt') -Value @"
Git for Windows $version
Binary: $url
SHA256: $sha256
Source and component source references: https://github.com/git-for-windows/git/releases/tag/v2.55.0.windows.5
Keep all upstream license files and source references when redistributing this runtime.
"@ -Encoding utf8
New-Item -ItemType Directory -Path (Split-Path -Parent $runtimePath) -Force | Out-Null
# Verify both absolute targets before moving this task-created extraction directory.
$resolvedStage = [IO.Path]::GetFullPath($stagePath)
$resolvedRuntime = [IO.Path]::GetFullPath($runtimePath)
if (-not $resolvedStage.StartsWith($projectRoot + '\', [StringComparison]::OrdinalIgnoreCase) -or -not $resolvedRuntime.StartsWith($projectRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Runtime paths escaped the workspace' }
Move-Item -LiteralPath $resolvedStage -Destination $resolvedRuntime
Write-Output "Portable Bash/Git runtime ready: $runtimePath"

