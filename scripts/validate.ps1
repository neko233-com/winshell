[CmdletBinding()]
param([string]$Executable = (Join-Path $PSScriptRoot '..\target\debug\winshell.exe'), [switch]$UI)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$outputDir = Join-Path ([IO.Path]::GetTempPath()) ("winshell-validation-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $outputDir | Out-Null
foreach ($validationShell in @('bash', 'powershell', 'cmd')) {
    $out = Join-Path $outputDir "$validationShell.stdout"
    $err = Join-Path $outputDir "$validationShell.stderr"
    $process = Start-Process -FilePath $Executable -ArgumentList '--smoke-test', $validationShell -PassThru -WindowStyle Hidden -RedirectStandardOutput $out -RedirectStandardError $err
    if (-not $process.WaitForExit(90000)) { throw "Validation timed out: $validationShell (PID $($process.Id))" }
    Get-Content -LiteralPath $out
    if ($process.ExitCode -ne 0 -or (Get-Content -LiteralPath $out -Raw) -notmatch "PASS $validationShell") {
        throw "Validation failed for $validationShell`: $(Get-Content -LiteralPath $err -Raw)"
    }
}
if ($UI) {
    $report = Join-Path $outputDir 'native-ui.txt'
    # Native window tests need an actual visible window and committed input frame.
    $process = Start-Process -FilePath $Executable -ArgumentList '--ui-smoke-test', "`"$report`"" -PassThru
    if (-not $process.WaitForExit(120000)) { throw "Native UI validation timed out (PID $($process.Id))" }
    $result = Get-Content -LiteralPath $report -Raw
    if ($process.ExitCode -ne 0 -or $result -notlike 'PASS*') {
        $terminalReport = [IO.Path]::ChangeExtension($report, 'terminal.txt')
        if (Test-Path -LiteralPath $terminalReport) { Get-Content -LiteralPath $terminalReport }
        throw $result
    }
    Write-Output $result
}
Write-Output "Validation reports: $outputDir"
