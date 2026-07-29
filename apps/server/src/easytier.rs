use crate::config::ServerConfig;
use crate::models::{
    CredentialResult, EasyTierStatus, MemberSnapshot, NetworkSecret, SiteSnapshot,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

const SHARED_RPC_PORT: u16 = 15888;
const MANAGER_RPC_BASE: u16 = 15900;

struct ManagedProcess {
    child: Child,
    rpc_port: u16,
}

#[derive(Default)]
struct RuntimeState {
    processes: HashMap<String, ManagedProcess>,
    last_error: String,
}

#[derive(Clone)]
pub struct EasyTierSupervisor {
    config: ServerConfig,
    state: Arc<Mutex<RuntimeState>>,
    apply_lock: Arc<Mutex<()>>,
}

#[derive(Deserialize)]
struct CliCredential {
    credential_id: String,
    credential_secret: String,
}

#[derive(Default, Deserialize)]
struct CliMember {
    #[serde(default)]
    id: String,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    ipv4: String,
    #[serde(default)]
    lat_ms: String,
    #[serde(default)]
    rx_bytes: String,
    #[serde(default)]
    tx_bytes: String,
    #[serde(default)]
    tunnel_proto: String,
    #[serde(default)]
    version: String,
}

impl EasyTierSupervisor {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(RuntimeState::default())),
            apply_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn apply(&self, snapshot: &SiteSnapshot) -> Result<(), String> {
        let _operation = self.apply_lock.lock().await;
        self.shutdown_inner().await;
        if !snapshot.initialized || self.config.easytier_disabled {
            return Ok(());
        }

        let shared = self.spawn_shared(snapshot).await?;
        {
            let mut state = self.state.lock().await;
            state.processes.insert("shared".into(), shared);
            state.last_error.clear();
        }
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;

        if snapshot.mode == "private" {
            for network in snapshot
                .networks
                .iter()
                .filter(|network| network.status == "active")
            {
                match self.spawn_manager(snapshot, network).await {
                    Ok(process) => {
                        self.state
                            .lock()
                            .await
                            .processes
                            .insert(network.id.clone(), process);
                    }
                    Err(error) => {
                        self.state.lock().await.last_error = error.clone();
                        self.shutdown_inner().await;
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    async fn spawn_shared(&self, snapshot: &SiteSnapshot) -> Result<ManagedProcess, String> {
        let mut args = vec![
            "--network-name".to_string(),
            snapshot.shared_name.clone(),
            "--hostname".to_string(),
            format!("{}-共享节点", snapshot.site_name),
            "--secure-mode".to_string(),
            "true".to_string(),
            "--local-private-key".to_string(),
            snapshot.shared_private_key.clone(),
            "--no-tun".to_string(),
            "--listeners".to_string(),
            format!("tcp://0.0.0.0:{}", self.config.easytier_port),
            format!("udp://0.0.0.0:{}", self.config.easytier_port),
            "--rpc-portal".to_string(),
            format!("127.0.0.1:{SHARED_RPC_PORT}"),
            "--rpc-portal-whitelist".to_string(),
            "127.0.0.0/8,::1/128".to_string(),
            "--disable-upnp".to_string(),
            "true".to_string(),
            "--relay-all-peer-rpc".to_string(),
            "true".to_string(),
        ];
        if snapshot.mode == "private" {
            let names: Vec<_> = snapshot
                .networks
                .iter()
                .filter(|network| network.status == "active")
                .map(|network| network.internal_name.clone())
                .collect();
            if !names.is_empty() {
                args.push("--relay-network-whitelist".into());
                args.extend(names);
            }
        }
        self.spawn("shared", SHARED_RPC_PORT, args).await
    }

    async fn spawn_manager(
        &self,
        snapshot: &SiteSnapshot,
        network: &NetworkSecret,
    ) -> Result<ManagedProcess, String> {
        let rpc_port = MANAGER_RPC_BASE + network.slot as u16;
        let credential_dir = self
            .config
            .data_dir
            .join("easytier/networks")
            .join(&network.id);
        std::fs::create_dir_all(&credential_dir).map_err(|error| error.to_string())?;
        let peer_config_path = credential_dir.join("peer.toml");
        let peer_config = format!(
            "[[peer]]\nuri = {}\npeer_public_key = {}\n",
            serde_json::to_string(&format!(
                "tcp://{}:{}",
                self.config.internal_easytier_host, self.config.easytier_port
            ))
            .map_err(|error| error.to_string())?,
            serde_json::to_string(&snapshot.shared_public_key).map_err(|error| error.to_string())?,
        );
        std::fs::write(&peer_config_path, peer_config).map_err(|error| error.to_string())?;
        let args = vec![
            "--instance-name".to_string(),
            format!("private-{}", network.id),
            "--network-name".to_string(),
            network.internal_name.clone(),
            "--network-secret".to_string(),
            network.internal_secret.clone(),
            "--hostname".to_string(),
            format!("{}-{}", snapshot.site_name, network.name),
            "--secure-mode".to_string(),
            "true".to_string(),
            "--local-private-key".to_string(),
            network.private_key.clone(),
            "--credential-file".to_string(),
            credential_dir
                .join("credentials.json")
                .to_string_lossy()
                .to_string(),
            "--no-listener".to_string(),
            "--no-tun".to_string(),
            "--config-file".to_string(),
            peer_config_path.to_string_lossy().to_string(),
            "--rpc-portal".to_string(),
            format!("127.0.0.1:{rpc_port}"),
            "--rpc-portal-whitelist".to_string(),
            "127.0.0.0/8,::1/128".to_string(),
            "--disable-upnp".to_string(),
            "true".to_string(),
        ];
        let process = self.spawn(&network.id, rpc_port, args).await?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(process)
    }

    async fn spawn(
        &self,
        name: &str,
        rpc_port: u16,
        args: Vec<String>,
    ) -> Result<ManagedProcess, String> {
        if !self.config.easytier_core.exists() {
            return Err(format!(
                "没有找到 EasyTier Core：{}",
                self.config.easytier_core.display()
            ));
        }
        let log_path = self
            .config
            .data_dir
            .join("logs")
            .join(format!("easytier-{name}.log"));
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|error| error.to_string())?;
        let stderr = stdout.try_clone().map_err(|error| error.to_string())?;
        let child = Command::new(&self.config.easytier_core)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| format!("启动 EasyTier {name} 失败：{error}"))?;
        Ok(ManagedProcess { child, rpc_port })
    }

    pub async fn issue_credential(
        &self,
        network: &NetworkSecret,
        credential_id: &str,
    ) -> Result<CredentialResult, String> {
        if self.config.easytier_disabled {
            return Ok(CredentialResult {
                credential_id: credential_id.to_string(),
                credential_secret: format!("test-credential-{credential_id}"),
            });
        }
        let rpc_port = MANAGER_RPC_BASE + network.slot as u16;
        let output = Command::new(&self.config.easytier_cli)
            .args([
                "-p",
                &format!("127.0.0.1:{rpc_port}"),
                "-o",
                "json",
                "credential",
                "generate",
                "--ttl",
                "31536000",
                "--credential-id",
                credential_id,
                "--reusable",
                "false",
            ])
            .output()
            .await
            .map_err(|error| format!("执行 EasyTier CLI 失败：{error}"))?;
        if !output.status.success() {
            return Err(format!(
                "签发 EasyTier 凭据失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let value: CliCredential = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("解析 EasyTier 凭据失败：{error}"))?;
        Ok(CredentialResult {
            credential_id: value.credential_id,
            credential_secret: value.credential_secret,
        })
    }

    pub async fn revoke_credential(
        &self,
        network: &NetworkSecret,
        credential_id: &str,
    ) -> Result<(), String> {
        if self.config.easytier_disabled {
            return Ok(());
        }
        let rpc_port = MANAGER_RPC_BASE + network.slot as u16;
        let output = Command::new(&self.config.easytier_cli)
            .args([
                "-p",
                &format!("127.0.0.1:{rpc_port}"),
                "-o",
                "json",
                "credential",
                "revoke",
                credential_id,
            ])
            .output()
            .await
            .map_err(|error| format!("执行 EasyTier CLI 失败：{error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "撤销 EasyTier 凭据失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }

    pub async fn observed_members(&self) -> Result<Vec<MemberSnapshot>, String> {
        if self.config.easytier_disabled {
            return Ok(Vec::new());
        }
        let output = Command::new(&self.config.easytier_cli)
            .args([
                "-p",
                &format!("127.0.0.1:{SHARED_RPC_PORT}"),
                "-o",
                "json",
                "peer",
                "list",
            ])
            .output()
            .await
            .map_err(|error| format!("查询 EasyTier 成员失败：{error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        let members: Vec<CliMember> = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("解析 EasyTier 成员失败：{error}"))?;
        Ok(members
            .into_iter()
            .map(|member| MemberSnapshot {
                id: member.id,
                hostname: member.hostname,
                ipv4: member.ipv4,
                latency: member.lat_ms,
                protocol: member.tunnel_proto,
                rx_bytes: member.rx_bytes,
                tx_bytes: member.tx_bytes,
                version: member.version,
            })
            .collect())
    }

    pub async fn status(&self) -> EasyTierStatus {
        if self.config.easytier_disabled {
            return EasyTierStatus {
                running: true,
                healthy: true,
                disabled_for_test: true,
                ..Default::default()
            };
        }
        let mut state = self.state.lock().await;
        let mut shared_running = false;
        let mut shared_rpc_port = None;
        let mut manager_total = 0;
        let mut manager_running = 0;
        let mut manager_rpc_ports = Vec::new();
        for (name, process) in state.processes.iter_mut() {
            let running = process.child.try_wait().ok().flatten().is_none();
            if name == "shared" {
                shared_running = running;
                if running {
                    shared_rpc_port = Some(process.rpc_port);
                }
            } else {
                manager_total += 1;
                if running {
                    manager_running += 1;
                    manager_rpc_ports.push(process.rpc_port);
                }
            }
        }
        let last_error = state.last_error.clone();
        drop(state);

        let shared_ready = match shared_rpc_port {
            Some(port) => self.rpc_has_peers(port, 1).await,
            None => false,
        };
        let mut managers_ready = true;
        for port in manager_rpc_ports {
            if !self.rpc_has_peers(port, 2).await {
                managers_ready = false;
                break;
            }
        }
        let healthy = shared_ready && manager_running == manager_total && managers_ready;
        EasyTierStatus {
            running: shared_running,
            healthy,
            disabled_for_test: false,
            shared_running,
            manager_total,
            manager_running,
            last_error: if !healthy && last_error.is_empty() {
                "EasyTier RPC 尚未就绪或管理节点未接入共享节点".into()
            } else {
                last_error
            },
        }
    }

    async fn rpc_has_peers(&self, rpc_port: u16, minimum: usize) -> bool {
        let command = Command::new(&self.config.easytier_cli)
            .args([
                "-p",
                &format!("127.0.0.1:{rpc_port}"),
                "-o",
                "json",
                "peer",
                "list",
            ])
            .output();
        let Ok(Ok(output)) = tokio::time::timeout(std::time::Duration::from_secs(3), command).await
        else {
            return false;
        };
        output.status.success()
            && serde_json::from_slice::<Vec<CliMember>>(&output.stdout)
                .is_ok_and(|members| members.len() >= minimum)
    }

    pub async fn shutdown(&self) {
        let _operation = self.apply_lock.lock().await;
        self.shutdown_inner().await;
    }

    async fn shutdown_inner(&self) {
        let mut state = self.state.lock().await;
        for process in state.processes.values_mut() {
            let _ = process.child.kill().await;
            let _ = process.child.wait().await;
        }
        state.processes.clear();
    }

    pub async fn rpc_ports(&self) -> HashMap<String, u16> {
        self.state
            .lock()
            .await
            .processes
            .iter()
            .map(|(name, process)| (name.clone(), process.rpc_port))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn disabled_supervisor_issues_deterministic_test_credential() {
        let directory = tempdir().unwrap();
        let supervisor = EasyTierSupervisor::new(ServerConfig {
            web_port: 17280,
            easytier_port: 11010,
            data_dir: directory.path().to_path_buf(),
            web_dir: directory.path().to_path_buf(),
            easytier_core: "missing".into(),
            easytier_cli: "missing".into(),
            internal_easytier_host: "127.0.0.1".into(),
            easytier_disabled: true,
        });
        let network = NetworkSecret {
            id: "network".into(),
            name: "测试".into(),
            internal_name: "internal".into(),
            internal_secret: "secret".into(),
            private_key: "key".into(),
            status: "active".into(),
            slot: 0,
        };
        let credential = supervisor
            .issue_credential(&network, "credential-id")
            .await
            .unwrap();
        assert_eq!(credential.credential_id, "credential-id");
        assert!(credential.credential_secret.contains("credential-id"));
    }
}
