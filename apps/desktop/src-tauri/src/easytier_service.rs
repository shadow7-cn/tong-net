use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

const SERVICE_ADDRESS: &str = "127.0.0.1:17283";
const SERVICE_LABEL: &str = "com.tingxi.tongnet.easytier";
const SERVICE_PROTOCOL_VERSION: u32 = 3;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceNetworkConfig {
    pub network_name: String,
    pub auth_type: String,
    pub auth_secret: String,
    pub device_name: String,
    pub server_address: String,
    pub peer_public_key: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceRequest {
    token: String,
    action: String,
    config: Option<ServiceNetworkConfig>,
    owner_pid: Option<u32>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceResponse {
    pub ok: bool,
    pub error: String,
    pub running: bool,
    pub core_pid: Option<u32>,
    pub protocol_version: u32,
}

struct ServiceHost {
    token: String,
    core_path: PathBuf,
    runtime_dir: PathBuf,
    child: Option<Child>,
    owner_pid: Option<u32>,
}

impl ServiceHost {
    fn response(&mut self) -> ServiceResponse {
        self.refresh_child();
        ServiceResponse {
            ok: true,
            error: String::new(),
            running: self.child.is_some(),
            core_pid: self.child.as_ref().map(Child::id),
            protocol_version: SERVICE_PROTOCOL_VERSION,
        }
    }

    fn refresh_child(&mut self) {
        let exited = self
            .child
            .as_mut()
            .and_then(|child| child.try_wait().ok())
            .flatten()
            .is_some();
        let owner_exited = self.owner_pid.is_some_and(|pid| !is_process_alive(pid));
        if exited {
            self.child = None;
            self.owner_pid = None;
            let _ = std::fs::remove_file(self.runtime_dir.join("core.pid"));
        } else if owner_exited {
            self.stop_core();
        }
    }

    fn start_core(&mut self, config: ServiceNetworkConfig, owner_pid: u32) -> Result<(), String> {
        self.refresh_child();
        if self.child.is_some() {
            return Err("EasyTier Core 已经在运行".into());
        }
        self.stop_residual_core();
        let address = validate_peer_address(&config.server_address)?;
        std::fs::create_dir_all(&self.runtime_dir).map_err(|error| error.to_string())?;
        let log_path = self.runtime_dir.join("core.log");
        let stdout = std::fs::File::create(&log_path).map_err(|error| error.to_string())?;
        let stderr = stdout.try_clone().map_err(|error| error.to_string())?;
        let mut arguments = vec![
            "--network-name".to_string(),
            config.network_name.trim().to_string(),
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
        let peer_public_key = config.peer_public_key.trim();
        if peer_public_key.is_empty() {
            arguments.extend([
                "--peers".to_string(),
                format!("tcp://{address}"),
                "--peers".to_string(),
                format!("udp://{address}"),
            ]);
        } else {
            if peer_public_key.chars().any(char::is_control) {
                return Err("EasyTier 服务端身份公钥无效".into());
            }
            let peer_config_path = self.runtime_dir.join("peer.toml");
            let peer_config = format!(
                "[[peer]]\nuri = {}\npeer_public_key = {}\n\n[[peer]]\nuri = {}\npeer_public_key = {}\n",
                serde_json::to_string(&format!("tcp://{address}"))
                    .map_err(|error| error.to_string())?,
                serde_json::to_string(peer_public_key)
                    .map_err(|error| error.to_string())?,
                serde_json::to_string(&format!("udp://{address}"))
                    .map_err(|error| error.to_string())?,
                serde_json::to_string(peer_public_key)
                    .map_err(|error| error.to_string())?,
            );
            std::fs::write(&peer_config_path, peer_config).map_err(|error| error.to_string())?;
            arguments.extend([
                "--config-file".to_string(),
                peer_config_path.to_string_lossy().to_string(),
            ]);
        }
        if config.auth_type == "credential" {
            arguments.push("--credential".into());
            arguments.push(config.auth_secret.clone());
        }
        let mut command = Command::new(&self.core_path);
        command.args(arguments);
        if config.auth_type == "network_secret" {
            command.env("ET_NETWORK_SECRET", &config.auth_secret);
        }
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| format!("特权服务无法启动 EasyTier Core：{error}"))?;
        std::fs::write(self.runtime_dir.join("core.pid"), child.id().to_string())
            .map_err(|error| error.to_string())?;
        self.owner_pid = Some(owner_pid);
        self.child = Some(child);
        Ok(())
    }

    fn stop_core(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        } else {
            self.stop_residual_core();
        }
        self.owner_pid = None;
        let _ = std::fs::remove_file(self.runtime_dir.join("core.pid"));
    }

    fn stop_residual_core(&self) {
        let pid_path = self.runtime_dir.join("core.pid");
        let Some(pid) = std::fs::read_to_string(&pid_path)
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
        else {
            return;
        };
        if !is_easytier_process(pid) {
            let _ = std::fs::remove_file(pid_path);
            return;
        }
        #[cfg(unix)]
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        #[cfg(windows)]
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
        let _ = std::fs::remove_file(pid_path);
    }

    fn handle(&mut self, request: ServiceRequest) -> ServiceResponse {
        if request.token != self.token {
            return ServiceResponse {
                ok: false,
                error: "EasyTier 服务认证失败".into(),
                running: self.child.is_some(),
                core_pid: self.child.as_ref().map(Child::id),
                protocol_version: SERVICE_PROTOCOL_VERSION,
            };
        }
        let result = match request.action.as_str() {
            "status" => Ok(()),
            "start" => match (request.config, request.owner_pid) {
                (Some(config), Some(owner_pid)) => self.start_core(config, owner_pid),
                _ => Err("启动 EasyTier 缺少配置或所属进程".into()),
            },
            "stop" => {
                self.stop_core();
                Ok(())
            }
            _ => Err("未知的 EasyTier 服务操作".into()),
        };
        match result {
            Ok(()) => self.response(),
            Err(error) => ServiceResponse {
                ok: false,
                error,
                running: self.child.is_some(),
                core_pid: self.child.as_ref().map(Child::id),
                protocol_version: SERVICE_PROTOCOL_VERSION,
            },
        }
    }
}

fn validate_peer_address(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.contains("://") || value.chars().any(char::is_whitespace) {
        return Err("EasyTier 服务收到无效连接地址".into());
    }
    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| "EasyTier 服务收到无效连接地址".to_string())?;
    if host.is_empty() || port.parse::<u16>().is_err() {
        return Err("EasyTier 服务收到无效连接地址".into());
    }
    Ok(value.to_string())
}

