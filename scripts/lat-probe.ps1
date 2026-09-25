param(
    [Parameter(Mandatory)] [string]$Exe,
    [int]$Words = 40,
    [int]$Hogs = 0,
    [string]$Label = "run",
    # psmux namespace (-L). Never the default one; generated if omitted.
    [Alias('L')] [string]$Namespace = "lat-probe-$PID-$(Get-Random)"
)
# Keystroke->pushed-frame latency probe. Acts as a persistent psmux client over
# TCP (AUTH/PERSISTENT/client-attach), sends one char at a time via send-text,
# and times until a pushed frame contains the typed prefix. Runs ONLY in a
# unique -L namespace; snapshots default `ls` before/after.
# Usage: pwsh -File scripts/lat-probe.ps1 -Exe <path-to-psmux.exe> [-Hogs N] [-Label name] [-L ns]
# Run once with a baseline exe and once with a new build to compare latencies.
if ([string]::IsNullOrWhiteSpace($Namespace) -or $Namespace -eq 'default') {
    throw "Refusing to run: -Namespace must be a unique non-default psmux namespace"
}
$ErrorActionPreference = 'Stop'
$ns = $Namespace
$sess = "p"
$before = (& $Exe ls 2>&1 | Out-String)
$hogProcs = @()
try {
    & $Exe -L $ns new-session -d -s $sess -x 120 -y 30 "pwsh -NoLogo -NoProfile" | Out-Null
    $base = "${ns}__${sess}"
    $dir = Join-Path $env:USERPROFILE ".psmux"
    $portFile = Join-Path $dir "$base.port"; $keyFile = Join-Path $dir "$base.key"
    for ($i = 0; $i -lt 100 -and -not ((Test-Path $portFile) -and (Test-Path $keyFile)); $i++) { Start-Sleep -Milliseconds 50 }
    $port = [int](Get-Content $portFile -Raw).Trim(); $key = (Get-Content $keyFile -Raw).Trim()
    Start-Sleep -Seconds 3   # let pwsh reach its prompt

    $tcp = [System.Net.Sockets.TcpClient]::new("127.0.0.1", $port)
    $tcp.NoDelay = $true
    $ns_ = $tcp.GetStream(); $ns_.ReadTimeout = 3000
    $rd = [System.IO.StreamReader]::new($ns_); $wr = [System.IO.StreamWriter]::new($ns_); $wr.AutoFlush = $true
    $wr.Write("AUTH $key`n"); $auth = $rd.ReadLine()
    if (-not $auth.StartsWith("OK")) { throw "auth failed: $auth" }
    $wr.Write("PERSISTENT`nclient-attach`nclient-size 120 30`n")

    for ($h = 0; $h -lt $Hogs; $h++) {
        $hogProcs += Start-Process pwsh -ArgumentList '-NoProfile', '-Command', 'while($true){}' -WindowStyle Hidden -PassThru
    }
    if ($Hogs -gt 0) { Start-Sleep -Seconds 2 }

    $sw = [System.Diagnostics.Stopwatch]::new()
    $samples = [System.Collections.Generic.List[double]]::new()
    $misses = 0
    for ($w = 0; $w -lt $Words; $w++) {
        $word = "zqx" + ('{0:D6}' -f (Get-Random -Maximum 999999))
        for ($c = 0; $c -lt $word.Length; $c++) {
            $prefix = $word.Substring(0, $c + 1)
            $sw.Restart()
            $wr.Write("send-text " + $word[$c] + "`n")
            if ($c -lt 3) { Start-Sleep -Milliseconds 30; continue }
            $hit = $false
            while ($sw.ElapsedMilliseconds -lt 1000) {
                try { $line = $rd.ReadLine() } catch { break }
                if ($null -eq $line) { break }
                if ($line.Contains($prefix)) { $hit = $true; break }
            }
            if ($hit) { $samples.Add($sw.Elapsed.TotalMilliseconds) } else { $misses++ }
            Start-Sleep -Milliseconds 30
        }
        $wr.Write("send-key esc`n"); Start-Sleep -Milliseconds 60
    }
    $tcp.Close()
    $s = $samples | Sort-Object
    $n = $s.Count
    function Pct($p) { if ($n -eq 0) { return 'n/a' } ; '{0:N1}' -f $s[[math]::Min($n - 1, [int][math]::Floor($p * $n))] }
    "{0}: hogs={1} n={2} misses={3} p50={4}ms p90={5}ms p99={6}ms max={7}ms" -f $Label, $Hogs, $n, $misses, (Pct 0.5), (Pct 0.9), (Pct 0.99), ('{0:N1}' -f ($s | Select-Object -Last 1))
}
finally {
    foreach ($p in $hogProcs) { try { Stop-Process -Id $p.Id -Force } catch {} }
    & $Exe -L $ns kill-server 2>&1 | Out-Null
    $after = (& $Exe ls 2>&1 | Out-String)
    if ($before -ne $after) { "WARNING: default namespace ls changed!`nBEFORE:`n$before`nAFTER:`n$after" }
}
