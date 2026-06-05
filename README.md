# VPNGate Link

<p>
  <a href="README.zh-CN.md">简体中文</a> |
  <strong>English</strong>
</p>

<p>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-2024-f74c00?style=flat-square&logo=rust&logoColor=white">
  <img alt="React" src="https://img.shields.io/badge/React-19-149eca?style=flat-square&logo=react&logoColor=white">
  <img alt="TypeScript" src="https://img.shields.io/badge/TypeScript-5-3178c6?style=flat-square&logo=typescript&logoColor=white">
  <img alt="Vite" src="https://img.shields.io/badge/Vite-7-646cff?style=flat-square&logo=vite&logoColor=white">
  <img alt="OpenVPN" src="https://img.shields.io/badge/OpenVPN-link-ea7e20?style=flat-square&logo=openvpn&logoColor=white">
  <img alt="Ubuntu Server" src="https://img.shields.io/badge/Ubuntu_Server-production-e95420?style=flat-square&logo=ubuntu&logoColor=white">
  <img alt="Other platforms" src="https://img.shields.io/badge/Other_platforms-experimental-6b7280?style=flat-square">
</p>

VPNGate Link is a server-side link manager for VPNGate public relay nodes. It lets an Ubuntu VPS expose a local HTTP/SOCKS5 relay, connect that relay to a selected VPNGate OpenVPN tunnel, and then use that local relay as the upstream/front proxy for mieru, hysteria2, sing-box, or other private proxy stacks.

```text
Local client -> mieru / hysteria2 -> VPS -> 127.0.0.1:19080 -> vgl0 -> VPNGate exit -> Internet
```

<table>
  <tr>
    <td><strong>Backend</strong><br>Rust, Axum, Tokio</td>
    <td><strong>Console</strong><br>React, Vite, TypeScript</td>
    <td><strong>VPNGate link</strong><br>OpenVPN + VPNGate public relay list</td>
  </tr>
  <tr>
    <td><strong>Relay</strong><br>HTTP proxy + SOCKS5 proxy</td>
    <td><strong>Ubuntu Server</strong><br>primary production target</td>
    <td><strong>Other platforms</strong><br>experimental packages</td>
  </tr>
</table>

<table>
  <tr>
    <td><strong>For users</strong><br>Install the deb package, open the web console, choose a VPNGate server, and use <code>127.0.0.1:19080</code>.</td>
    <td><strong>For developers</strong><br>Run Rust + React locally, deploy to Ubuntu Server, or build release artifacts.</td>
  </tr>
  <tr>
    <td><strong>Proxy integration</strong><br>Use the local relay as the upstream/front proxy for mieru, hysteria2, sing-box, or similar stacks.</td>
    <td><strong>Release flow</strong><br>GitHub Actions builds Ubuntu deb packages first, plus experimental Windows Server x64 zip artifacts.</td>
  </tr>
</table>

## Contents

