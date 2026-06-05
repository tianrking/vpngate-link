param(
    [string]$EnvFile = ""
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($EnvFile)) {
    $EnvFile = Join-Path $Root "vpngate-link.env"
}

function Set-EnvFromFile {
    param([string]$Path)
    if (!(Test-Path $Path)) {
        return
    }

    Get-Content $Path | ForEach-Object {
        $Line = $_.Trim()
        if ([string]::IsNullOrWhiteSpace($Line) -or $Line.StartsWith("#")) {
            return
        }
        $Parts = $Line.Split("=", 2)
        if ($Parts.Count -ne 2) {
            return
        }
        [Environment]::SetEnvironmentVariable($Parts[0].Trim(), $Parts[1].Trim(), "Process")
    }
}

function Set-DefaultEnv {
    param([string]$Name, [string]$Value)
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($Name, "Process"))) {
        [Environment]::SetEnvironmentVariable($Name, $Value, "Process")
    }
}

function Convert-ToAbsolutePath {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $Value
    }
    if ([System.IO.Path]::IsPathRooted($Value)) {
        return $Value
    }
    return [System.IO.Path]::GetFullPath((Join-Path $Root $Value))
}

Set-EnvFromFile $EnvFile

Set-DefaultEnv "VGL_CONTROL" "127.0.0.1:18081"
Set-DefaultEnv "VGL_RELAY" "127.0.0.1:19080"
Set-DefaultEnv "VGL_DATA_DIR" (Join-Path $Root "data")
Set-DefaultEnv "VGL_WEB_DIR" (Join-Path $Root "web")
Set-DefaultEnv "VGL_TUN" "vgl0"
Set-DefaultEnv "VGL_REFRESH_SECONDS" "1260"
Set-DefaultEnv "VGL_MAX_NODES" "300"
Set-DefaultEnv "VGL_CATALOG_URL" "https://www.vpngate.net/api/iphone/"
Set-DefaultEnv "OPENVPN_CMD" "openvpn.exe"
Set-DefaultEnv "OPENVPN_AUTH_USER" "vpn"
Set-DefaultEnv "OPENVPN_AUTH_PASS" "vpn"

$DataDir = Convert-ToAbsolutePath $env:VGL_DATA_DIR
$WebDir = Convert-ToAbsolutePath $env:VGL_WEB_DIR
[Environment]::SetEnvironmentVariable("VGL_DATA_DIR", $DataDir, "Process")
[Environment]::SetEnvironmentVariable("VGL_WEB_DIR", $WebDir, "Process")
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

if ([string]::IsNullOrWhiteSpace($env:VGL_TOKEN) -or $env:VGL_TOKEN -eq "__GENERATE_ON_INSTALL__") {
    $Token = [Guid]::NewGuid().ToString("N") + [Guid]::NewGuid().ToString("N").Substring(0, 4)
    [Environment]::SetEnvironmentVariable("VGL_TOKEN", $Token, "Process")

    if (Test-Path $EnvFile) {
        $Lines = Get-Content $EnvFile
        $Found = $false
        $Updated = foreach ($Line in $Lines) {
            if ($Line -match "^VGL_TOKEN=") {
                $Found = $true
                "VGL_TOKEN=$Token"
            } else {
                $Line
            }
        }
        if (!$Found) {
            $Updated += "VGL_TOKEN=$Token"
        }
        Set-Content -Path $EnvFile -Value $Updated -Encoding ascii
    }
}

$Exe = Join-Path $Root "vpngate-link.exe"
if (!(Test-Path $Exe)) {
    throw "Missing executable: $Exe"
}

Write-Host "VPNGate Link for Windows"
Write-Host "Control UI: http://$env:VGL_CONTROL"
Write-Host "Relay:      socks5/http://$env:VGL_RELAY"
Write-Host "Token:      $env:VGL_TOKEN"
Write-Host ""
Write-Host "Windows mode note: relay outbound device binding is Linux-only."
Write-Host "Use Ubuntu Server for production-grade per-relay tunnel isolation."
Write-Host ""

& $Exe
exit $LASTEXITCODE
