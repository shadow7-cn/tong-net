use crate::config::app_data_dir;
use aes_gcm::{
    aead::{rand_core::RngCore, Aead, OsRng},
    Aes256Gcm, KeyInit, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
#[cfg(not(target_os = "macos"))]
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

const MAX_LOG_LINES: usize = 300;

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EasyTierConfig {
    network_name: String,
    network_secret: String,
    device_name: String,
    server_address: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredEasyTierConfig {
    network_name: String,
    encrypted_network_secret: String,
    device_name: String,
    server_address: String,
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
    members: Vec<EasyTierMember>,
    logs: Vec<String>,
}

struct EasyTierInner {
    #[cfg(not(target_os = "macos"))]
    child: Option<CommandChild>,
    running: bool,
    phase: String,
    network_name: String,
    device_name: String,
    virtual_ip: String,
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
                #[cfg(not(target_os = "macos"))]
                child: None,
                running: false,
                phase: "未连接".into(),
                network_name: String::new(),
                device_name: String::new(),
                virtual_ip: String::new(),
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
        network_name: config.network_name.trim().into(),
        encrypted_network_secret: encrypt_secret(directory, &config.network_secret)?,
        device_name: config.device_name.trim().into(),
        server_address: config.server_address.trim().into(),
    };
    let value = serde_json::to_vec_pretty(&stored).map_err(|error| error.to_string())?;
    std::fs::write(config_path(directory), value).map_err(|error| error.to_string())
}

fn load_config_at(directory: &Path) -> Result<EasyTierConfig, String> {
    let value = std::fs::read(config_path(directory)).map_err(|error| error.to_string())?;
    let stored: StoredEasyTierConfig =
        serde_json::from_slice(&value).map_err(|error| error.to_string())?;
    Ok(EasyTierConfig {
        network_name: stored.network_name,
        network_secret: decrypt_secret(directory, &stored.encrypted_network_secret)?,
        device_name: stored.device_name,
        server_address: stored.server_address,
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
    let address = value
        .trim()
        .parse::<std::net::SocketAddr>()
        .map_err(|_| "连接地址必须是有效的 IP:端口，例如 203.0.113.10:11010".to_string())?;
    Ok([format!("tcp://{address}"), format!("udp://{address}")])
}

fn status_from_inner(inner: &EasyTierInner) -> EasyTierStatus {
    EasyTierStatus {
        running: inner.running,
        connected: inner.running && !inner.virtual_ip.is_empty(),
        phase: inner.phase.clone(),
        network_name: inner.network_name.clone(),
        device_name: inner.device_name.clone(),
        virtual_ip: inner.virtual_ip.clone(),
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
    let output = std::process::Command::new(bundled_binary_path("easytier-cli")?)
        .args(["-p", "127.0.0.1:17282", "-o", "json", "peer", "list"])
        .output()
        .map_err(|error| format!("无法查询 EasyTier RPC：{error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    parse_members(&output.stdout)
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

#[cfg(not(target_os = "macos"))]
fn kill_process(pid: u32) -> Result<(), String> {
    #[cfg(windows)]
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
    #[cfg(all(unix, not(target_os = "macos")))]
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
    Ok(status_from_inner(&inner))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
async fn run_admin_shell(command: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let script = format!(
            "do shell script {} with administrator privileges",
            serde_json::to_string(&command).map_err(|error| error.to_string())?
        );
        let output = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", &script])
            .output()
            .map_err(|error| format!("无法打开 macOS 管理员授权：{error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(target_os = "macos")]
async fn start_macos(
    state: &tauri::State<'_, EasyTierRuntime>,
    config: &EasyTierConfig,
) -> Result<(), String> {
    let runtime_dir = app_data_dir().join("easytier-runtime");
    std::fs::create_dir_all(&runtime_dir).map_err(|error| error.to_string())?;
    let (pid_path, log_path, secret_path) = runtime_paths();
    let peers = peers_for_address(&config.server_address)?;
    std::fs::write(&secret_path, &config.network_secret).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&secret_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }

    let mut parts = vec![
        format!(
            "ET_NETWORK_SECRET=$(cat {})",
            shell_quote(&secret_path.to_string_lossy())
        ),
        shell_quote(&bundled_binary_path("easytier-core")?.to_string_lossy()),
        "--network-name".into(),
        shell_quote(config.network_name.trim()),
        "--dhcp true".into(),
        "--hostname".into(),
        shell_quote(config.device_name.trim()),
        "--no-listener".into(),
        "--rpc-portal 127.0.0.1:17282".into(),
    ];
    for peer in peers {
        parts.push("--peers".into());
        parts.push(shell_quote(&peer));
    }
    parts.push(format!(
        "</dev/null >{} 2>&1 & CORE_PID=$!; echo $CORE_PID >{}; rm -f {}; (while kill -0 {} 2>/dev/null && kill -0 $CORE_PID 2>/dev/null; do sleep 1; done; kill -TERM $CORE_PID 2>/dev/null || true; sleep 1; kill -KILL $CORE_PID 2>/dev/null || true; rm -f {}) </dev/null >/dev/null 2>&1 &",
        shell_quote(&log_path.to_string_lossy()),
        shell_quote(&pid_path.to_string_lossy()),
        shell_quote(&secret_path.to_string_lossy()),
        std::process::id(),
        shell_quote(&pid_path.to_string_lossy())
    ));
    let result = run_admin_shell(parts.join(" ")).await;
    if result.is_err() {
        let _ = std::fs::remove_file(&secret_path);
        return result;
    }
    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "EasyTier 状态不可用".to_string())?;
    inner.running = true;
    inner.phase = "正在连接".into();
    inner.network_name = config.network_name.trim().into();
    inner.device_name = config.device_name.trim().into();
    inner.virtual_ip.clear();
    inner.members.clear();
    inner.ever_connected = false;
    inner.logs.clear();
    inner.log_path = Some(log_path);
    inner.pid_path = Some(pid_path);
    Ok(())
}

#[cfg(target_os = "windows")]
fn spawn_windows_exit_watcher(app_pid: u32, core_pid: u32) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    let script = format!(
        "while (Get-Process -Id {app_pid} -ErrorAction SilentlyContinue) {{ \
         if (-not (Get-Process -Id {core_pid} -ErrorAction SilentlyContinue)) {{ exit }}; \
         Start-Sleep -Milliseconds 500 }}; \
         Stop-Process -Id {core_pid} -Force -ErrorAction SilentlyContinue"
    );
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    std::process::Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法启动 EasyTier 退出监视器：{error}"))
}

#[tauri::command]
pub async fn start_easytier(
    _app: tauri::AppHandle,
    state: tauri::State<'_, EasyTierRuntime>,
    config: EasyTierConfig,
) -> Result<EasyTierStatus, String> {
    validate_config(&config)?;

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

    #[cfg(target_os = "macos")]
    {
        start_macos(&state, &config).await?;
        get_easytier_status(state)
    }

    #[cfg(not(target_os = "macos"))]
    let peers = peers_for_address(&config.server_address)?;
    #[cfg(not(target_os = "macos"))]
    let mut args = vec![
        "--network-name".to_string(),
        config.network_name.trim().to_string(),
        "--dhcp".to_string(),
        "true".to_string(),
        "--hostname".to_string(),
        config.device_name.trim().to_string(),
        "--no-listener".to_string(),
        "--rpc-portal".to_string(),
        "127.0.0.1:17282".to_string(),
    ];
    #[cfg(not(target_os = "macos"))]
    for peer in peers {
        args.push("--peers".into());
        args.push(peer);
    }

    #[cfg(not(target_os = "macos"))]
    let command = _app
        .shell()
        .sidecar("easytier-core")
        .map_err(|error| format!("无法加载内置 EasyTier Core：{error}"))?
        .args(args)
        .env("ET_NETWORK_SECRET", config.network_secret);
    #[cfg(not(target_os = "macos"))]
    let (mut receiver, mut child) = command
        .spawn()
        .map_err(|error| format!("无法启动内置 EasyTier Core：{error}"))?;

    #[cfg(not(target_os = "macos"))]
    {
        let core_pid = child.pid();
        let (pid_path, _, _) = runtime_paths();
        std::fs::create_dir_all(pid_path.parent().unwrap()).map_err(|error| error.to_string())?;
        std::fs::write(&pid_path, core_pid.to_string()).map_err(|error| error.to_string())?;
        #[cfg(target_os = "windows")]
        if let Err(error) = spawn_windows_exit_watcher(std::process::id(), core_pid) {
            let _ = child.kill();
            let _ = std::fs::remove_file(&pid_path);
            return Err(error);
        }

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
        inner.members.clear();
        inner.ever_connected = false;
        inner.logs.clear();
        inner.pid_path = Some(pid_path);
        push_log(&mut inner, "已启动同网互通内置 EasyTier Core".into());
    }

    #[cfg(not(target_os = "macos"))]
    let runtime = state.inner.clone();
    #[cfg(not(target_os = "macos"))]
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

    #[cfg(not(target_os = "macos"))]
    get_easytier_status(state)
}

#[tauri::command]
pub async fn stop_easytier(
    state: tauri::State<'_, EasyTierRuntime>,
) -> Result<EasyTierStatus, String> {
    #[cfg(target_os = "macos")]
    {
        let pid_path = state
            .inner
            .lock()
            .map_err(|_| "EasyTier 状态不可用".to_string())?
            .pid_path
            .clone();
        if let Some(path) = pid_path {
            let command = format!(
                "PID=$(cat {}); kill -TERM $PID 2>/dev/null || true; sleep 1; kill -KILL $PID 2>/dev/null || true; rm -f {}",
                shell_quote(&path.to_string_lossy()),
                shell_quote(&path.to_string_lossy())
            );
            run_admin_shell(command).await?;
        }
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "EasyTier 状态不可用".to_string())?;
        inner.running = false;
        inner.phase = "已停止".into();
        inner.virtual_ip.clear();
        inner.members.clear();
        inner.ever_connected = false;
        inner.pid_path = None;
        push_log(&mut inner, "用户已停止 EasyTier".into());
        Ok(status_from_inner(&inner))
    }

    #[cfg(not(target_os = "macos"))]
    {
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
        if let Some(path) = inner.pid_path.take() {
            let _ = std::fs::remove_file(path);
        }
        push_log(&mut inner, "用户已停止 EasyTier".into());
        Ok(status_from_inner(&inner))
    }
}

fn validate_config(config: &EasyTierConfig) -> Result<(), String> {
    if config.network_name.trim().is_empty()
        || config.network_secret.is_empty()
        || config.device_name.trim().is_empty()
        || config.server_address.trim().is_empty()
    {
        return Err("请填写网络名称、网络密码、设备名称和连接地址".into());
    }
    peers_for_address(&config.server_address)?;
    Ok(())
}

pub fn cleanup_on_exit(_runtime: &EasyTierRuntime) {
    #[cfg(not(target_os = "macos"))]
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
        parse_members, peers_for_address, save_config_at, sync_residual_runtime_at, EasyTierConfig,
        EasyTierInner,
    };
    use std::collections::VecDeque;

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
            network_name: "test-net".into(),
            network_secret: "plain-secret-123".into(),
            device_name: "desktop".into(),
            server_address: "127.0.0.1:11010".into(),
        };

        save_config_at(directory.path(), &config).unwrap();
        let first_file = std::fs::read_to_string(config_path(directory.path())).unwrap();
        assert!(!first_file.contains("plain-secret-123"));
        assert!(first_file.contains("encryptedNetworkSecret"));

        let restored = load_config_at(directory.path()).unwrap();
        assert_eq!(restored.network_name, config.network_name);
        assert_eq!(restored.network_secret, config.network_secret);
        assert_eq!(restored.device_name, config.device_name);
        assert_eq!(restored.server_address, config.server_address);

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

    #[test]
    fn reports_reconnecting_after_a_connected_network_loses_its_ip() {
        let mut inner = EasyTierInner {
            #[cfg(not(target_os = "macos"))]
            child: None,
            running: true,
            phase: "正在连接".into(),
            network_name: "test-net".into(),
            device_name: "desktop".into(),
            virtual_ip: String::new(),
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
            #[cfg(not(target_os = "macos"))]
            child: None,
            running: false,
            phase: "未连接".into(),
            network_name: String::new(),
            device_name: String::new(),
            virtual_ip: String::new(),
            members: Vec::new(),
            ever_connected: false,
            logs: VecDeque::new(),
            log_path: None,
            pid_path: None,
        };
        let config = EasyTierConfig {
            network_name: "test-net".into(),
            network_secret: "secret".into(),
            device_name: "desktop".into(),
            server_address: "127.0.0.1:11010".into(),
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
            #[cfg(not(target_os = "macos"))]
            child: None,
            running: false,
            phase: "未连接".into(),
            network_name: String::new(),
            device_name: String::new(),
            virtual_ip: String::new(),
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
