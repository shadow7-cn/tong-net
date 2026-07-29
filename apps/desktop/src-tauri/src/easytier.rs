use crate::config::app_data_dir;
use crate::easytier_service::{self, ServiceNetworkConfig};
use aes_gcm::{
    aead::{rand_core::RngCore, Aead, OsRng},
    Aes256Gcm, KeyInit, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

const MAX_LOG_LINES: usize = 300;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EasyTierConfig {
    server_url: String,
    network_name: String,
    network_password: String,
    device_name: String,
    allow_insecure_http: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredEasyTierConfig {
    #[serde(default)]
    server_url: String,
    network_name: String,
    #[serde(alias = "encryptedNetworkSecret")]
    encrypted_network_password: String,
    device_name: String,
    #[serde(default)]
    allow_insecure_http: bool,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EasyTierMember {
    id: String,
    hostname: String,
    ipv4: String,
    cost: String,
    latency: String,
    loss_rate: String,
    rx_bytes: String,
    tx_bytes: String,
    protocol: String,
    nat_type: String,
    version: String,
    local: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct EasyTierCliMember {
    id: String,
    hostname: String,
    ipv4: String,
    cost: String,
    lat_ms: String,
    loss_rate: String,
    rx_bytes: String,
    tx_bytes: String,
    tunnel_proto: String,
    nat_type: String,
    version: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EasyTierStatus {
    running: bool,
    connected: bool,
    phase: String,
    network_name: String,
    device_name: String,
    virtual_ip: String,
    server_mode: String,
    server_url: String,
    insecure_http: bool,
    members: Vec<EasyTierMember>,
    logs: Vec<String>,
}

struct EasyTierInner {
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    child: Option<CommandChild>,
    running: bool,
    phase: String,
    network_name: String,
    device_name: String,
    virtual_ip: String,
    server_mode: String,
    server_url: String,
    session_token: String,
    last_heartbeat: Option<Instant>,
    members: Vec<EasyTierMember>,
    ever_connected: bool,
    logs: VecDeque<String>,
    log_path: Option<PathBuf>,
    pid_path: Option<PathBuf>,
}

pub struct EasyTierRuntime {
    inner: Arc<Mutex<EasyTierInner>>,
}

impl Default for EasyTierRuntime {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EasyTierInner {
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                child: None,
                running: false,
                phase: "未连接".into(),
                network_name: String::new(),
                device_name: String::new(),
                virtual_ip: String::new(),
                server_mode: String::new(),
                server_url: String::new(),
                session_token: String::new(),
                last_heartbeat: None,
                members: Vec::new(),
                ever_connected: false,
                logs: VecDeque::new(),
                log_path: None,
                pid_path: None,
            })),
        }
    }
}

fn config_path(directory: &Path) -> PathBuf {
    directory.join("easytier-config.json")
}

fn key_path(directory: &Path) -> PathBuf {
    directory.join("easytier-config.key")
}

fn runtime_paths() -> (PathBuf, PathBuf, PathBuf) {
    let directory = app_data_dir().join("easytier-runtime");
    (
        directory.join("core.pid"),
        directory.join("core.log"),
        directory.join("network-secret"),
    )
}

fn device_id_path() -> PathBuf {
    app_data_dir().join("easytier-device-id")
}

fn load_or_create_device_id() -> Result<String, String> {
    let path = device_id_path();
    if let Ok(value) = std::fs::read_to_string(&path) {
        let value = value.trim();
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let value = uuid::Uuid::new_v4().to_string();
    std::fs::write(&path, &value).map_err(|error| error.to_string())?;
    Ok(value)
}

fn load_or_create_key(directory: &Path) -> Result<[u8; 32], String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let path = key_path(directory);
    if let Ok(bytes) = std::fs::read(&path) {
        return bytes
            .try_into()
            .map_err(|_| "EasyTier 配置密钥长度无效".to_string());
    }
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    std::fs::write(&path, key).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    Ok(key)
}

fn encrypt_secret(directory: &Path, secret: &str) -> Result<String, String> {
    let key = load_or_create_key(directory)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|error| error.to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), secret.as_bytes())
        .map_err(|_| "加密 EasyTier 密码失败".to_string())?;
    let mut payload = nonce_bytes.to_vec();
    payload.extend(ciphertext);
    Ok(STANDARD.encode(payload))
}

