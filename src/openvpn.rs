use std::path::PathBuf;

use anyhow::{Context, bail};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time::timeout,
};
use tracing::{info, warn};

use crate::{
    model::{Node, NodeStatus},
    store::{AppState, save_nodes, save_runtime},
};

#[derive(Default)]
pub struct OpenVpnRuntime {
    child: Option<Child>,
    config_file: Option<PathBuf>,
}

pub async fn connect_node(state: &AppState, node_id: &str) -> anyhow::Result<String> {
    state
        .log("INFO", "openvpn", format!("connecting {node_id}"))
        .await;
    {
        let mut runtime = state.runtime.write().await;
        runtime.connecting = true;
        runtime.last_message = format!("connecting {node_id}");
        save_runtime(&state.cfg, &runtime).await.ok();
    }

    stop_openvpn(state).await.ok();

    let node = {
        let nodes = state.nodes.read().await;
        nodes
            .iter()
            .find(|n| n.id == node_id)
            .cloned()
            .with_context(|| format!("node not found: {node_id}"))?
    };

    let config_file = state.cfg.configs_dir().join(format!("{}.conf", node.id));
    tokio::fs::write(&config_file, &node.config_text).await?;

    let mut child = match spawn_openvpn(state, &config_file).await {
        Ok(child) => child,
        Err(err) => {
            mark_failed(state, &node, err.to_string()).await;
            return Err(err);
        }
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            child.kill().await.ok();
            let err = anyhow::anyhow!("openvpn stdout not captured");
            mark_failed(state, &node, err.to_string()).await;
            return Err(err);
        }
    };
    let mut lines = BufReader::new(stdout).lines();
    let deadline = state.cfg.connect_timeout;
    let ready = timeout(deadline, async {
        while let Some(line) = lines.next_line().await? {
            let lower = line.to_ascii_lowercase();
            info!(target: "openvpn", "{line}");
            if lower.contains("initialization sequence completed") {
                return anyhow::Ok(());
            }
            if lower.contains("auth_failed")
                || lower.contains("authentication failed")
                || lower.contains("cannot open tun")
                || lower.contains("cannot allocate tun")
                || lower.contains("fatal error")
            {
                bail!("openvpn failed: {line}");
            }
        }
        bail!("openvpn exited before initialization")
    })
    .await;

    match ready {
        Ok(Ok(())) => {
            tokio::spawn(async move {
                while let Ok(Some(line)) = lines.next_line().await {
                    info!(target: "openvpn", "{line}");
                }
            });

            setup_policy_routing(&state.cfg.tunnel_device).await.ok();

            {
                let mut guard = state.openvpn.lock().await;
                guard.child = Some(child);
                guard.config_file = Some(config_file);
            }

            {
                let mut nodes = state.nodes.write().await;
                for item in nodes.iter_mut() {
                    if item.id == node.id {
                        item.status = NodeStatus::Active;
                        item.last_error = None;
                    } else if matches!(item.status, NodeStatus::Active) {
                        item.status = NodeStatus::New;
                    }
                }
                save_nodes(&state.cfg, &nodes).await.ok();
            }

            {
                let mut runtime = state.runtime.write().await;
                runtime.active_node_id = Some(node.id.clone());
                runtime.connecting = false;
                runtime.last_message = format!("connected {}", node.id);
                save_runtime(&state.cfg, &runtime).await.ok();
            }
            state
                .log("INFO", "openvpn", format!("connected {}", node.id))
                .await;

            Ok(format!("connected {}", node.id))
        }
        Ok(Err(err)) => {
            child.kill().await.ok();
            mark_failed(state, &node, err.to_string()).await;
            Err(err)
        }
        Err(_) => {
            child.kill().await.ok();
            let msg = format!("openvpn timeout after {}s", deadline.as_secs());
            mark_failed(state, &node, msg.clone()).await;
            bail!(msg)
        }
    }
}

