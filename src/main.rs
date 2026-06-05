mod catalog;
mod config;
mod model;
mod openvpn;
mod proxy;
mod server;
mod store;

use tokio::net::TcpListener;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt};

use crate::{
    config::Config,
    server::{do_refresh, router},
    store::{AppState, ensure_dirs, load_nodes, load_settings, save_runtime},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=warn,reqwest=warn,hyper=warn")),
        )
        .init();

    let cfg = Config::from_env()?;
    ensure_dirs(&cfg).await?;

    let state = AppState::new(cfg.clone());
    {
        let nodes = load_nodes(&cfg).await?;
        let mut guard = state.nodes.write().await;
        *guard = nodes;
    }
    {
        let settings = load_settings(&cfg).await?;
        let mut guard = state.settings.write().await;
        *guard = settings;
    }
    {
        let count = state.nodes.read().await.len();
        let mut runtime = state.runtime.write().await;
        runtime.node_count = count;
        runtime.last_message = "ready".to_string();
        save_runtime(&cfg, &runtime).await.ok();
    }

    let relay_addr = cfg.relay_addr;
    let tunnel_device = cfg.tunnel_device.clone();
    tokio::spawn(async move {
        if let Err(err) = proxy::serve_relay(relay_addr, tunnel_device).await {
            error!("relay failed: {err:?}");
        }
    });

    let refresh_state = state.clone();
    tokio::spawn(async move {
        loop {
            if let Err(err) = do_refresh(&refresh_state).await {
                error!("background refresh failed: {err:?}");
            }
            tokio::time::sleep(refresh_state.cfg.refresh_interval).await;
        }
    });

    let listener = TcpListener::bind(cfg.control_addr).await?;
    info!("control API listening on {}", cfg.control_addr);
    axum::serve(listener, router(state)).await?;
    Ok(())
}