fn decrypt_secret(directory: &Path, payload: &str) -> Result<String, String> {
    let bytes = STANDARD
        .decode(payload)
        .map_err(|_| "EasyTier 密码密文格式无效".to_string())?;
    if bytes.len() <= 12 {
        return Err("EasyTier 密码密文长度无效".into());
    }
    let key = load_or_create_key(directory)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|error| error.to_string())?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&bytes[..12]), &bytes[12..])
        .map_err(|_| "无法解密 EasyTier 密码".to_string())?;
    String::from_utf8(plaintext).map_err(|_| "EasyTier 密码不是有效文本".to_string())
}

fn save_config_at(directory: &Path, config: &EasyTierConfig) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let stored = StoredEasyTierConfig {
        server_url: config.server_url.trim().trim_end_matches('/').into(),
        network_name: config.network_name.trim().into(),
        encrypted_network_password: encrypt_secret(directory, &config.network_password)?,
        device_name: config.device_name.trim().into(),
        allow_insecure_http: config.allow_insecure_http,
    };
    let value = serde_json::to_vec_pretty(&stored).map_err(|error| error.to_string())?;
    std::fs::write(config_path(directory), value).map_err(|error| error.to_string())
}

fn load_config_at(directory: &Path) -> Result<EasyTierConfig, String> {
    let value = std::fs::read(config_path(directory)).map_err(|error| error.to_string())?;
    let stored: StoredEasyTierConfig =
        serde_json::from_slice(&value).map_err(|error| error.to_string())?;
    Ok(EasyTierConfig {
        server_url: stored.server_url,
        network_name: stored.network_name,
        network_password: decrypt_secret(directory, &stored.encrypted_network_password)?,
        device_name: stored.device_name,
        allow_insecure_http: stored.allow_insecure_http,
    })
}

fn save_config(config: &EasyTierConfig) -> Result<(), String> {
    save_config_at(&app_data_dir(), config)
}

fn load_config() -> Result<EasyTierConfig, String> {
    let directory = app_data_dir();
    if !config_path(&directory).exists() {
        return Ok(EasyTierConfig::default());
    }
    load_config_at(&directory)
}

fn push_log(inner: &mut EasyTierInner, line: String) {
    if line.trim().is_empty() {
        return;
    }
    inner.logs.push_back(line);
    while inner.logs.len() > MAX_LOG_LINES {
        inner.logs.pop_front();
    }
}

