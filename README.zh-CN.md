# VPNGate Link

<p>
  <strong>简体中文</strong> |
  <a href="README.md">English</a>
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

VPNGate Link 是一个面向服务器的 VPNGate 公共节点连接管理服务。它在 Ubuntu VPS 上提供本地 HTTP/SOCKS5 relay，通过网页选择并连接 VPNGate OpenVPN 隧道，然后让 mieru、hysteria2、sing-box 等私有代理服务把出站流量转发到这个本地 relay。

```text
本地客户端 -> mieru / hysteria2 -> VPS -> 127.0.0.1:19080 -> vgl0 -> VPNGate 出口 -> 目标网站
```

<table>
  <tr>
    <td><strong>后端</strong><br>Rust, Axum, Tokio</td>
    <td><strong>管理台</strong><br>React, Vite, TypeScript</td>
    <td><strong>VPNGate Link</strong><br>OpenVPN + VPNGate 公共节点列表</td>
  </tr>
  <tr>
    <td><strong>本地 relay</strong><br>HTTP proxy + SOCKS5 proxy</td>
    <td><strong>Ubuntu Server</strong><br>主要生产目标</td>
    <td><strong>其他平台</strong><br>实验性支持</td>
  </tr>
</table>

<table>
  <tr>
    <td><strong>给使用者</strong><br>安装 deb 包，打开管理网页，选择 VPNGate 服务器，然后使用 <code>127.0.0.1:19080</code>。</td>
    <td><strong>给开发者</strong><br>本地运行 Rust + React，部署到 Ubuntu Server，或者构建发布产物。</td>
  </tr>
  <tr>
    <td><strong>代理集成</strong><br>把本地 relay 作为 mieru、hysteria2、sing-box 等协议栈的出站/前置代理。</td>
    <td><strong>发布流程</strong><br>GitHub Actions 优先构建 Ubuntu deb 包，同时生成实验性的 Windows Server x64 zip。</td>
  </tr>
</table>

## 目录

