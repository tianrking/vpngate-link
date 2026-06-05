use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

#[derive(Clone, Debug)]
pub struct Config {
    pub data_dir: PathBuf,
    pub web_dir: PathBuf,
    pub catalog_url: String,
    pub control_addr: SocketAddr,
    pub relay_addr: SocketAddr,
    pub tunnel_device: String,
    pub api_token: Option<String>,
    pub openvpn_cmd: String,
    pub openvpn_user: String,
    pub openvpn_pass: String,
    pub refresh_interval: Duration,
    pub connect_timeout: Duration,
    pub max_nodes: usize,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = env::var("VGL_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./gateway_data"));
        let web_dir = env::var("VGL_WEB_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./web/dist"));
        let control_addr = env::var("VGL_CONTROL")
            .unwrap_or_else(|_| "127.0.0.1:18081".to_string())
            .parse()?;
        let relay_addr = env::var("VGL_RELAY")
            .unwrap_or_else(|_| "127.0.0.1:19080".to_string())
            .parse()?;

        Ok(Self {
            data_dir,
            web_dir,
            catalog_url: env::var("VGL_CATALOG_URL")
                .unwrap_or_else(|_| "https://www.vpngate.net/api/iphone/".to_string()),
            control_addr,
            relay_addr,
            tunnel_device: env::var("VGL_TUN").unwrap_or_else(|_| "vgl0".to_string()),
            api_token: env::var("VGL_TOKEN").ok().filter(|v| !v.trim().is_empty()),
            openvpn_cmd: env::var("OPENVPN_CMD").unwrap_or_else(|_| "openvpn".to_string()),
            openvpn_user: env::var("OPENVPN_AUTH_USER").unwrap_or_else(|_| "vpn".to_string()),
            openvpn_pass: env::var("OPENVPN_AUTH_PASS").unwrap_or_else(|_| "vpn".to_string()),
            refresh_interval: Duration::from_secs(
                env::var("VGL_REFRESH_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1260),
            ),
            connect_timeout: Duration::from_secs(
                env::var("VGL_CONNECT_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(35),
            ),
            max_nodes: env::var("VGL_MAX_NODES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(300),
        })
    }

    pub fn configs_dir(&self) -> PathBuf {
        self.data_dir.join("configs")
    }

    pub fn nodes_file(&self) -> PathBuf {
        self.data_dir.join("nodes.json")
    }

    pub fn state_file(&self) -> PathBuf {
        self.data_dir.join("state.json")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }

    pub fn auth_file(&self) -> PathBuf {
        self.data_dir.join("openvpn_auth.txt")
    }
}