fn peers_for_address(value: &str) -> Result<[String; 2], String> {
    let address = value.trim();
    if address.is_empty() || address.contains("://") || address.chars().any(char::is_whitespace) {
        return Err("连接地址必须是有效的域名或 IP 加端口".into());
    }
    let (host, port) = address
        .rsplit_once(':')
        .ok_or_else(|| "连接地址必须包含端口".to_string())?;
    if host.is_empty() || port.parse::<u16>().is_err() {
        return Err("连接地址必须是有效的域名或 IP 加端口".into());
    }
    Ok([format!("tcp://{address}"), format!("udp://{address}")])
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerInfo {
    initialized: bool,
    mode: String,
    version: String,
    minimum_desktop_version: String,
    public_host: String,
    easytier_port: u16,
    shared_public_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrivateConnectRequest {
    network_name: String,
    network_password: String,
    client_device_id: String,
    device_name: String,
    platform: String,
    client_version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivateConnectResponse {
    session_token: String,
    network: PrivateNetworkConfig,
}

#[derive(Deserialize)]
struct PrivateNetworkConfig {
    name: String,
    credential: String,
    peers: Vec<String>,
    #[serde(rename = "peerPublicKey")]
    _peer_public_key: String,
}

struct PreparedConnection {
    service: ServiceNetworkConfig,
    mode: String,
    server_url: String,
    session_token: String,
}

fn normalize_server_url(value: &str) -> Result<String, String> {
    let value = value.trim().trim_end_matches('/');
    let url = reqwest::Url::parse(value)
        .map_err(|_| "服务端地址无效，请填写完整的 http:// 或 https:// 地址".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("服务端地址无效，请填写完整的 http:// 或 https:// 地址".into());
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err("服务端地址不能包含路径、查询参数或片段".into());
    }
    Ok(value.to_string())
}

fn version_tuple(value: &str) -> Option<(u32, u32, u32)> {
    let mut values = value.trim_start_matches('v').split('.');
    Some((
        values.next()?.parse().ok()?,
        values.next()?.parse().ok()?,
        values.next()?.split('-').next()?.parse().ok()?,
    ))
}

async fn prepare_connection(config: &EasyTierConfig) -> Result<PreparedConnection, String> {
    let server_url = normalize_server_url(&config.server_url)?;
    if server_url.starts_with("http://") && !config.allow_insecure_http {
        return Err("INSECURE_HTTP_CONFIRM_REQUIRED".into());
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?;
    let info = client
        .get(format!("{server_url}/api/v1/info"))
        .send()
        .await
        .map_err(|error| format!("无法连接组网服务端：{error}"))?
        .error_for_status()
        .map_err(|error| format!("读取组网服务信息失败：{error}"))?
        .json::<ServerInfo>()
        .await
        .map_err(|error| format!("解析组网服务信息失败：{error}"))?;
    if !info.initialized {
        return Err("组网服务端尚未完成首次设置".into());
    }
    if version_tuple(env!("CARGO_PKG_VERSION")) < version_tuple(&info.minimum_desktop_version) {
        return Err(format!(
            "桌面端版本过低，服务端要求至少使用 {}",
            info.minimum_desktop_version
        ));
    }
    if version_tuple(&info.version).is_none() {
        return Err("组网服务端版本格式无效".into());
    }

    if info.mode == "public" {
        let server_address = format!("{}:{}", info.public_host, info.easytier_port);
        peers_for_address(&server_address)?;
        return Ok(PreparedConnection {
            service: ServiceNetworkConfig {
                network_name: config.network_name.trim().into(),
                auth_type: "network_secret".into(),
                auth_secret: config.network_password.clone(),
                device_name: config.device_name.trim().into(),
                server_address,
                peer_public_key: info.shared_public_key,
            },
            mode: "public".into(),
            server_url,
            session_token: String::new(),
        });
    }
    if info.mode != "private" {
        return Err("组网服务端返回了不支持的节点模式".into());
    }

    let response = client
        .post(format!("{server_url}/api/v1/private/connect"))
        .json(&PrivateConnectRequest {
            network_name: config.network_name.trim().into(),
            network_password: config.network_password.clone(),
            client_device_id: load_or_create_device_id()?,
            device_name: config.device_name.trim().into(),
            platform: std::env::consts::OS.into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
        })
        .send()
        .await
        .map_err(|error| format!("连接私有网络失败：{error}"))?;
    if !response.status().is_success() {
        let message = response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|value| value["message"].as_str().map(str::to_string))
            .unwrap_or_else(|| "网络名称或密码无效".into());
        return Err(message);
    }
    let response = response
        .json::<PrivateConnectResponse>()
        .await
        .map_err(|error| format!("解析私有网络凭据失败：{error}"))?;
    let server_address = response
        .network
        .peers
        .first()
        .and_then(|value| value.split_once("://").map(|(_, address)| address))
        .ok_or_else(|| "服务端没有返回有效的 EasyTier 连接地址".to_string())?
        .to_string();
    peers_for_address(&server_address)?;
    Ok(PreparedConnection {
        service: ServiceNetworkConfig {
            network_name: response.network.name,
            auth_type: "credential".into(),
            auth_secret: response.network.credential,
            device_name: config.device_name.trim().into(),
            server_address,
            // EasyTier 2.6.4 rejects a credential-only node that pins a shared
            // node before the admin node has propagated its trusted key list.
            peer_public_key: String::new(),
        },
        mode: "private".into(),
        server_url,
        session_token: response.session_token,
    })
}

fn send_heartbeat(server_url: &str, token: &str, virtual_ip: &str) -> Result<(), String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
        .map_err(|error| error.to_string())?
        .post(format!("{server_url}/api/v1/private/heartbeat"))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "virtualIp": virtual_ip,
            "clientVersion": env!("CARGO_PKG_VERSION")
        }))
        .send()
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    Ok(())
}

async fn disconnect_private_session(server_url: &str, token: &str) {
    if let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .build()
    {
        let _ = client
            .post(format!("{server_url}/api/v1/private/disconnect"))
            .bearer_auth(token)
            .json(&serde_json::json!({}))
            .send()
            .await;
    }
}

fn status_from_inner(inner: &EasyTierInner) -> EasyTierStatus {
    EasyTierStatus {
        running: inner.running,
        connected: inner.running && !inner.virtual_ip.is_empty(),
        phase: inner.phase.clone(),
        network_name: inner.network_name.clone(),
        device_name: inner.device_name.clone(),
        virtual_ip: inner.virtual_ip.clone(),
        server_mode: inner.server_mode.clone(),
        server_url: inner.server_url.clone(),
        insecure_http: inner.server_url.starts_with("http://"),
        members: inner.members.clone(),
        logs: inner.logs.iter().cloned().collect(),
    }
}

fn apply_member_snapshot(
    inner: &mut EasyTierInner,
    virtual_ip: String,
    members: Vec<EasyTierMember>,
) {
    inner.virtual_ip = virtual_ip;
    inner.members = members;
    if inner.virtual_ip.is_empty() {
        inner.phase = if inner.ever_connected {
            "连接中断，正在重连".into()
        } else {
            "正在获取虚拟 IP".into()
        };
    } else {
        inner.ever_connected = true;
        inner.phase = "已连接".into();
    }
}