impl Drop for ServiceHost {
    fn drop(&mut self) {
        self.stop_core();
    }
}

pub fn run_from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--install-easytier-service") {
        let result = (|| {
            let core_path = PathBuf::from(argument_value(&args, "--core-path")?);
            let runtime_dir = PathBuf::from(argument_value(&args, "--runtime-dir")?);
            prepare_runtime(&runtime_dir)?;
            install_service(&core_path, &runtime_dir)
        })();
        if let Err(error) = result {
            eprintln!("安装同网互通 EasyTier 服务失败：{error}");
            return Some(1);
        }
        return Some(0);
    }
    if !args.iter().any(|arg| arg == "--easytier-service") {
        return None;
    }
    #[cfg(target_os = "windows")]
    let result = run_windows_service();
    #[cfg(not(target_os = "windows"))]
    let result = parse_service_paths(&args).and_then(|(core_path, runtime_dir, token_file)| {
        run_service(
            core_path,
            runtime_dir,
            token_file,
            Arc::new(AtomicBool::new(true)),
        )
    });
    if let Err(error) = result {
        eprintln!("同网互通 EasyTier 服务退出：{error}");
        Some(1)
    } else {
        Some(0)
    }
}

fn parse_service_paths(args: &[String]) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    Ok((
        PathBuf::from(argument_value(args, "--core-path")?),
        PathBuf::from(argument_value(args, "--runtime-dir")?),
        PathBuf::from(argument_value(args, "--token-file")?),
    ))
}

fn argument_value(args: &[String], name: &str) -> Result<String, String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .ok_or_else(|| format!("缺少 {name}"))
}