- [For Users](#for-users)
- [How It Works](#how-it-works)
- [Feature Checklist](#feature-checklist)
- [Use as mieru / hysteria2 Upstream](#use-as-mieru--hysteria2-upstream)
- [For Developers](#for-developers)
- [Windows Server](#windows-server)
- [CI and Releases](#ci-and-releases)
- [Configuration](#configuration)
- [API Testing](#api-testing)
- [References](#references)

## For Users

This is the normal path when you only want to install the server, open the web console, choose a VPNGate exit server, and start using the local relay.

Production use is designed around Ubuntu Server. Other operating systems may build or run selected parts of the project, but they are treated as experimental.

### 1. Install from a deb package

Download the matching package from GitHub Releases:

```text
vpngate-link_amd64.deb
vpngate-link_arm64.deb
```

Install it on Ubuntu Server:

```bash
sudo apt-get update
sudo apt-get install -y ./vpngate-link_amd64.deb
```

The package installs:

- Backend binary: `/opt/vpngate-link/vpngate-link`
- React console: `/opt/vpngate-link/web`
- Config file: `/etc/default/vpngate-link`
- systemd service: `vpngate-link.service`
- Local control UI: `127.0.0.1:18081`
- Local HTTP/SOCKS5 relay: `127.0.0.1:19080`

Check the service:

```bash
sudo systemctl status vpngate-link --no-pager -l
sudo journalctl -u vpngate-link -e --no-pager
```

### 2. Open the web console

The control UI only listens on localhost by default. From your laptop:

```bash
ssh -L 18081:127.0.0.1:18081 root@VPS_IP
```

Then open:

```text
http://127.0.0.1:18081
```

Find the generated token:

```bash
sudo grep '^VGL_TOKEN=' /etc/default/vpngate-link
```

### 3. Choose an exit server

In the web console:

1. Refresh the node list.
2. Search by country, region, speed, ping, or score.
3. Pick a server and click connect.
4. Check the exit IP in the console.
5. Use `127.0.0.1:19080` as the upstream HTTP/SOCKS5 relay for your local relay service.

Once connected, the VPS exposes:

```text
SOCKS5: 127.0.0.1:19080
HTTP:   127.0.0.1:19080
```

Your private protocol server on the same VPS can then forward outbound traffic to this relay.

## How It Works

VPNGate Link does not run or own the public VPN servers. The server catalog comes from the VPNGate Academic Experiment Project, which publishes a public list of volunteer-operated VPN relay servers.

The backend fetches:

```text
https://www.vpngate.net/api/iphone/
```

That endpoint returns a CSV-style catalog. Each row contains metadata such as country, score, ping, speed, IP/hostname, OpenVPN support fields, and a base64-encoded OpenVPN configuration. VPNGate Link parses the catalog, stores the nodes locally, and lets you choose one in the web console.

When you connect:

1. The selected VPNGate OpenVPN config is decoded and written into the data directory.
2. OpenVPN is started with a fixed tun device, default `vgl0`.
3. The local relay keeps listening on `127.0.0.1:19080`.
4. On Linux, outbound sockets from the relay are bound to the tun device with `SO_BINDTODEVICE`.
5. Your mieru/hysteria2/sing-box service can use `127.0.0.1:19080` as its upstream proxy.

Ubuntu Server is the production target because it provides the Linux networking controls needed for per-relay tunnel binding.

## Feature Checklist

| Area | Status | Notes |
| --- | --- | --- |
| Ubuntu Server deb package | Production | Installs backend, React console, config, systemd service, and token generation. |
| Web console | Implemented | Node refresh, search, connect, disconnect, favorites, settings, logs, health, and exit IP checks. |
| VPNGate catalog | Implemented | Fetches and parses the public VPNGate CSV/OpenVPN catalog from `VGL_CATALOG_URL`. |
| OpenVPN management | Implemented | Decodes selected node config, starts OpenVPN, waits for readiness, and stores runtime state. |
| Local relay | Implemented | One local port supports SOCKS5 and HTTP proxy traffic on `127.0.0.1:19080`. |
| Linux tunnel binding | Implemented | Relay outbound sockets bind to the tun device with `SO_BINDTODEVICE`. |
| mieru / hysteria2 integration | Documented | Example upstream configs are provided in `examples/`. |
| API token auth | Implemented | `Authorization: Bearer <token>` and `x-vgl-token` are supported. |
| CI release artifacts | Implemented | Builds Ubuntu amd64/arm64 deb packages and experimental Windows x64 zip. |
| Windows Server | Experimental | Runs backend and console, but strict tun-device outbound binding is not guaranteed. |

## Use as mieru / hysteria2 Upstream

The recommended deployment shape:

```text
User device
  -> private protocol client
  -> VPS private protocol server
  -> VPNGate Link local relay 127.0.0.1:19080
  -> selected VPNGate exit
  -> target site
```

Example configs are in:

- [examples/mita-upstream.json](examples/mita-upstream.json)
- [examples/hysteria2-outbounds.yaml](examples/hysteria2-outbounds.yaml)

Minimal mieru-style upstream example:

```json
{
  "egress": {
    "proxies": [
      {
        "name": "edge-relay",
        "protocol": "SOCKS5_PROXY_PROTOCOL",
        "host": "127.0.0.1",
        "port": 19080
      }
    ],
    "rules": [
      {
        "ipRanges": ["*"],
        "domainNames": ["*"],
        "action": "PROXY",
        "proxyNames": ["edge-relay"]
      }
    ]
  }
}
```

## For Developers

Use this path when you want to build, modify, deploy, or publish the project.

### Local development

Backend:

```bash
cargo fmt -- --check
cargo check
cargo test
cargo run
```

Frontend:

```bash
cd web
npm install
npm run dev
```

Production frontend build:

```bash
cd web
npm run build
```

Run backend with explicit paths:

```bash
VGL_CONTROL=127.0.0.1:18081 \
VGL_RELAY=127.0.0.1:19080 \
VGL_DATA_DIR=/tmp/vpngate-link-data \
VGL_WEB_DIR="$PWD/web/dist" \
VGL_TOKEN=dev-token \
cargo run
```

### Deploy from source on an Ubuntu server

On an Ubuntu VPS:

```bash
sudo bash install.sh
```

Required server capabilities:

- `/dev/net/tun`
- OpenVPN
- iproute2
- iptables
- systemd

If the VPS is LXC/OpenVZ, enable TUN/TAP from the provider panel.

### Build a deb package

On Debian/Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y build-essential dpkg-dev curl ca-certificates npm openvpn iproute2 iptables
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
bash scripts/package-deb.sh
```

Outputs:

```text
target/deb/vpngate-link_0.1.0_amd64.deb
target/deb/vpngate-link_amd64.deb
```

The fixed-name package, such as `vpngate-link_amd64.deb`, is intended for GitHub Release assets and one-line installers.

### One-line release installer

After publishing deb assets to GitHub Releases:

```bash
curl -fsSL https://raw.githubusercontent.com/YOUR_GITHUB_USER/vpngate-link/main/scripts/install-release.sh | sudo VGL_REPO=YOUR_GITHUB_USER/vpngate-link bash
```

Or install from your own deb URL:

```bash
curl -fsSL https://raw.githubusercontent.com/YOUR_GITHUB_USER/vpngate-link/main/scripts/install-release.sh | sudo VGL_DEB_URL=https://example.com/vpngate-link_amd64.deb bash
```

## Windows Server

Windows Server support is experimental. It can run the backend, the web console, OpenVPN control, and the local HTTP/SOCKS5 relay, but Ubuntu Server remains the production target.

Build:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -Arch x64
```

Output:

```text
target/windows/vpngate-link-windows-x64.zip
```

Run after extracting on Windows Server:

```powershell
powershell -ExecutionPolicy Bypass -File .\run.ps1
```

Install startup task from an Administrator PowerShell:

```powershell
powershell -ExecutionPolicy Bypass -File .\install-startup-task.ps1
```

Windows requires OpenVPN to be installed. If `openvpn.exe` is not in `PATH`, edit `vpngate-link.env`:

```text
OPENVPN_CMD=C:\Program Files\OpenVPN\bin\openvpn.exe
```

Important limitation: Windows does not provide a direct equivalent of Linux `SO_BINDTODEVICE`. The Windows package can run the console and relay, but it cannot guarantee that relay outbound sockets are strictly bound to a specific tun device. Use Ubuntu Server for production-grade per-relay tunnel isolation.

## CI and Releases

GitHub Actions workflow:

```text
.github/workflows/deb.yml
```

It runs:

- Rust and React quality checks on Ubuntu 22.04 and 24.04
- amd64 deb build on `ubuntu-22.04`
- arm64 deb build on `ubuntu-22.04-arm`
- deb install tests inside Ubuntu 22.04, 24.04, and 26.04 containers
- experimental Windows Server 2022 and 2025 build and smoke tests
- automatic release upload on `v*` tags

Publish a release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Release assets:

```text
vpngate-link_amd64.deb
vpngate-link_arm64.deb
vpngate-link-windows-x64.zip
```

## Configuration

Default Linux config file:

```text
/etc/default/vpngate-link
```

Common environment variables:

```bash
VGL_CONTROL=127.0.0.1:18081
VGL_RELAY=127.0.0.1:19080
VGL_TUN=vgl0
VGL_DATA_DIR=/opt/vpngate-link/data
VGL_WEB_DIR=/opt/vpngate-link/web
VGL_TOKEN=change-this-token
VGL_REFRESH_SECONDS=1260
VGL_CONNECT_TIMEOUT_SECONDS=35
VGL_MAX_NODES=300
VGL_CATALOG_URL=https://www.vpngate.net/api/iphone/
OPENVPN_CMD=openvpn
OPENVPN_AUTH_USER=vpn
OPENVPN_AUTH_PASS=vpn
```

## API Testing

Control API surface:

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/api/status` | Runtime status, active node, relay/control address, auth state. |
| `GET` | `/api/nodes` | Current VPNGate node catalog cached by the server. |
| `GET` | `/api/logs` | In-memory service logs. |
| `GET` | `/api/health` | Relay and tunnel health snapshot. |
| `GET` | `/api/exit_ip` | Checks the public IP through the local relay. |
| `GET` | `/api/settings` | Current routing and selection settings. |
| `POST` | `/api/settings` | Update routing and selection settings. |
| `POST` | `/api/favorite` | Toggle a node in the favorite list with `{"id":"NODE_ID"}`. |
| `POST` | `/api/test_node` | TCP reachability test for a node with `{"id":"NODE_ID"}`. |
| `POST` | `/api/refresh` | Refresh the VPNGate catalog. |
| `POST` | `/api/autoconnect` | Select and connect using current settings. |
| `POST` | `/api/connect` | Connect a specific node with `{"id":"NODE_ID"}`. |
| `POST` | `/api/disconnect` | Stop the current OpenVPN session. |

With token:

```bash
TOKEN="$(sudo grep '^VGL_TOKEN=' /etc/default/vpngate-link | cut -d= -f2-)"
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/status
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/health
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/logs
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/exit_ip
curl -H "Authorization: Bearer $TOKEN" -X POST http://127.0.0.1:18081/api/refresh
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/nodes
```

Connect using a node id copied from the node list:

```bash
curl -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -X POST http://127.0.0.1:18081/api/connect \
  -d '{"id":"NODE_ID"}'
```

Disconnect:

```bash
curl -H "Authorization: Bearer $TOKEN" -X POST http://127.0.0.1:18081/api/disconnect
```

Check relay exit:

```bash
curl -x socks5h://127.0.0.1:19080 https://api.ipify.org
```

The returned IP should be the selected VPNGate exit, not the VPS IP.

## References

- VPNGate public site: <https://www.vpngate.net/>
- VPNGate project overview: <https://www.vpngate.net/en/about_overview.aspx>
- VPNGate project operators and volunteer relay note: <https://www.vpngate.net/en/about_us.aspx>
- VPNGate CSV/OpenVPN catalog endpoint used by this project: <https://www.vpngate.net/api/iphone/>