fn bundled_binary_path(base_name: &str) -> Result<PathBuf, String> {
    let installed_name = if cfg!(target_os = "windows") {
        format!("{base_name}.exe")
    } else {
        base_name.to_string()
    };
    let installed = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .parent()
        .map(|parent| parent.join(&installed_name))
        .ok_or_else(|| "无法定位同网互通程序目录".to_string())?;
    if installed.exists() {
        return Ok(installed);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let triple = "aarch64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let triple = "x86_64-apple-darwin";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    let triple = "x86_64-pc-windows-msvc";
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    return Err("当前平台尚未准备 EasyTier 内置程序".into());

    #[cfg(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64")
    ))]
    {
        let extension = if cfg!(target_os = "windows") {
            ".exe"
        } else {
            ""
        };
        let development = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(format!("{base_name}-{triple}{extension}"));
        development
            .exists()
            .then_some(development)
            .ok_or_else(|| format!("没有找到内置 {base_name}，请先执行 npm run prepare:easytier"))
    }
}

fn query_members() -> Result<(String, Vec<EasyTierMember>), String> {
    let mut command = std::process::Command::new(bundled_binary_path("easytier-cli")?);
    command.args(["-p", "127.0.0.1:17282", "-o", "json", "peer", "list"]);
    let output = run_command_with_timeout(&mut command, Duration::from_millis(1200))
        .map_err(|error| format!("无法查询 EasyTier RPC：{error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    parse_members(&output.stdout)
}

fn run_command_with_timeout(
    command: &mut std::process::Command,
    timeout: Duration,
) -> Result<std::process::Output, String> {
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(_) => return child.wait_with_output().map_err(|error| error.to_string()),
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("查询超时".into());
            }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

fn parse_members(output: &[u8]) -> Result<(String, Vec<EasyTierMember>), String> {
    let cli_members: Vec<EasyTierCliMember> =
        serde_json::from_slice(output).map_err(|error| format!("解析成员列表失败：{error}"))?;
    let mut virtual_ip = String::new();
    let members = cli_members
        .into_iter()
        .map(|member| {
            let local = member.cost == "Local";
            let member = EasyTierMember {
                id: member.id,
                hostname: member.hostname,
                ipv4: member.ipv4,
                cost: member.cost,
                latency: member.lat_ms,
                loss_rate: member.loss_rate,
                rx_bytes: member.rx_bytes,
                tx_bytes: member.tx_bytes,
                protocol: member.tunnel_proto,
                nat_type: member.nat_type,
                version: member.version,
                local,
            };
            if member.local {
                virtual_ip = member.ipv4.clone();
            }
            member
        })
        .collect();
    Ok((virtual_ip, members))
}

fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()
            .map(|output| output.status.success() && !output.stdout.is_empty())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
            })
            .unwrap_or(false)
    }
}

fn is_easytier_process(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "command="])
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).contains("easytier-core")
            })
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout)
                        .to_ascii_lowercase()
                        .contains("easytier-core")
            })
            .unwrap_or(false)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn kill_process(pid: u32) -> Result<(), String> {
    let status = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
    status
        .map_err(|error| error.to_string())
        .and_then(|status| {
            status
                .success()
                .then_some(())
                .ok_or_else(|| "结束 EasyTier Core 失败".into())
        })
}

fn sync_residual_runtime_at(
    inner: &mut EasyTierInner,
    pid_path: PathBuf,
    log_path: PathBuf,
    config: EasyTierConfig,
) {
    if inner.running {
        return;
    }
    let Some(pid) = read_pid(&pid_path) else {
        return;
    };
    if !is_process_alive(pid) || !is_easytier_process(pid) {
        let _ = std::fs::remove_file(pid_path);
        return;
    }
    inner.running = true;
    inner.phase = "已接管残留 Core".into();
    inner.network_name = config.network_name;
    inner.device_name = config.device_name;
    inner.pid_path = Some(pid_path);
    inner.log_path = Some(log_path);
    push_log(
        inner,
        format!("检测并接管仍在运行的 EasyTier Core（PID {pid}）"),
    );
}

fn sync_residual_runtime(inner: &mut EasyTierInner) {
    let (pid_path, log_path, _) = runtime_paths();
    sync_residual_runtime_at(inner, pid_path, log_path, load_config().unwrap_or_default());
}

#[tauri::command]
pub fn get_easytier_config() -> Result<EasyTierConfig, String> {
    load_config()
}

