use std::{path::Path, sync::Arc};

use tokio::sync::{Mutex, RwLock};

use crate::{
    config::Config,
    model::{GatewaySettings, LogEntry, Node, RuntimeStatus},
    openvpn::OpenVpnRuntime,
};

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub nodes: Arc<RwLock<Vec<Node>>>,
    pub runtime: Arc<RwLock<RuntimeStatus>>,
    pub settings: Arc<RwLock<GatewaySettings>>,
    pub logs: Arc<RwLock<Vec<LogEntry>>>,
    pub openvpn: Arc<Mutex<OpenVpnRuntime>>,
}

impl AppState {
    pub fn new(cfg: Config) -> Self {
        let relay_addr = cfg.relay_addr.to_string();
        let control_addr = cfg.control_addr.to_string();
        let auth_enabled = cfg.api_token.is_some();
        Self {
            cfg: Arc::new(cfg),
            nodes: Arc::new(RwLock::new(Vec::new())),
            runtime: Arc::new(RwLock::new(RuntimeStatus::new(
                relay_addr,
                control_addr,
                auth_enabled,
            ))),
            settings: Arc::new(RwLock::new(GatewaySettings::default())),
            logs: Arc::new(RwLock::new(Vec::new())),
            openvpn: Arc::new(Mutex::new(OpenVpnRuntime::default())),
        }
    }
}

impl AppState {
    pub async fn log(&self, level: &str, module: &str, message: impl Into<String>) {
        let mut logs = self.logs.write().await;
        logs.push(LogEntry {
            ts: unix_now(),
            level: level.to_string(),
            module: module.to_string(),
            message: message.into(),
        });
        if logs.len() > 1000 {
            let extra = logs.len() - 1000;
            logs.drain(0..extra);
        }
    }
}

pub async fn ensure_dirs(cfg: &Config) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(&cfg.data_dir).await?;
    tokio::fs::create_dir_all(cfg.configs_dir()).await?;
    let auth = cfg.auth_file();
    if tokio::fs::metadata(&auth).await.is_err() {
        tokio::fs::write(
            &auth,
            format!("{}\n{}\n", cfg.openvpn_user, cfg.openvpn_pass),
        )
        .await?;
        set_private_permissions(&auth).await?;
    }
    Ok(())
}

#[cfg(unix)]
async fn set_private_permissions(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut perms = tokio::fs::metadata(path).await?.permissions();
    perms.set_mode(0o600);
    tokio::fs::set_permissions(path, perms).await?;
    Ok(())
}

#[cfg(not(unix))]
async fn set_private_permissions(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

pub async fn load_nodes(cfg: &Config) -> anyhow::Result<Vec<Node>> {
    match tokio::fs::read_to_string(cfg.nodes_file()).await {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(err) => Err(err.into()),
    }
}

pub async fn load_settings(cfg: &Config) -> anyhow::Result<GatewaySettings> {
    match tokio::fs::read_to_string(cfg.settings_file()).await {
        Ok(text) => Ok(serde_json::from_str(&text)?),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(GatewaySettings::default()),
        Err(err) => Err(err.into()),
    }
}

pub async fn save_nodes(cfg: &Config, nodes: &[Node]) -> anyhow::Result<()> {
    write_json_atomic(&cfg.nodes_file(), nodes).await
}

pub async fn save_runtime(cfg: &Config, runtime: &RuntimeStatus) -> anyhow::Result<()> {
    write_json_atomic(&cfg.state_file(), runtime).await
}

pub async fn save_settings(cfg: &Config, settings: &GatewaySettings) -> anyhow::Result<()> {
    write_json_atomic(&cfg.settings_file(), settings).await
}

async fn write_json_atomic<T: serde::Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> anyhow::Result<()> {
    let tmp = path.with_extension("tmp");
    let text = serde_json::to_string_pretty(value)?;
    tokio::fs::write(&tmp, text).await?;
    tokio::fs::rename(tmp, path).await?;
    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