fn run_service(
    core_path: PathBuf,
    runtime_dir: PathBuf,
    token_file: PathBuf,
    keep_running: Arc<AtomicBool>,
) -> Result<(), String> {
    let token = std::fs::read_to_string(token_file)
        .map_err(|error| format!("读取服务令牌失败：{error}"))?;
    let listener = TcpListener::bind(SERVICE_ADDRESS)
        .map_err(|error| format!("EasyTier 服务端口不可用：{error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.to_string())?;
    let mut host = ServiceHost {
        token,
        core_path,
        runtime_dir,
        child: None,
        owner_pid: None,
    };
    while keep_running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = handle_stream(&mut host, &mut stream);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.to_string()),
        }
        host.refresh_child();
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
windows_service::define_windows_service!(ffi_service_main, windows_service_main);

#[cfg(target_os = "windows")]
fn run_windows_service() -> Result<(), String> {
    windows_service::service_dispatcher::start("TongNetEasyTierService", ffi_service_main)
        .map_err(|error| format!("启动 Windows 服务调度器失败：{error}"))
}

#[cfg(target_os = "windows")]
fn windows_service_main(_arguments: Vec<std::ffi::OsString>) {
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    let keep_running = Arc::new(AtomicBool::new(true));
    let handler_flag = keep_running.clone();
    let event_handler = move |control| match control {
        ServiceControl::Stop => {
            handler_flag.store(false, Ordering::Relaxed);
            ServiceControlHandlerResult::NoError
        }
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    };
    let Ok(status_handle) =
        service_control_handler::register("TongNetEasyTierService", event_handler)
    else {
        return;
    };
    let status = |current_state, controls_accepted| ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state,
        controls_accepted,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    };
    let _ =
        status_handle.set_service_status(status(ServiceState::Running, ServiceControlAccept::STOP));
    let args = std::env::args_os()
        .map(|value| value.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let result = parse_service_paths(&args)
        .and_then(|(core, runtime, token)| run_service(core, runtime, token, keep_running));
    let exit_code = if result.is_ok() { 0 } else { 1 };
    let _ = status_handle.set_service_status(ServiceStatus {
        exit_code: ServiceExitCode::Win32(exit_code),
        ..status(ServiceState::Stopped, ServiceControlAccept::empty())
    });
}

fn handle_stream(host: &mut ServiceHost, stream: &mut TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| error.to_string())?;
    let mut payload = String::new();
    stream
        .read_to_string(&mut payload)
        .map_err(|error| error.to_string())?;
    let request =
        serde_json::from_str::<ServiceRequest>(&payload).map_err(|error| error.to_string())?;
    let response = host.handle(request);
    stream
        .write_all(&serde_json::to_vec(&response).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn send_request(
    runtime_dir: &Path,
    action: &str,
    config: Option<ServiceNetworkConfig>,
) -> Result<ServiceResponse, String> {
    let token = std::fs::read_to_string(runtime_dir.join("service-token"))
        .map_err(|_| "EasyTier 特权服务尚未安装".to_string())?;
    let request = ServiceRequest {
        token,
        action: action.into(),
        config,
        owner_pid: (action == "start").then(std::process::id),
    };
    let mut stream = TcpStream::connect_timeout(
        &SERVICE_ADDRESS
            .parse()
            .map_err(|error: std::net::AddrParseError| error.to_string())?,
        Duration::from_millis(500),
    )
    .map_err(|_| "EasyTier 特权服务未运行".to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .map_err(|error| error.to_string())?;
    stream
        .write_all(&serde_json::to_vec(&request).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|error| error.to_string())?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    let response: ServiceResponse =
        serde_json::from_slice(&response).map_err(|error| error.to_string())?;
    if response.ok {
        Ok(response)
    } else {
        Err(response.error)
    }
}

pub fn status(runtime_dir: &Path) -> Result<ServiceResponse, String> {
    send_request(runtime_dir, "status", None)
}

pub fn start(runtime_dir: &Path, config: ServiceNetworkConfig) -> Result<ServiceResponse, String> {
    send_request(runtime_dir, "start", Some(config))
}

pub fn stop(runtime_dir: &Path) -> Result<ServiceResponse, String> {
    send_request(runtime_dir, "stop", None)
}

pub async fn ensure_installed(core_path: PathBuf, runtime_dir: PathBuf) -> Result<(), String> {
    prepare_runtime(&runtime_dir)?;
    if let Ok(response) = status(&runtime_dir) {
        if response.protocol_version == SERVICE_PROTOCOL_VERSION {
            return Ok(());
        }
    }
    let install_runtime_dir = runtime_dir.clone();
    tauri::async_runtime::spawn_blocking(move || install_service(&core_path, &install_runtime_dir))
        .await
        .map_err(|error| error.to_string())??;
    for _ in 0..30 {
        if let Ok(response) = status(&runtime_dir) {
            if response.protocol_version == SERVICE_PROTOCOL_VERSION {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err("EasyTier 特权服务安装后未能启动".into())
}

fn prepare_runtime(runtime_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(runtime_dir).map_err(|error| error.to_string())?;
    let token_path = runtime_dir.join("service-token");
    if !token_path.exists() {
        std::fs::write(&token_path, uuid::Uuid::new_v4().simple().to_string())
            .map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn install_service(core_path: &Path, runtime_dir: &Path) -> Result<(), String> {
    let source_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let helper_dir = Path::new("/Library/PrivilegedHelperTools");
    let helper_path = helper_dir.join(SERVICE_LABEL);
    let installed_core = helper_dir.join(format!("{SERVICE_LABEL}.core"));
    let plist_path = PathBuf::from(format!("/Library/LaunchDaemons/{SERVICE_LABEL}.plist"));
    let staged_plist = runtime_dir.join("easytier-service.plist");
    let token_path = runtime_dir.join("service-token");
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\"><dict>\
         <key>Label</key><string>{SERVICE_LABEL}</string>\
         <key>ProgramArguments</key><array>\
         <string>{}</string><string>--easytier-service</string>\
         <string>--core-path</string><string>{}</string>\
         <string>--runtime-dir</string><string>{}</string>\
         <string>--token-file</string><string>{}</string>\
         </array><key>RunAtLoad</key><true/><key>KeepAlive</key><true/>\
         <key>ProcessType</key><string>Interactive</string>\
         </dict></plist>",
        xml_escape(&helper_path.to_string_lossy()),
        xml_escape(&installed_core.to_string_lossy()),
        xml_escape(&runtime_dir.to_string_lossy()),
        xml_escape(&token_path.to_string_lossy()),
    );
    std::fs::write(&staged_plist, plist).map_err(|error| error.to_string())?;
    let command = format!(
        "mkdir -p {helper_dir}; \
         cp {source_exe} {helper}; cp {source_core} {installed_core}; \
         chown root:wheel {helper} {installed_core}; chmod 755 {helper} {installed_core}; \
         cp {staged_plist} {plist}; chown root:wheel {plist}; chmod 644 {plist}; \
         launchctl bootout system/{label} >/dev/null 2>&1 || true; \
         launchctl bootstrap system {plist}; launchctl enable system/{label}; \
         launchctl kickstart system/{label}",
        helper_dir = shell_quote(&helper_dir.to_string_lossy()),
        source_exe = shell_quote(&source_exe.to_string_lossy()),
        helper = shell_quote(&helper_path.to_string_lossy()),
        source_core = shell_quote(&core_path.to_string_lossy()),
        installed_core = shell_quote(&installed_core.to_string_lossy()),
        staged_plist = shell_quote(&staged_plist.to_string_lossy()),
        plist = shell_quote(&plist_path.to_string_lossy()),
        label = SERVICE_LABEL,
    );
    run_macos_admin(command)
}

#[cfg(target_os = "windows")]
fn install_service(core_path: &Path, runtime_dir: &Path) -> Result<(), String> {
    let source_exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let script_path = runtime_dir.join("install-easytier-service.ps1");
    let token_path = runtime_dir.join("service-token");
    let ps_quote = |value: &Path| value.display().to_string().replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference = 'Stop'\r\n\
         $name = 'TongNetEasyTierService'\r\n\
         $existing = Get-Service -Name $name -ErrorAction SilentlyContinue\r\n\
         if ($existing) {{ Stop-Service -Name $name -Force -ErrorAction SilentlyContinue; sc.exe delete $name | Out-Null; Start-Sleep -Seconds 1 }}\r\n\
         $serviceDir = Join-Path $env:ProgramFiles 'TongNet\\EasyTierService'\r\n\
         New-Item -ItemType Directory -Force -Path $serviceDir | Out-Null\r\n\
         $serviceExe = Join-Path $serviceDir 'tong-net-easytier-service.exe'\r\n\
         $coreExe = Join-Path $serviceDir 'easytier-core.exe'\r\n\
         Copy-Item -LiteralPath '{}' -Destination $serviceExe -Force\r\n\
         Copy-Item -LiteralPath '{}' -Destination $coreExe -Force\r\n\
         $binaryPath = '\"' + $serviceExe + '\" --easytier-service --core-path \"' + $coreExe + '\" --runtime-dir \"{}\" --token-file \"{}\"'\r\n\
         New-Service -Name $name -BinaryPathName $binaryPath -DisplayName '同网互通 EasyTier 服务' -StartupType Automatic | Out-Null\r\n\
         sc.exe failure $name reset= 0 actions= restart/1000 | Out-Null\r\n\
         Start-Service -Name $name\r\n",
        ps_quote(&source_exe),
        ps_quote(core_path),
        ps_quote(runtime_dir),
        ps_quote(&token_path),
    );
    std::fs::write(&script_path, script).map_err(|error| error.to_string())?;
    let command = format!(
        "$args = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\"'; \
         Start-Process -FilePath 'powershell.exe' -Verb RunAs -Wait -ArgumentList $args",
        script_path.display().to_string().replace('\'', "''")
    );
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &command])
        .status()
        .map_err(|error| format!("无法打开 Windows 管理员授权：{error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "安装 Windows EasyTier 服务失败".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn install_service(_core_path: &Path, _runtime_dir: &Path) -> Result<(), String> {
    Err("当前系统暂不支持 EasyTier 特权服务".into())
}

#[cfg(target_os = "macos")]
fn run_macos_admin(command: String) -> Result<(), String> {
    let script = format!(
        "do shell script {} with administrator privileges",
        serde_json::to_string(&command).map_err(|error| error.to_string())?
    );
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .output()
        .map_err(|error| format!("无法打开 macOS 管理员授权：{error}"))?;
    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| String::from_utf8_lossy(&output.stderr).trim().to_string())
}

fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()
            .map(|output| output.status.success() && !output.stdout.is_empty())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        Command::new("tasklist")
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
        Command::new("/bin/ps")
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
        Command::new("tasklist")
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

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::{xml_escape, ServiceHost, ServiceNetworkConfig, ServiceRequest};
    use std::path::PathBuf;

    #[test]
    fn rejects_requests_with_the_wrong_service_token() {
        let mut host = ServiceHost {
            token: "correct".into(),
            core_path: PathBuf::new(),
            runtime_dir: PathBuf::new(),
            child: None,
            owner_pid: None,
        };
        let response = host.handle(ServiceRequest {
            token: "wrong".into(),
            action: "status".into(),
            config: None,
            owner_pid: None,
        });
        assert!(!response.ok);
        assert_eq!(response.error, "EasyTier 服务认证失败");
    }

    #[test]
    fn escapes_launchd_xml_values() {
        assert_eq!(xml_escape("/A&B/<C>"), "/A&amp;B/&lt;C&gt;");
    }

    #[cfg(unix)]
    #[test]
    fn starts_and_stops_a_core_owned_by_the_service() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let core_path = directory.path().join("fake-easytier-core");
        std::fs::write(&core_path, "#!/bin/sh\nsleep 30\n").unwrap();
        std::fs::set_permissions(&core_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut host = ServiceHost {
            token: "token".into(),
            core_path,
            runtime_dir: directory.path().into(),
            child: None,
            owner_pid: None,
        };
        host.start_core(
            ServiceNetworkConfig {
                network_name: "test".into(),
                auth_type: "network_secret".into(),
                auth_secret: "secret".into(),
                device_name: "desktop".into(),
                server_address: "203.0.113.10:11010".into(),
                peer_public_key: "test-public-key".into(),
            },
            std::process::id(),
        )
        .unwrap();

        assert!(host.response().running);
        assert!(directory.path().join("core.pid").exists());

        host.stop_core();
        assert!(!host.response().running);
        assert!(!directory.path().join("core.pid").exists());
    }
}