#[tauri::command]
pub fn save_easytier_config(config: EasyTierConfig) -> Result<EasyTierConfig, String> {
    validate_config(&config)?;
    save_config(&config)?;
    Ok(config)
}

#[tauri::command]
pub fn get_easytier_status(
    state: tauri::State<'_, EasyTierRuntime>,
) -> Result<EasyTierStatus, String> {
    let should_query = {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "EasyTier 状态不可用".to_string())?;
        sync_residual_runtime(&mut inner);
        inner.running
    };
    let rpc_status = should_query.then(query_members);
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "EasyTier 状态不可用".to_string())?;
    if let Some(log_path) = inner.log_path.clone() {
        if let Ok(content) = std::fs::read_to_string(log_path) {
            inner.logs.clear();
            for line in content.lines() {
                push_log(&mut inner, line.to_string());
            }
        }
    }
    if let Some(result) = rpc_status {
        match result {
            Ok((virtual_ip, members)) => {
                apply_member_snapshot(&mut inner, virtual_ip, members);
            }
            Err(_) => {
                if let Some(pid) = inner.pid_path.as_deref().and_then(read_pid) {
                    if !is_process_alive(pid) || !is_easytier_process(pid) {
                        inner.running = false;
                        inner.virtual_ip.clear();
                        inner.members.clear();
                        inner.phase = "Core 异常退出".into();
                        let _ = std::fs::remove_file(inner.pid_path.as_ref().unwrap());
                        inner.pid_path = None;
                    } else if inner.ever_connected {
                        inner.virtual_ip.clear();
                        inner.members.clear();
                        inner.phase = "连接中断，正在重连".into();
                    }
                }
            }
        }
    }
    let heartbeat = if inner.running
        && inner.server_mode == "private"
        && !inner.session_token.is_empty()
        && inner
            .last_heartbeat
            .is_none_or(|value| value.elapsed() >= Duration::from_secs(10))
    {
        inner.last_heartbeat = Some(Instant::now());
        Some((
            inner.server_url.clone(),
            inner.session_token.clone(),
            inner.virtual_ip.clone(),
        ))
    } else {
        None
    };
    let status = status_from_inner(&inner);
    drop(inner);
    if let Some((server_url, token, virtual_ip)) = heartbeat {
        if let Err(error) = send_heartbeat(&server_url, &token, &virtual_ip) {
            if let Ok(mut inner) = state.inner.lock() {
                push_log(&mut inner, format!("服务端心跳失败：{error}"));
            }
        }
    }
    Ok(status)
}

