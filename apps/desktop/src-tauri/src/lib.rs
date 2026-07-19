mod config;
mod db;
mod server;

use chrono::Utc;
use config::{app_data_dir, load_settings, save_settings, AppSettings};
use serde::Serialize;
use server::{lan_ip, make_core, serve, serve_listener, ServerCore, ServiceInfo};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::Emitter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use uuid::Uuid;

struct RunningService {
    info: ServiceInfo,
    shutdown: oneshot::Sender<()>,
    events: tokio::sync::broadcast::Sender<String>,
    core: Arc<ServerCore>,
}

struct AppRuntime {
    settings: Mutex<AppSettings>,
    service: Mutex<Option<RunningService>>,
    native_cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            settings: Mutex::new(load_settings()),
            service: Mutex::new(None),
            native_cancellations: Mutex::new(HashMap::new()),
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
            token_required: !settings.allow_tokenless_access,
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
    let server_core = core.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = serve_listener(server_core, web_root, listener, shutdown_rx).await {
            eprintln!("同网互通服务退出：{error}");
        }
    });
    let lan_url = if settings.allow_tokenless_access {
        format!("http://{}:{}/", lan_ip(), settings.port)
    } else {
        format!("http://{}:{}/?token={}", lan_ip(), settings.port, token)
    };
    let info = ServiceInfo {
        running: true,
        port: settings.port,
        lan_url,
        token,
        token_required: !settings.allow_tokenless_access,
        started_at: Some(Utc::now().to_rfc3339()),
    };
    *state
        .service
        .lock()
        .map_err(|_| "服务状态不可用".to_string())? = Some(RunningService {
        info: info.clone(),
        shutdown: shutdown_tx,
        events,
        core,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeTransferProgress {
    transfer_id: String,
    transferred_bytes: u64,
    total_bytes: u64,
}

async fn copy_with_progress(
    source: &Path,
    destination: &Path,
    transfer_id: &str,
    total_bytes: u64,
    canceled: Arc<AtomicBool>,
    app: &tauri::AppHandle,
) -> Result<u64, String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "保存路径无效".to_string())?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;
    let temp_path = parent.join(format!(
        ".{}.{}.part",
        destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("tong-net"),
        transfer_id
    ));
    let result = async {
        let mut input = tokio::fs::File::open(source)
            .await
            .map_err(|error| error.to_string())?;
        let mut output = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|error| error.to_string())?;
        let mut buffer = vec![0u8; 256 * 1024];
        let mut transferred = 0u64;
        loop {
            if canceled.load(Ordering::Relaxed) {
                return Err("传输已取消".to_string());
            }
            let read = input
                .read(&mut buffer)
                .await
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .await
                .map_err(|error| error.to_string())?;
            transferred += read as u64;
            let _ = app.emit(
                "native-transfer-progress",
                NativeTransferProgress {
                    transfer_id: transfer_id.to_string(),
                    transferred_bytes: transferred,
                    total_bytes,
                },
            );
        }
        output.flush().await.map_err(|error| error.to_string())?;
        if destination.exists() {
            tokio::fs::remove_file(destination)
                .await
                .map_err(|error| error.to_string())?;
        }
        tokio::fs::rename(&temp_path, destination)
            .await
            .map_err(|error| error.to_string())?;
        Ok(transferred)
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(temp_path).await;
    }
    result
}

#[tauri::command]
async fn save_file_as(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppRuntime>,
    file_id: String,
    destination: String,
    transfer_id: String,
) -> Result<(), String> {
    let core = state
        .service
        .lock()
        .map_err(|_| "服务状态不可用".to_string())?
        .as_ref()
        .map(|service| service.core.clone())
        .ok_or_else(|| "互通服务未运行".to_string())?;
    let (name, stored_name, size) = core
        .db
        .file_path_info(&file_id)?
        .ok_or_else(|| "文件不存在".to_string())?;
    core.db
        .start_transfer(&transfer_id, "download", &name, "本机另存", size)?;
    let canceled = Arc::new(AtomicBool::new(false));
    state
        .native_cancellations
        .lock()
        .map_err(|_| "传输状态不可用".to_string())?
        .insert(transfer_id.clone(), canceled.clone());
    let source = core.settings.save_dir.join(stored_name);
    let result = copy_with_progress(
        &source,
        Path::new(&destination),
        &transfer_id,
        size,
        canceled,
        &app,
    )
    .await;
    state
        .native_cancellations
        .lock()
        .map_err(|_| "传输状态不可用".to_string())?
        .remove(&transfer_id);
    match result {
        Ok(transferred) => core
            .db
            .finish_transfer(&transfer_id, "success", transferred)?,
        Err(ref error) if error == "传输已取消" => core.db.cancel_transfer(&transfer_id)?,
        Err(_) => core.db.finish_transfer(&transfer_id, "failed", 0)?,
    }
    let _ = core
        .events
        .send(serde_json::json!({ "type": "transfer_updated" }).to_string());
    result.map(|_| ())
}

#[tauri::command]
fn cancel_native_transfer(
    state: tauri::State<'_, AppRuntime>,
    transfer_id: String,
) -> Result<(), String> {
    if let Some(flag) = state
        .native_cancellations
        .lock()
        .map_err(|_| "传输状态不可用".to_string())?
        .get(&transfer_id)
    {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppRuntime::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            get_service_status,
            start_service,
            stop_service,
            open_save_directory,
            save_file_as,
            cancel_native_transfer
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
