# Mesure l'empreinte de BoringWindows au repos.
#   cargo build --release
#   pwsh scripts/measure-idle.ps1
# Objectifs (PLAN.md) : RAM < 30 Mo, CPU ~0 %, binaire < 15 Mo.
param(
    [string]$Exe = "target\release\boringwindows.exe",
    [int]$WarmupSeconds = 3,
    [int]$Seconds = 10
)

$ErrorActionPreference = "Stop"
$p = Start-Process -FilePath $Exe -PassThru
try {
    Start-Sleep -Seconds $WarmupSeconds
    $p.Refresh()
    if ($p.HasExited) { throw "BoringWindows s'est arrêté (code $($p.ExitCode))" }
    $cpu0 = $p.TotalProcessorTime
    Start-Sleep -Seconds $Seconds
    $p.Refresh()
    $cpuMs = ($p.TotalProcessorTime - $cpu0).TotalMilliseconds
    $cpuPct = $cpuMs / ($Seconds * 1000) / [Environment]::ProcessorCount * 100

    "Binaire           : {0,8:N1} Mo" -f ((Get-Item $Exe).Length / 1MB)
    "RAM (working set) : {0,8:N1} Mo" -f ($p.WorkingSet64 / 1MB)
    "RAM privée        : {0,8:N1} Mo" -f ($p.PrivateMemorySize64 / 1MB)
    "CPU au repos      : {0,8:N2} %  ({1:N0} ms sur {2} s)" -f $cpuPct, $cpuMs, $Seconds
}
finally {
    if (-not $p.HasExited) { Stop-Process -Id $p.Id }
}
