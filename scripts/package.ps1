[CmdletBinding()]
param([switch]$SkipBuild, [switch]$WithoutRuntime)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $projectRoot
try {
    if (-not $WithoutRuntime) { & (Join-Path $PSScriptRoot 'setup-runtime.ps1') }
    if (-not $SkipBuild) {
        cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed' }
    }
    $versionLine = Select-String -LiteralPath (Join-Path $projectRoot 'Cargo.toml') -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    $version = $versionLine.Matches[0].Groups[1].Value
    $flavor = if ($WithoutRuntime) { 'thin' } else { 'portable' }
    $name = "winshell-$version-windows-x64-$flavor"
    $destination = Join-Path $projectRoot "dist\$name"
    if (Test-Path -LiteralPath $destination) { throw "Package already exists: $destination. Preserve it or choose a fresh checkout." }
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    python (Join-Path $PSScriptRoot 'collect-rust-licenses.py') $destination
    if ($LASTEXITCODE -ne 0) { throw 'Dependency notice collection failed' }
    Copy-Item -LiteralPath (Join-Path $projectRoot 'target\release\winshell.exe') -Destination $destination
    foreach ($file in @('README.md', 'README.zh-CN.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'config.example.toml')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot $file) -Destination $destination
    }
    if (-not $WithoutRuntime) {
        Copy-Item -LiteralPath (Join-Path $projectRoot 'docs\runtime-sources.json') -Destination $destination
        Copy-Item -LiteralPath (Join-Path $projectRoot 'docs\runtime-package-versions.txt') -Destination $destination
        New-Item -ItemType Directory -Path (Join-Path $destination 'runtime') | Out-Null
        Copy-Item -LiteralPath (Join-Path $projectRoot 'runtime\git') -Destination (Join-Path $destination 'runtime\git') -Recurse
    }
    $zip = "$destination.zip"
    tar.exe -a -c -f $zip -C (Split-Path -Parent $destination) $name
    if ($LASTEXITCODE -ne 0) { throw 'ZIP packaging failed' }
    $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$zip.sha256" -Value "$hash  $name.zip" -Encoding ascii
    Write-Output "Created $zip"
} finally { Pop-Location }
