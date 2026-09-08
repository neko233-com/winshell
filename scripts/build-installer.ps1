[CmdletBinding()]
param([string]$Compiler)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $Compiler) {
    $candidate = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($candidate) { $Compiler = $candidate.Source }
    else {
        foreach ($path in @("${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe", "$env:ProgramFiles\Inno Setup 7\ISCC.exe")) {
            if (Test-Path -LiteralPath $path) { $Compiler = $path; break }
        }
    }
}
if (-not $Compiler) { throw 'Inno Setup compiler is required. GitHub Windows CI provides it.' }
$version = (Select-String -LiteralPath "$projectRoot\Cargo.toml" -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$package = Join-Path $projectRoot "dist\winshell-$version-windows-x64-portable"
if (-not (Test-Path -LiteralPath "$package\runtime\git\bin\bash.exe")) { throw 'Prepare the standalone portable package first.' }
& $Compiler "/DAppVersion=$version" "/DPackageDir=$package" "$projectRoot\installer\winshell.iss"
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }
$setup = Join-Path $projectRoot "dist\winshell-$version-windows-x64-setup.exe"
$hash = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -LiteralPath "$setup.sha256" -Value "$hash  $([IO.Path]::GetFileName($setup))" -Encoding ascii
