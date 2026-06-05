param(
    [string]$Arch = "x64"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$Version = (Select-String -Path "Cargo.toml" -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value
$OutDir = Join-Path $Root "target/windows"
$PackageDir = Join-Path $OutDir "vpngate-link-windows-$Arch"
$ZipPath = Join-Path $OutDir "vpngate-link-windows-$Arch.zip"

if (!(Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo is required."
}
if (!(Get-Command npm -ErrorAction SilentlyContinue)) {
    throw "npm is required."
}

Write-Host "Building React management console..."
Push-Location (Join-Path $Root "web")
try {
    if (Test-Path "package-lock.json") {
        npm ci
    } else {
        npm install
    }
    npm run build
} finally {
    Pop-Location
}

Write-Host "Building Rust release binary..."
cargo build --release --locked

Remove-Item -Recurse -Force $PackageDir -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $PackageDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $PackageDir "web") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $PackageDir "data") | Out-Null

Copy-Item (Join-Path $Root "target/release/vpngate-link.exe") (Join-Path $PackageDir "vpngate-link.exe")
Copy-Item (Join-Path $Root "README.md") (Join-Path $PackageDir "README.md")
Copy-Item (Join-Path $Root "packaging/windows/vpngate-link.env") (Join-Path $PackageDir "vpngate-link.env")
Copy-Item (Join-Path $Root "scripts/windows/run.ps1") (Join-Path $PackageDir "run.ps1")
Copy-Item (Join-Path $Root "scripts/windows/install-startup-task.ps1") (Join-Path $PackageDir "install-startup-task.ps1")
Copy-Item (Join-Path $Root "scripts/windows/uninstall-startup-task.ps1") (Join-Path $PackageDir "uninstall-startup-task.ps1")
Copy-Item -Recurse (Join-Path $Root "web/dist/.") (Join-Path $PackageDir "web")

Remove-Item -Force $ZipPath -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $PackageDir "*") -DestinationPath $ZipPath

Write-Host $ZipPath