#[tauri::command]
pub async fn start_easytier(
    _app: tauri::AppHandle,
    state: tauri::State<'_, EasyTierRuntime>,
    config: EasyTierConfig,
) -> Result<EasyTierStatus, String> {
    validate_config(&config)?;
    let prepared = prepare_connection(&config).await?;

    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "EasyTier 状态不可用".to_string())?;
        sync_residual_runtime(&mut inner);
        if inner.running {
            return Err("EasyTier 已经在运行".into());
        }
    }
    save_config(&config)?;

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let runtime_dir = app_data_dir().join("easytier-runtime");
        easytier_service::ensure_installed(
            bundled_binary_path("easytier-core")?,
            runtime_dir.clone(),
        )
        .await?;
        easytier_service::start(&runtime_dir, prepared.service.clone())?;
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        let (pid_path, log_path, _) = runtime_paths();
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "EasyTier 状态不可用".to_string())?;
        inner.running = true;
        inner.phase = "正在连接".into();
        inner.network_name = config.network_name.trim().into();
        inner.device_name = config.device_name.trim().into();
        inner.virtual_ip.clear();
        inner.server_mode = prepared.mode;
        inner.server_url = prepared.server_url;
        inner.session_token = prepared.session_token;
        inner.last_heartbeat = None;
        inner.members.clear();
        inner.ever_connected = false;
        inner.logs.clear();
        inner.log_path = Some(log_path);
        inner.pid_path = Some(pid_path);
        push_log(&mut inner, "已通过特权服务启动 EasyTier Core".into());
        Ok(status_from_inner(&inner))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let peers = peers_for_address(&prepared.service.server_address)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut args = vec![
        "--network-name".to_string(),
        prepared.service.network_name.clone(),
        "--dhcp".to_string(),
        "true".to_string(),
        "--hostname".to_string(),
        config.device_name.trim().to_string(),
        "--no-listener".to_string(),
        "--rpc-portal".to_string(),
        "127.0.0.1:17282".to_string(),
        "--secure-mode".to_string(),
        "true".to_string(),
    ];
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    for peer in peers {
        args.push("--peers".into());
        args.push(peer);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    if prepared.service.auth_type == "credential" {
        args.push("--credential".into());
        args.push(prepared.service.auth_secret.clone());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = _app
        .shell()
        .sidecar("easytier-core")
        .map_err(|error| format!("无法加载内置 EasyTier Core：{error}"))?
        .args(args);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    if prepared.service.auth_type == "network_secret" {
        command = command.env("ET_NETWORK_SECRET", prepared.service.auth_secret.clone());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let (mut receiver, child) = command
        .spawn()
        .map_err(|error| format!("无法启动内置 EasyTier Core：{error}"))?;

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let core_pid = child.pid();
        let (pid_path, _, _) = runtime_paths();
        std::fs::create_dir_all(pid_path.parent().unwrap()).map_err(|error| error.to_string())?;
        std::fs::write(&pid_path, core_pid.to_string()).map_err(|error| error.to_string())?;

        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "EasyTier 状态不可用".to_string())?;
        inner.child = Some(child);
        inner.running = true;
        inner.phase = "正在连接".into();
        inner.network_name = config.network_name.trim().into();
        inner.device_name = config.device_name.trim().into();
        inner.virtual_ip.clear();
        inner.server_mode = prepared.mode;
        inner.server_url = prepared.server_url;
        inner.session_token = prepared.session_token;
        inner.last_heartbeat = None;
        inner.members.clear();
        inner.ever_connected = false;
        inner.logs.clear();
        inner.pid_path = Some(pid_path);
        push_log(&mut inner, "已启动同网互通内置 EasyTier Core".into());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let runtime = state.inner.clone();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    tauri::async_runtime::spawn(async move {
        while let Some(event) = receiver.recv().await {
            let mut inner = match runtime.lock() {
                Ok(value) => value,
                Err(_) => return,
            };
            match event {
                CommandEvent::Stdout(bytes) | CommandEvent::Stderr(bytes) => {
                    let output = String::from_utf8_lossy(&bytes);
                    for line in output.lines() {
                        let clean = line.replace('\u{1b}', "");
                        if clean.contains("peer") || clean.contains("Peer") || clean.contains("10.")
                        {
                            inner.phase = "已建立网络连接".into();
                        }
                        push_log(&mut inner, clean);
                    }
                }
                CommandEvent::Terminated(payload) => {
                    inner.running = false;
                    inner.child = None;
                    if let Some(path) = inner.pid_path.take() {
                        let _ = std::fs::remove_file(path);
                    }
                    inner.phase = format!("已停止（退出码 {:?}）", payload.code);
                }
                CommandEvent::Error(error) => {
                    push_log(&mut inner, format!("运行错误：{error}"));
                    inner.phase = "运行错误".into();
                }
                _ => {}
            }
        }
    });

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    get_easytier_status(state)
}

#[tauri::command]
pub async fn stop_easytier(
    state: tauri::State<'_, EasyTierRuntime>,
) -> Result<EasyTierStatus, String> {
    let private_session = state
        .inner
        .lock()
        .ok()
        .filter(|inner| inner.server_mode == "private" && !inner.session_token.is_empty())
        .map(|inner| (inner.server_url.clone(), inner.session_token.clone()));

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let runtime_dir = app_data_dir().join("easytier-runtime");
        if easytier_service::status(&runtime_dir).is_err() {
            easytier_service::ensure_installed(
                bundled_binary_path("easytier-core")?,
                runtime_dir.clone(),
            )
            .await?;
        }
        easytier_service::stop(&runtime_dir)?;
        let status = {
            let mut inner = state
                .inner
                .lock()
                .map_err(|_| "EasyTier 状态不可用".to_string())?;
            inner.running = false;
            inner.phase = "已停止".into();
            inner.virtual_ip.clear();
            inner.members.clear();
            inner.ever_connected = false;
            inner.server_mode.clear();
            inner.session_token.clear();
            inner.last_heartbeat = None;
            inner.pid_path = None;
            push_log(&mut inner, "用户已停止 EasyTier".into());
            status_from_inner(&inner)
        };
        if let Some((server_url, token)) = private_session {
            disconnect_private_session(&server_url, &token).await;
        }
        Ok(status)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let status = {
            let mut inner = state
                .inner
                .lock()
                .map_err(|_| "EasyTier 状态不可用".to_string())?;
            let managed_child = inner.child.is_some();
            if let Some(child) = inner.child.take() {
                child
                    .kill()
                    .map_err(|error| format!("停止 EasyTier 失败：{error}"))?;
            }
            if !managed_child {
                if let Some(pid) = inner.pid_path.as_deref().and_then(read_pid) {
                    if is_easytier_process(pid) {
                        kill_process(pid)?;
                    }
                }
            }
            inner.running = false;
            inner.phase = "已停止".into();
            inner.virtual_ip.clear();
            inner.members.clear();
            inner.ever_connected = false;
            inner.server_mode.clear();
            inner.session_token.clear();
            inner.last_heartbeat = None;
            if let Some(path) = inner.pid_path.take() {
                let _ = std::fs::remove_file(path);
            }
            push_log(&mut inner, "用户已停止 EasyTier".into());
            status_from_inner(&inner)
        };
        if let Some((server_url, token)) = private_session {
            disconnect_private_session(&server_url, &token).await;
        }
        Ok(status)
    }
}