- [给使用者](#给使用者)
- [工作原理](#工作原理)
- [功能清单](#功能清单)
- [作为 mieru / hysteria2 的前置出口](#作为-mieru--hysteria2-的前置出口)
- [给开发者](#给开发者)
- [Windows Server](#windows-server)
- [CI 与发布](#ci-与发布)
- [配置项](#配置项)
- [API 测试](#api-测试)
- [参考链接](#参考链接)

## 给使用者

如果你只是想在 VPS 上安装服务、打开网页、选择一个服务器连接，然后把 `127.0.0.1:19080` 当成 mieru/hysteria2 的前置出口，用这一段就够了。

生产使用围绕 Ubuntu Server 设计。其他操作系统可能可以构建或运行部分功能，但都按实验功能处理。

### 1. 下载并安装 deb

从 GitHub Releases 下载对应架构的包：

```text
vpngate-link_amd64.deb
vpngate-link_arm64.deb
```

在 Ubuntu Server 上安装：

```bash
sudo apt-get update
sudo apt-get install -y ./vpngate-link_amd64.deb
```

安装后会自动放好这些东西：

- 后端程序：`/opt/vpngate-link/vpngate-link`
- React 管理网页：`/opt/vpngate-link/web`
- 配置文件：`/etc/default/vpngate-link`
- systemd 服务：`vpngate-link.service`
- 管理网页监听：`127.0.0.1:18081`
- 本地 HTTP/SOCKS5 relay：`127.0.0.1:19080`

检查服务状态：

```bash
sudo systemctl status vpngate-link --no-pager -l
sudo journalctl -u vpngate-link -e --no-pager
```

### 2. 打开管理网页

管理网页默认只监听 VPS 本机，所以推荐用 SSH 隧道从自己电脑打开：

```bash
ssh -L 18081:127.0.0.1:18081 root@VPS_IP
```

然后浏览器打开：

```text
http://127.0.0.1:18081
```

查看自动生成的 token：

```bash
sudo grep '^VGL_TOKEN=' /etc/default/vpngate-link
```

### 3. 在网页里选择服务器

进入管理网页后：

1. 刷新节点列表。
2. 按国家、协议、状态、收藏、可达性、延迟、速度、负载、评分筛选。
3. 点击 **Scan Visible** 对当前筛选出来的节点做真实 TCP 可达性扫描。
4. 选择一个可达服务器并连接。后端会启动 OpenVPN，隧道就绪后才会标记为 active。
5. 在网页里查看当前出口 IP。
6. 把 `127.0.0.1:19080` 配成你的 mieru/hysteria2/sing-box 出站代理。

连接成功后，VPS 本机可用：

```text
SOCKS5: 127.0.0.1:19080
HTTP:   127.0.0.1:19080
```

你的私有协议服务只需要把出站流量交给这个 relay，就可以利用网页里选择的动态出口。

## 工作原理

这些服务器不是本项目自建的，也不是固定内置的。节点来自 VPNGate Academic Experiment Project 提供的公开志愿者 VPN Relay 列表。

后端会抓取：

```text
https://www.vpngate.net/api/iphone/
```

这个接口返回类似 CSV 的节点目录。每一行包含国家、评分、延迟、速度、IP/主机名、OpenVPN 支持字段，以及 base64 编码的 OpenVPN 配置。VPNGate Link 会解析这个目录，把节点保存到本地，然后在管理网页里展示出来。

你点击连接时：

1. 后端解码选中节点的 OpenVPN 配置。
2. 写入本地数据目录。
3. 启动 OpenVPN，并固定 tun 设备名，默认是 `vgl0`。
4. 本地 relay 继续监听 `127.0.0.1:19080`。
5. Linux 上 relay 的出站 socket 会通过 `SO_BINDTODEVICE` 绑定到 tun 设备。
6. mieru/hysteria2/sing-box 可以把 `127.0.0.1:19080` 当成出站代理。

生产环境推荐 Ubuntu Server，因为它提供了实现“指定 relay 流量走指定 tun 出口”所需的 Linux 网络控制能力。

## 功能清单

| 模块 | 状态 | 说明 |
| --- | --- | --- |
| Ubuntu Server deb 包 | 生产可用 | 安装后端、React 管理台、配置文件、systemd 服务，并自动生成 token。 |
| Web 管理台 | 已实现 | 节点刷新、多维筛选、批量可达性扫描、连接、断开、收藏、设置、日志、健康检查、出口 IP 检查。 |
| VPNGate 节点目录 | 已实现 | 从 `VGL_CATALOG_URL` 抓取并解析 VPNGate 公开 CSV/OpenVPN 节点目录。 |
| OpenVPN 管理 | 已实现 | 解码选中节点配置，启动 OpenVPN，等待连接就绪，并保存运行状态。 |
| 本地 relay | 已实现 | 一个本地端口同时支持 SOCKS5 和 HTTP proxy，默认 `127.0.0.1:19080`。 |
| Linux 隧道绑定 | 已实现 | relay 出站 socket 通过 `SO_BINDTODEVICE` 绑定到 tun 设备。 |
| mieru / hysteria2 集成 | 已文档化 | `examples/` 里提供上游代理参考配置。 |
| API token 鉴权 | 已实现 | 支持 `Authorization: Bearer <token>` 和 `x-vgl-token`。 |
| CI 发布产物 | 已实现 | 构建 Ubuntu amd64/arm64 deb 包和实验性的 Windows x64 zip。 |
| Windows Server | 实验性 | 可运行后端和管理台，但不保证出站严格绑定到指定 tun 设备。 |

## 作为 mieru / hysteria2 的前置出口

推荐部署形态：

```text
用户设备
  -> 私有协议客户端
  -> VPS 上的私有协议服务端
  -> VPNGate Link 本地 relay 127.0.0.1:19080
  -> 选中的 VPNGate 出口
  -> 目标网站
```

参考配置在：

- [examples/mita-upstream.json](examples/mita-upstream.json)
- [examples/hysteria2-outbounds.yaml](examples/hysteria2-outbounds.yaml)

最小 mieru 风格上游代理示例：

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

## 给开发者

如果你要改代码、二次开发、自己部署、打包或者发版，看这一段。

### 本地开发

后端：

```bash
cargo fmt -- --check
cargo check
cargo test
cargo run
```

前端：

```bash
cd web
npm install
npm run dev
```

生产构建前端：

```bash
cd web
npm run build
```

显式指定目录运行后端：

```bash
VGL_CONTROL=127.0.0.1:18081 \
VGL_RELAY=127.0.0.1:19080 \
VGL_DATA_DIR=/tmp/vpngate-link-data \
VGL_WEB_DIR="$PWD/web/dist" \
VGL_TOKEN=dev-token \
cargo run
```

macOS 或 Windows 本地开发可以验证 API、节点目录解析、React 构建和 relay 监听。真实切换出口 IP 需要 Ubuntu/Linux、OpenVPN、`/dev/net/tun` 和策略路由能力。

### Ubuntu 真机验证

安装到 Ubuntu VPS 后运行：

```bash
sudo scripts/live-ubuntu-check.sh
```

这个脚本会把生产路径端到端跑一遍：

1. 读取 `/etc/default/vpngate-link`。
2. 调用本机控制 API。
3. 刷新 VPNGate 节点目录。
4. 通过 `/api/test_nodes` 扫描 TCP 候选节点。
5. 通过 `/api/connect` 连接第一个可达节点。
6. 通过 `socks5h://127.0.0.1:19080` 获取公网出口 IP。
7. 如果 relay 出口 IP 仍然等于 VPS 直连 IP，脚本会失败。

### 从源码部署到 Ubuntu 服务器

在 Ubuntu VPS 上：

```bash
sudo bash install.sh
```

服务器需要：

- `/dev/net/tun`
- OpenVPN
- iproute2
- iptables
- systemd

如果 VPS 是 LXC/OpenVZ，需要在服务商面板打开 TUN/TAP。

### 构建 deb 包

在 Debian/Ubuntu 构建机上：

```bash
sudo apt-get update
sudo apt-get install -y build-essential dpkg-dev curl ca-certificates npm openvpn iproute2 iptables
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
bash scripts/package-deb.sh
```

输出：

```text
target/deb/vpngate-link_0.1.0_amd64.deb
target/deb/vpngate-link_amd64.deb
```

固定文件名的包，比如 `vpngate-link_amd64.deb`，适合上传到 GitHub Release，给一键安装脚本下载。

### 一条命令安装发布版

上传 deb 到 GitHub Releases 后：

```bash
curl -fsSL https://raw.githubusercontent.com/YOUR_GITHUB_USER/vpngate-link/main/scripts/install-release.sh | sudo VGL_REPO=YOUR_GITHUB_USER/vpngate-link bash
```

也可以指定自己的 deb 下载地址：

```bash
curl -fsSL https://raw.githubusercontent.com/YOUR_GITHUB_USER/vpngate-link/main/scripts/install-release.sh | sudo VGL_DEB_URL=https://example.com/vpngate-link_amd64.deb bash
```

## Windows Server

Windows Server 是实验性支持。它可以运行后端、管理网页、OpenVPN 启停、本地 HTTP/SOCKS5 relay，但 Ubuntu Server 仍然是生产主线。

打包：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/package-windows.ps1 -Arch x64
```

输出：

```text
target/windows/vpngate-link-windows-x64.zip
```

在 Windows Server 上解压后运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\run.ps1
```

管理员 PowerShell 安装开机自启任务：

```powershell
powershell -ExecutionPolicy Bypass -File .\install-startup-task.ps1
```

Windows 需要先安装 OpenVPN。如果 `openvpn.exe` 不在 `PATH` 里，修改 `vpngate-link.env`：

```text
OPENVPN_CMD=C:\Program Files\OpenVPN\bin\openvpn.exe
```

重要限制：Windows 没有 Linux `SO_BINDTODEVICE` 的直接等价能力。Windows 包可以运行网页和 relay，但不能保证 relay 出站 socket 严格绑定到某个 tun 设备。生产级“私有协议 -> relay -> 指定 VPN 出口”请使用 Ubuntu Server。

## CI 与发布

GitHub Actions 工作流：

```text
.github/workflows/deb.yml
```

它会执行：

- Ubuntu 22.04 / 24.04 上的 Rust 与 React 质量检查
- `ubuntu-22.04` 构建 amd64 deb
- `ubuntu-22.04-arm` 构建 arm64 deb
- Ubuntu 22.04 / 24.04 / 26.04 容器安装验证
- 实验性的 Windows Server 2022 / 2025 构建和启动验证
- `v*` tag 自动上传 release 产物

发布一个版本：

```bash
git tag v0.1.0
git push origin v0.1.0
```

Release 产物：

```text
vpngate-link_amd64.deb
vpngate-link_arm64.deb
vpngate-link-windows-x64.zip
```

## 配置项

Linux 默认配置文件：

```text
/etc/default/vpngate-link
```

常用环境变量：

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

## API 测试

控制 API 一览：

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/api/status` | 运行状态、当前节点、relay/control 地址、鉴权状态。 |
| `GET` | `/api/nodes` | 服务端缓存的 VPNGate 节点目录。 |
| `GET` | `/api/logs` | 内存中的服务日志。 |
| `GET` | `/api/health` | relay 和隧道健康状态快照。 |
| `GET` | `/api/exit_ip` | 通过本地 relay 检查公网出口 IP。 |
| `GET` | `/api/settings` | 当前路由和选择设置。 |
| `POST` | `/api/settings` | 更新路由和选择设置。 |
| `POST` | `/api/favorite` | 使用 `{"id":"NODE_ID"}` 切换收藏节点。 |
| `POST` | `/api/test_node` | 使用 `{"id":"NODE_ID"}` 测试节点 TCP 可达性。 |
| `POST` | `/api/test_nodes` | 使用 `{"ids":["NODE_ID"]}` 批量测试节点 TCP 可达性；每次最多 200 个 id。 |
| `POST` | `/api/refresh` | 刷新 VPNGate 节点目录。 |
| `POST` | `/api/autoconnect` | 根据当前设置自动选择并连接。 |
| `POST` | `/api/connect` | 使用 `{"id":"NODE_ID"}` 连接指定节点。 |
| `POST` | `/api/disconnect` | 停止当前 OpenVPN 会话。 |

带 token：

```bash
TOKEN="$(sudo grep '^VGL_TOKEN=' /etc/default/vpngate-link | cut -d= -f2-)"
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/status
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/health
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/logs
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/exit_ip
curl -H "Authorization: Bearer $TOKEN" -X POST http://127.0.0.1:18081/api/refresh
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18081/api/nodes
```

用 API 扫描筛选或指定的节点：

```bash
curl -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -X POST http://127.0.0.1:18081/api/test_nodes \
  -d '{"ids":["NODE_ID"]}'
```

用节点列表里复制的 id 连接：

```bash
curl -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -X POST http://127.0.0.1:18081/api/connect \
  -d '{"id":"NODE_ID"}'
```

断开连接：

```bash
curl -H "Authorization: Bearer $TOKEN" -X POST http://127.0.0.1:18081/api/disconnect
```

验证 relay 出口：

```bash
curl -x socks5h://127.0.0.1:19080 https://api.ipify.org
```

返回值应该是当前选中的 VPNGate 出口 IP，而不是 VPS 自己的 IP。

## 参考链接

- VPNGate 官网：<https://www.vpngate.net/>
- VPNGate 项目概览：<https://www.vpngate.net/en/about_overview.aspx>
- VPNGate 管理者与志愿者 relay 说明：<https://www.vpngate.net/en/about_us.aspx>
- 本项目使用的 VPNGate CSV/OpenVPN 节点目录：<https://www.vpngate.net/api/iphone/>
