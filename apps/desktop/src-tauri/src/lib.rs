mod config;
mod db;
mod server;

use chrono::Utc;
use config::{app_data_dir, load_settings, save_settings, AppSettings};
use server::{lan_ip, make_core, serve, serve_listener, ServiceInfo};
use std::{path::PathBuf, sync::Mutex};
use tokio::sync::oneshot;
use uuid::Uuid;

struct RunningService {
    info: ServiceInfo,
    shutdown: oneshot::Sender<()>,
    events: tokio::sync::broadcast::Sender<String>,
}

struct AppRuntime {
    settings: Mutex<AppSettings>,
    service: Mutex<Option<RunningService>>,
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            settings: Mutex::new(load_settings()),
            service: Mutex::new(None),
        }
    }
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppRuntime>) -> Result<AppSettings, String> {
    state
        .settings
        .lock()
        .map(|value| value.clone())
        .map_err(|_| "设置不可用".into())
}

#[tauri::command]
fn update_settings(
    state: tauri::State<'_, AppRuntime>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    if !(1024..=65535).contains(&settings.port) {
        return Err("端口必须在 1024 到 65535 之间".into());
    }
    if settings.host_name.trim().is_empty() {
        return Err("本机主机名称不能为空".into());
    }
    if state
        .service
        .lock()
        .map_err(|_| "服务状态不可用".to_string())?
        .is_some()
    {
        return Err("请先停止互通服务再修改设置".into());
    }
    save_settings(&settings)?;
    *state
        .settings
        .lock()
        .map_err(|_| "设置不可用".to_string())? = settings.clone();
    Ok(settings)
}

#[tauri::command]
fn get_service_status(state: tauri::State<'_, AppRuntime>) -> Result<ServiceInfo, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "设置不可用".to_string())?
        .clone();
    Ok(state
        .service
        .lock()
        .map_err(|_| "服务状态不可用".to_string())?
        .as_ref()
        .map(|running| running.info.clone())
        .unwrap_or(ServiceInfo {
            running: false,
            port: settings.port,
            lan_url: String::new(),
            token: String::new(),
            started_at: None,
        }))
}

#[tauri::command]
async fn start_service(state: tauri::State<'_, AppRuntime>) -> Result<ServiceInfo, String> {
    if let Some(info) = state
        .service
        .lock()
        .map_err(|_| "服务状态不可用".to_string())?
        .as_ref()
        .map(|running| running.info.clone())
    {
        return Ok(info);
    }
    let settings = state
        .settings
        .lock()
        .map_err(|_| "设置不可用".to_string())?
        .clone();
    let data_dir = app_data_dir();
    std::fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&settings.save_dir).map_err(|error| error.to_string())?;
    let previous_token = std::fs::read_to_string(data_dir.join("access-token")).ok();
    let token = if settings.rotate_token {
        Uuid::new_v4().simple().to_string()
    } else {
        previous_token.unwrap_or_else(|| Uuid::new_v4().simple().to_string())
    };
    std::fs::write(data_dir.join("access-token"), &token).map_err(|error| error.to_string())?;
    let core = make_core(
        settings.clone(),
        token.clone(),
        data_dir.join("tong-net.sqlite3"),
        data_dir.join("temp"),
    )?;
    let events = core.events.clone();
    let web_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist");
    if !web_root.join("index.html").exists() {
        return Err("Web 资源尚未构建，请先执行 npm run build".into());
    }
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", settings.port))
        .await
        .map_err(|error| format!("端口 {} 无法使用：{error}", settings.port))?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = serve_listener(core, web_root, listener, shutdown_rx).await {
            eprintln!("同网互通服务退出：{error}");
        }
    });
    let lan_url = format!(
        "http://{}:{}/?token={}#/web",
        lan_ip(),
        settings.port,
        token
    );
    let info = ServiceInfo {
        running: true,
        port: settings.port,
        lan_url,
        token,
        started_at: Some(Utc::now().to_rfc3339()),
    };
    *state
        .service
        .lock()
        .map_err(|_| "服务状态不可用".to_string())? = Some(RunningService {
        info: info.clone(),
        shutdown: shutdown_tx,
        events,
    });
    Ok(info)
}

#[tauri::command]
fn stop_service(state: tauri::State<'_, AppRuntime>) -> Result<ServiceInfo, String> {
    if let Some(service) = state
        .service
        .lock()
        .map_err(|_| "服务状态不可用".to_string())?
        .take()
    {
        let _ = service
            .events
            .send(serde_json::json!({ "type": "service_stopping" }).to_string());
        let _ = service.shutdown.send(());
    }
    get_service_status(state)
}

#[tauri::command]
fn open_save_directory(state: tauri::State<'_, AppRuntime>) -> Result<(), String> {
    let path = state
        .settings
        .lock()
        .map_err(|_| "设置不可用".to_string())?
        .save_dir
        .clone();
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    open::that(path).map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppRuntime::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            get_service_status,
            start_service,
            stop_service,
            open_save_directory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

pub async fn run_standalone(
    port: u16,
    data_dir: PathBuf,
    web_root: PathBuf,
    token: String,
) -> Result<(), String> {
    let settings = AppSettings {
        port,
        host_name: "自测主机".into(),
        save_dir: data_dir.join("files"),
        ..AppSettings::default()
    };
    let core = make_core(
        settings,
        token,
        data_dir.join("tong-net.sqlite3"),
        data_dir.join("temp"),
    )?;
    let (_shutdown_tx, shutdown_rx) = oneshot::channel();
    serve(core, web_root, port, shutdown_rx).await
}