fn validate_config(config: &EasyTierConfig) -> Result<(), String> {
    if config.server_url.trim().is_empty()
        || config.network_name.trim().is_empty()
        || config.network_password.is_empty()
        || config.device_name.trim().is_empty()
    {
        return Err("请填写服务端地址、网络名称、网络密码和设备名称".into());
    }
    normalize_server_url(&config.server_url)?;
    Ok(())
}

pub fn cleanup_on_exit(_runtime: &EasyTierRuntime) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let _ = easytier_service::stop(&app_data_dir().join("easytier-runtime"));
        if let Ok(mut inner) = _runtime.inner.lock() {
            inner.running = false;
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    if let Ok(mut inner) = _runtime.inner.lock() {
        let managed_child = inner.child.is_some();
        if let Some(child) = inner.child.take() {
            let _ = child.kill();
        }
        if !managed_child {
            if let Some(pid) = inner.pid_path.as_deref().and_then(read_pid) {
                if is_easytier_process(pid) {
                    let _ = kill_process(pid);
                }
            }
        }
        if let Some(path) = inner.pid_path.take() {
            let _ = std::fs::remove_file(path);
        }
        inner.running = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_member_snapshot, config_path, is_easytier_process, is_process_alive, load_config_at,
        parse_members, peers_for_address, run_command_with_timeout, save_config_at,
        sync_residual_runtime_at, EasyTierConfig, EasyTierInner,
    };
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    #[test]
    fn expands_one_ipv4_address_to_tcp_and_udp() {
        assert_eq!(
            peers_for_address("203.0.113.10:11010").unwrap(),
            ["tcp://203.0.113.10:11010", "udp://203.0.113.10:11010"]
        );
    }

    #[test]
    fn rejects_protocol_and_missing_port() {
        assert!(peers_for_address("tcp://203.0.113.10:11010").is_err());
        assert!(peers_for_address("203.0.113.10").is_err());
    }

    #[test]
    fn parses_virtual_ip_and_members_from_cli_json() {
        let json = br#"[
          {
            "cidr": "10.126.126.2/24",
            "ipv4": "10.126.126.2",
            "hostname": "local",
            "cost": "Local",
            "lat_ms": "-",
            "loss_rate": "-",
            "rx_bytes": "-",
            "tx_bytes": "-",
            "tunnel_proto": "-",
            "nat_type": "Unknown",
            "id": "1",
            "version": "2.6.4"
          },
          {
            "ipv4": "10.126.126.1",
            "hostname": "server",
            "cost": "p2p",
            "lat_ms": "12.3",
            "loss_rate": "0",
            "rx_bytes": "1.2 kB",
            "tx_bytes": "2.3 kB",
            "tunnel_proto": "udp",
            "nat_type": "OpenInternet",
            "id": "2",
            "version": "2.6.4"
          }
        ]"#;
        let (virtual_ip, members) = parse_members(json).unwrap();
        assert_eq!(virtual_ip, "10.126.126.2");
        assert_eq!(members.len(), 2);
        assert!(members[0].local);
        assert_eq!(members[1].protocol, "udp");
        let frontend_member = serde_json::to_value(&members[1]).unwrap();
        assert_eq!(frontend_member["protocol"], "udp");
        assert_eq!(frontend_member["latency"], "12.3");
        assert_eq!(frontend_member["rxBytes"], "1.2 kB");
    }

    #[test]
    fn encrypts_saved_password_and_can_restore_config() {
        let directory = tempfile::tempdir().unwrap();
        let config = EasyTierConfig {
            server_url: "http://127.0.0.1:17280".into(),
            network_name: "test-net".into(),
            network_password: "plain-secret-123".into(),
            device_name: "desktop".into(),
            allow_insecure_http: true,
        };

        save_config_at(directory.path(), &config).unwrap();
        let first_file = std::fs::read_to_string(config_path(directory.path())).unwrap();
        assert!(!first_file.contains("plain-secret-123"));
        assert!(first_file.contains("encryptedNetworkPassword"));

        let restored = load_config_at(directory.path()).unwrap();
        assert_eq!(restored.network_name, config.network_name);
        assert_eq!(restored.network_password, config.network_password);
        assert_eq!(restored.device_name, config.device_name);
        assert_eq!(restored.server_url, config.server_url);
        assert!(restored.allow_insecure_http);

        save_config_at(directory.path(), &config).unwrap();
        let second_file = std::fs::read_to_string(config_path(directory.path())).unwrap();
        assert_ne!(first_file, second_file);
    }

    #[test]
    fn detects_current_process_and_rejects_missing_pid() {
        assert!(is_process_alive(std::process::id()));
        assert!(!is_easytier_process(std::process::id()));
        assert!(!is_process_alive(u32::MAX));
    }

    #[cfg(unix)]
    #[test]
    fn terminates_an_rpc_command_that_exceeds_its_timeout() {
        let started = Instant::now();
        let result = run_command_with_timeout(
            std::process::Command::new("/bin/sh").args(["-c", "sleep 5"]),
            Duration::from_millis(80),
        );

        assert_eq!(result.unwrap_err(), "查询超时");
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn reports_reconnecting_after_a_connected_network_loses_its_ip() {
        let mut inner = EasyTierInner {
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            child: None,
            running: true,
            phase: "正在连接".into(),
            network_name: "test-net".into(),
            device_name: "desktop".into(),
            virtual_ip: String::new(),
            server_mode: String::new(),
            server_url: String::new(),
            session_token: String::new(),
            last_heartbeat: None,
            members: Vec::new(),
            ever_connected: false,
            logs: VecDeque::new(),
            log_path: None,
            pid_path: None,
        };

        apply_member_snapshot(&mut inner, "10.10.10.2".into(), Vec::new());
        assert_eq!(inner.phase, "已连接");
        assert!(inner.ever_connected);

        apply_member_snapshot(&mut inner, String::new(), Vec::new());
        assert_eq!(inner.phase, "连接中断，正在重连");
    }

    #[cfg(unix)]
    #[test]
    fn adopts_a_live_easytier_process_from_its_pid_file() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("easytier-core");
        symlink("/bin/sleep", &executable).unwrap();
        let mut child = std::process::Command::new(&executable)
            .arg("30")
            .spawn()
            .unwrap();
        let pid_path = directory.path().join("core.pid");
        let log_path = directory.path().join("core.log");
        std::fs::write(&pid_path, child.id().to_string()).unwrap();

        let mut inner = EasyTierInner {
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            child: None,
            running: false,
            phase: "未连接".into(),
            network_name: String::new(),
            device_name: String::new(),
            virtual_ip: String::new(),
            server_mode: String::new(),
            server_url: String::new(),
            session_token: String::new(),
            last_heartbeat: None,
            members: Vec::new(),
            ever_connected: false,
            logs: VecDeque::new(),
            log_path: None,
            pid_path: None,
        };
        let config = EasyTierConfig {
            server_url: "http://127.0.0.1:17280".into(),
            network_name: "test-net".into(),
            network_password: "secret".into(),
            device_name: "desktop".into(),
            allow_insecure_http: true,
        };

        sync_residual_runtime_at(&mut inner, pid_path.clone(), log_path, config);
        assert!(inner.running);
        assert_eq!(inner.phase, "已接管残留 Core");
        assert_eq!(inner.pid_path, Some(pid_path));

        child.kill().unwrap();
        child.wait().unwrap();
    }

    #[test]
    fn removes_a_pid_file_that_points_to_an_unrelated_process() {
        let directory = tempfile::tempdir().unwrap();
        let pid_path = directory.path().join("core.pid");
        std::fs::write(&pid_path, std::process::id().to_string()).unwrap();
        let mut inner = EasyTierInner {
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            child: None,
            running: false,
            phase: "未连接".into(),
            network_name: String::new(),
            device_name: String::new(),
            virtual_ip: String::new(),
            server_mode: String::new(),
            server_url: String::new(),
            session_token: String::new(),
            last_heartbeat: None,
            members: Vec::new(),
            ever_connected: false,
            logs: VecDeque::new(),
            log_path: None,
            pid_path: None,
        };

        sync_residual_runtime_at(
            &mut inner,
            pid_path.clone(),
            directory.path().join("core.log"),
            EasyTierConfig::default(),
        );

        assert!(!inner.running);
        assert!(!pid_path.exists());
    }
}
