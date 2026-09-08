[CmdletBinding()]
param([string]$Executable = (Join-Path $PSScriptRoot '..\target\debug\winshell.exe'), [switch]$UI)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$outputDir = Join-Path ([IO.Path]::GetTempPath()) ("winshell-validation-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $outputDir | Out-Null
foreach ($shellId in @('bash', 'powershell', 'cmd')) {
    $out = Join-Path $outputDir "$shellId.stdout"
    $err = Join-Path $outputDir "$shellId.stderr"
    $process = Start-Process -FilePath $Executable -ArgumentList '--smoke-test', $shellId -PassThru -WindowStyle Hidden -RedirectStandardOutput $out -RedirectStandardError $err
    if (-not $process.WaitForExit(90000)) { throw "Validation timed out: $shellId (PID $($process.Id))" }
    Get-Content -LiteralPath $out
    if ($process.ExitCode -ne 0 -or (Get-Content -LiteralPath $out -Raw) -notmatch "PASS $shellId") {
        throw "Validation failed for $shellId`: $(Get-Content -LiteralPath $err -Raw)"
    }
}
if ($UI) {
    $report = Join-Path $outputDir 'native-ui.txt'
    $process = Start-Process -FilePath $Executable -ArgumentList '--ui-smoke-test', "`"$report`"" -PassThru -WindowStyle Hidden
    if (-not $process.WaitForExit(120000)) { throw "Native UI validation timed out (PID $($process.Id))" }
    $result = Get-Content -LiteralPath $report -Raw
    if ($process.ExitCode -ne 0 -or $result -notlike 'PASS*') { throw $result }
    Write-Output $result
}
Write-Output "Validation reports: $outputDir"
