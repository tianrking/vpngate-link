param(
    [string]$InstallDir = "",
    [string]$TaskName = "VPNGateLink"
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Split-Path -Parent $MyInvocation.MyCommand.Path
}
$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)
$RunScript = Join-Path $InstallDir "run.ps1"

if (!(Test-Path $RunScript)) {
    throw "Missing run script: $RunScript"
}

$Action = New-ScheduledTaskAction `
    -Execute "powershell.exe" `
    -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$RunScript`"" `
    -WorkingDirectory $InstallDir
$Trigger = New-ScheduledTaskTrigger -AtStartup
$Principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -RunLevel Highest
$Settings = New-ScheduledTaskSettingsSet `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit (New-TimeSpan -Days 3650) `
    -RestartCount 3 `
    -RestartInterval (New-TimeSpan -Minutes 1)

Register-ScheduledTask `
    -TaskName $TaskName `
    -Action $Action `
    -Trigger $Trigger `
    -Principal $Principal `
    -Settings $Settings `
    -Force | Out-Null

Start-ScheduledTask -TaskName $TaskName

Write-Host "Installed startup task: $TaskName"
Write-Host "Control UI: http://127.0.0.1:18081"
Write-Host "Use SSH tunnel or firewall rules carefully for remote access."