pub async fn stop_openvpn(state: &AppState) -> anyhow::Result<()> {
    cleanup_policy_routing().await.ok();
    let mut guard = state.openvpn.lock().await;
    if let Some(child) = guard.child.as_mut() {
        child.kill().await.ok();
        child.wait().await.ok();
    }
    if let Some(path) = guard.config_file.take() {
        tokio::fs::remove_file(path).await.ok();
    }
    guard.child = None;

    {
        let mut nodes = state.nodes.write().await;
        for item in nodes.iter_mut() {
            if matches!(item.status, NodeStatus::Active) {
                item.status = NodeStatus::New;
            }
        }
        save_nodes(&state.cfg, &nodes).await.ok();
    }

    {
        let mut runtime = state.runtime.write().await;
        runtime.active_node_id = None;
        runtime.connecting = false;
        runtime.last_message = "disconnected".to_string();
        save_runtime(&state.cfg, &runtime).await.ok();
    }
    state.log("INFO", "openvpn", "disconnected").await;
    Ok(())
}

async fn spawn_openvpn(state: &AppState, config_file: &PathBuf) -> anyhow::Result<Child> {
    let mut cmd = Command::new(&state.cfg.openvpn_cmd);
    cmd.arg("--config")
        .arg(config_file)
        .arg("--dev")
        .arg(&state.cfg.tunnel_device)
        .arg("--dev-type")
        .arg("tun")
        .arg("--route-nopull")
        .arg("--pull-filter")
        .arg("ignore")
        .arg("route-ipv6")
        .arg("--pull-filter")
        .arg("ignore")
        .arg("ifconfig-ipv6")
        .arg("--route-delay")
        .arg("2")
        .arg("--connect-retry-max")
        .arg("1")
        .arg("--connect-timeout")
        .arg("15")
        .arg("--auth-user-pass")
        .arg(state.cfg.auth_file())
        .arg("--auth-nocache")
        .arg("--data-ciphers")
        .arg("AES-128-CBC:AES-256-GCM:AES-128-GCM:CHACHA20-POLY1305")
        .arg("--verb")
        .arg("3")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    cmd.spawn().context("failed to spawn openvpn")
}

async fn mark_failed(state: &AppState, node: &Node, message: String) {
    warn!("{message}");
    state.log("ERROR", "openvpn", message.clone()).await;
    {
        let mut nodes = state.nodes.write().await;
        if let Some(item) = nodes.iter_mut().find(|n| n.id == node.id) {
            item.status = NodeStatus::Failed;
            item.last_error = Some(message.clone());
        }
        save_nodes(&state.cfg, &nodes).await.ok();
    }
    {
        let mut runtime = state.runtime.write().await;
        runtime.connecting = false;
        runtime.active_node_id = None;
        runtime.last_message = message;
        save_runtime(&state.cfg, &runtime).await.ok();
    }
}

async fn setup_policy_routing(dev: &str) -> anyhow::Result<()> {
    run("ip", &["rule", "del", "table", "100"], false)
        .await
        .ok();
    run("ip", &["route", "flush", "table", "100"], false)
        .await
        .ok();
    run(
        "ip",
        &["route", "add", "default", "dev", dev, "table", "100"],
        true,
    )
    .await?;
    run("ip", &["rule", "add", "oif", dev, "table", "100"], true).await?;
    run("sysctl", &["-w", "net.ipv4.conf.all.rp_filter=2"], false)
        .await
        .ok();
    run(
        "sysctl",
        &["-w", "net.ipv4.conf.default.rp_filter=2"],
        false,
    )
    .await
    .ok();
    run(
        "sysctl",
        &["-w", &format!("net.ipv4.conf.{dev}.rp_filter=2")],
        false,
    )
    .await
    .ok();
    Ok(())
}

async fn cleanup_policy_routing() -> anyhow::Result<()> {
    run("ip", &["rule", "del", "table", "100"], false)
        .await
        .ok();
    run("ip", &["route", "flush", "table", "100"], false)
        .await
        .ok();
    Ok(())
}

async fn run(program: &str, args: &[&str], required: bool) -> anyhow::Result<()> {
    let output = Command::new(program).args(args).output().await;
    match output {
        Ok(output) if output.status.success() || !required => Ok(()),
        Ok(output) => bail!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(_err) if !required => Ok(()),
        Err(err) => Err(err.into()),
    }
}
