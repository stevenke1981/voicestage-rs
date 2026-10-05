param([string]$Profile = 'release')
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $repo "target\$Profile\voicestage.exe"
$out = Join-Path $repo "test-output\benchmark-$Profile"
New-Item -ItemType Directory -Path $out -Force | Out-Null
$core = Join-Path $out 'core.json'
$p = Start-Process -FilePath $exe -ArgumentList @('--benchmark-core', $core) -WindowStyle Hidden -PassThru
if (-not $p.WaitForExit(30000)) { throw 'Core benchmark timed out' }
$runs = @()
for ($i = 0; $i -lt 5; $i++) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $exe -ArgumentList @('--lang', 'en') -WindowStyle Hidden -PassThru
    if (-not $p.WaitForInputIdle(10000)) { throw 'GUI startup timed out' }
    $ready = $clock.Elapsed.TotalMilliseconds
    Start-Sleep -Milliseconds 1500
    $p.Refresh()
    $runs += [pscustomobject]@{ startup_ms = [Math]::Round($ready, 2); working_set_bytes = $p.WorkingSet64; private_bytes = $p.PrivateMemorySize64 }
    $null = $p.CloseMainWindow()
    if (-not $p.WaitForExit(10000)) { throw 'GUI did not close normally' }
}
[pscustomobject]@{ profile=$Profile; exe_bytes=(Get-Item $exe).Length; method='New process; WaitForInputIdle; memory after 1.5s; OS file cache NOT flushed; shared host'; runs=$runs; core=(Get-Content $core -Raw | ConvertFrom-Json) } | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $out 'result.json') -Encoding UTF8
Get-Content (Join-Path $out 'result.json')
