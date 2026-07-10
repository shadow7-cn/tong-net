use crate::{
    config::AppSettings,
    db::{Database, Device, FileRecord, MessageRecord, TransferRecord},
};
use axum::{
    body::Body,
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Multipart, Path, Query, State,
    },
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::{broadcast, oneshot},
};
use tokio_util::io::ReaderStream;
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
};
use uuid::Uuid;

pub type ApiResult<T> = Result<T, (StatusCode, Json<Value>)>;

fn api_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "message": message.into() })))
}

pub struct ServerCore {
    pub token: String,
    pub settings: AppSettings,
    pub db: Database,
    pub online_counts: Mutex<HashMap<String, usize>>,
    pub events: broadcast::Sender<String>,
    pub temp_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub running: bool,
    pub port: u16,
    pub lan_url: String,
    pub token: String,
    pub started_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: Option<String>,
    #[serde(rename = "deviceId")]
    device_id: Option<String>,
}

struct TempCleanup {
    path: PathBuf,
    armed: bool,
    core: Arc<ServerCore>,
    transfer_id: String,
}
impl Drop for TempCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
            let _ = self.core.db.finish_transfer(&self.transfer_id, "failed", 0);
            broadcast_refresh(&self.core, "transfer_failed");
        }
    }
}

fn verify_token(
    core: &ServerCore,
    headers: &HeaderMap,
    query_token: Option<&str>,
) -> ApiResult<()> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if bearer.or(query_token) == Some(core.token.as_str()) {
        Ok(())
    } else {
        Err(api_error(StatusCode::UNAUTHORIZED, "访问令牌无效"))
    }
}

fn device_id(headers: &HeaderMap) -> ApiResult<String> {
    headers
        .get("x-device-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "缺少访问端身份"))
}

fn participants(core: &ServerCore, headers: &HeaderMap, peer: &str) -> ApiResult<String> {
    let me = device_id(headers)?;
    let me_exists = core
        .db
        .device_exists(&me)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let peer_exists = core
        .db
        .device_exists(peer)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if !me_exists || !peer_exists || me == peer {
        return Err(api_error(StatusCode::BAD_REQUEST, "会话访问端无效"));
    }
    Ok(me)
}

fn broadcast_refresh(core: &ServerCore, kind: &str) {
    let _ = core.events.send(json!({ "type": kind }).to_string());
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapResponse {
    service_name: String,
    host_device_id: String,
    current_device: Device,
}

async fn bootstrap(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Query(query): Query<TokenQuery>,
) -> ApiResult<Json<BootstrapResponse>> {
    verify_token(&core, &headers, query.token.as_deref())?;
    let client_id = headers
        .get("x-client-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("browser");
    let encoded_name = headers
        .get("x-device-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("browser");
    let decoded_name = urlencoding::decode(encoded_name)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| "浏览器访问端".into());
    let name = if encoded_name == "browser" {
        "浏览器访问端"
    } else {
        decoded_name.as_str()
    };
    let encoded_source = headers
        .get("x-client-source")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("other");
    let decoded_source = urlencoding::decode(encoded_source)
        .map(|value| value.into_owned())
        .unwrap_or_else(|_| "其他浏览器".into());
    let browser_source = if encoded_source == "other" {
        "其他浏览器"
    } else {
        decoded_source.as_str()
    };
    let (device, is_new) = core
        .db
        .register_device(client_id, name, browser_source)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    if is_new {
        let _ =
            core.db
                .add_system_message(&device.id, "host", &format!("{} 已加入互通", device.name));
    }
    broadcast_refresh(&core, "device_online");
    Ok(Json(BootstrapResponse {
        service_name: "同网互通".into(),
        host_device_id: "host".into(),
        current_device: device,
    }))
}

async fn list_devices(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<Device>>> {
    verify_token(&core, &headers, None)?;
    let online = core
        .online_counts
        .lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "在线状态不可用"))?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    Ok(Json(core.db.list_devices(&online).map_err(|e| {
        api_error(StatusCode::INTERNAL_SERVER_ERROR, e)
    })?))
}

#[derive(Deserialize)]
struct RenameBody {
    name: String,
}
async fn rename_me(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Json(body): Json<RenameBody>,
) -> ApiResult<Json<Device>> {
    verify_token(&core, &headers, None)?;
    let id = device_id(&headers)?;
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "昵称应为 1 到 40 个字符",
        ));
    }
    let device = core
        .db
        .rename_device(&id, name)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    broadcast_refresh(&core, "devices_changed");
    Ok(Json(device))
}

async fn remove_device(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Path(target): Path<String>,
) -> ApiResult<StatusCode> {
    verify_token(&core, &headers, None)?;
    if device_id(&headers)? != "host" {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "只有本机主机可以移除访问端",
        ));
    }
    if target == "host" {
        return Err(api_error(StatusCode::BAD_REQUEST, "本机主机不能移除"));
    }
    if core
        .online_counts
        .lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "在线状态不可用"))?
        .contains_key(&target)
    {
        return Err(api_error(StatusCode::CONFLICT, "在线访问端不可移除"));
    }
    if !core
        .db
        .remove_device(&target)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
    {
        return Err(api_error(StatusCode::NOT_FOUND, "访问端不存在或已移除"));
    }
    broadcast_refresh(&core, "devices_changed");
    Ok(StatusCode::NO_CONTENT)
}

async fn messages(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Path(peer): Path<String>,
) -> ApiResult<Json<Vec<MessageRecord>>> {
    verify_token(&core, &headers, None)?;
    let me = participants(&core, &headers, &peer)?;
    Ok(Json(core.db.list_messages(&me, &peer).map_err(|e| {
        api_error(StatusCode::INTERNAL_SERVER_ERROR, e)
    })?))
}

#[derive(Deserialize)]
struct TextBody {
    content: String,
}
async fn send_text(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Path(peer): Path<String>,
    Json(body): Json<TextBody>,
) -> ApiResult<Json<MessageRecord>> {
    verify_token(&core, &headers, None)?;
    let me = participants(&core, &headers, &peer)?;
    let content = body.content.trim();
    if content.is_empty() || content.chars().count() > 4000 {
        return Err(api_error(StatusCode::BAD_REQUEST, "消息内容无效"));
    }
    let record = core
        .db
        .add_text_message(&me, &peer, content)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    broadcast_refresh(&core, "message_created");
    Ok(Json(record))
}

async fn upload_file(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Path(peer): Path<String>,
    mut multipart: Multipart,
) -> ApiResult<Json<MessageRecord>> {
    verify_token(&core, &headers, None)?;
    let me = participants(&core, &headers, &peer)?;
    let transfer_id = headers
        .get("x-transfer-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 80)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let total_bytes = headers
        .get("x-file-size")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default();
    let header_name = headers
        .get("x-file-name")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| urlencoding::decode(value).ok())
        .map(|value| value.into_owned())
        .unwrap_or_else(|| "未完成文件".into());
    let peer_name = core
        .db
        .list_devices(&[])
        .ok()
        .and_then(|items| items.into_iter().find(|item| item.id == peer))
        .map(|item| item.name)
        .unwrap_or_else(|| peer.clone());
    core.db
        .start_transfer(
            &transfer_id,
            "upload",
            &header_name,
            &peer_name,
            total_bytes,
        )
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    broadcast_refresh(&core, "transfer_started");
    tokio::fs::create_dir_all(&core.temp_dir)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tokio::fs::create_dir_all(&core.settings.save_dir)
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let upload_id = Uuid::new_v4().to_string();
    let temp_path = core.temp_dir.join(format!("{upload_id}.part"));
    let mut cleanup = TempCleanup {
        path: temp_path.clone(),
        armed: true,
        core: core.clone(),
        transfer_id: transfer_id.clone(),
    };
    let mut original_name = None;
    let result: Result<u64, String> = async {
        while let Some(mut field) = multipart.next_field().await.map_err(|e| e.to_string())? {
            if field.name() != Some("file") {
                continue;
            }
            let raw_name = field.file_name().unwrap_or("未命名文件");
            let safe_name = std::path::Path::new(raw_name)
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("未命名文件")
                .to_string();
            let _ =
                core.db
                    .start_transfer(&transfer_id, "upload", &safe_name, &peer_name, total_bytes);
            original_name = Some(safe_name);
            let mut output = tokio::fs::File::create(&temp_path)
                .await
                .map_err(|e| e.to_string())?;
            let mut size = 0u64;
            while let Some(chunk) = field.chunk().await.map_err(|e| e.to_string())? {
                size += chunk.len() as u64;
                output.write_all(&chunk).await.map_err(|e| e.to_string())?;
                let _ = core.db.update_transfer_progress(&transfer_id, size);
            }
            output.flush().await.map_err(|e| e.to_string())?;
            return Ok(size);
        }
        Err("没有收到文件".into())
    }
    .await;
    let size = match result {
        Ok(size) => size,
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(api_error(StatusCode::BAD_REQUEST, error));
        }
    };
    let name = original_name.unwrap_or_else(|| "未命名文件".into());
    let stored_name = format!("{upload_id}-{name}");
    let final_path = core.settings.save_dir.join(&stored_name);
    if let Err(error) = tokio::fs::rename(&temp_path, &final_path).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
        ));
    }
    cleanup.armed = false;
    let file = core
        .db
        .add_file(&name, &stored_name, size)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let record = core
        .db
        .add_file_message(&me, &peer, file)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    core.db
        .finish_transfer(&transfer_id, "success", size)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    broadcast_refresh(&core, "file_message_created");
    Ok(Json(record))
}

async fn cancel_transfer(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    verify_token(&core, &headers, None)?;
    core.db
        .cancel_transfer(&id)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    broadcast_refresh(&core, "transfer_canceled");
    Ok(StatusCode::NO_CONTENT)
}

async fn transfers(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<TransferRecord>>> {
    verify_token(&core, &headers, None)?;
    Ok(Json(core.db.list_transfers().map_err(|e| {
        api_error(StatusCode::INTERNAL_SERVER_ERROR, e)
    })?))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordsResponse {
    devices: Vec<Device>,
    messages: Vec<MessageRecord>,
    files: Vec<FileRecord>,
    transfers: Vec<TransferRecord>,
}

async fn records(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
) -> ApiResult<Json<RecordsResponse>> {
    verify_token(&core, &headers, None)?;
    let online = core
        .online_counts
        .lock()
        .map_err(|_| api_error(StatusCode::INTERNAL_SERVER_ERROR, "在线状态不可用"))?
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    Ok(Json(RecordsResponse {
        devices: core
            .db
            .list_all_devices(&online)
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?,
        messages: core
            .db
            .list_all_messages()
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?,
        files: core
            .db
            .list_files()
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?,
        transfers: core
            .db
            .list_transfers()
            .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?,
    }))
}

fn parse_range(value: Option<&str>, size: u64) -> Result<(u64, u64, bool), ()> {
    let Some(value) = value else {
        return Ok((0, size.saturating_sub(1), false));
    };
    let range = value.strip_prefix("bytes=").ok_or(())?;
    if range.contains(',') {
        return Err(());
    }
    let (start, end) = range.split_once('-').ok_or(())?;
    if start.is_empty() {
        let suffix: u64 = end.parse().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        return Ok((
            size.saturating_sub(suffix.min(size)),
            size.saturating_sub(1),
            true,
        ));
    }
    let start: u64 = start.parse().map_err(|_| ())?;
    let end: u64 = if end.is_empty() {
        size.saturating_sub(1)
    } else {
        end.parse().map_err(|_| ())?
    };
    if start > end || end >= size {
        return Err(());
    }
    Ok((start, end, true))
}

async fn download_file(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Query(query): Query<TokenQuery>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    verify_token(&core, &headers, query.token.as_deref())?;
    let Some((name, stored_name, size)) = core
        .db
        .file_path_info(&id)
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e))?
    else {
        return Err(api_error(StatusCode::NOT_FOUND, "文件不存在"));
    };
    let (start, end, partial) = parse_range(
        headers.get(header::RANGE).and_then(|v| v.to_str().ok()),
        size,
    )
    .map_err(|_| api_error(StatusCode::RANGE_NOT_SATISFIABLE, "Range 无效"))?;
    let mut file = tokio::fs::File::open(core.settings.save_dir.join(stored_name))
        .await
        .map_err(|_| api_error(StatusCode::NOT_FOUND, "文件不存在"))?;
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|e| api_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let length = end - start + 1;
    if end == size.saturating_sub(1) {
        if let Some(requester) = query.device_id.as_deref() {
            let peer_name = core
                .db
                .list_devices(&[])
                .ok()
                .and_then(|items| items.into_iter().find(|item| item.id == requester))
                .map(|item| item.name)
                .unwrap_or_else(|| "浏览器访问端".into());
            let _ = core
                .db
                .add_transfer("download", &name, &peer_name, "success");
        }
    }
    let stream = ReaderStream::new(file.take(length));
    let encoded = urlencoding::encode(&name);
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let headers = response.headers_mut();
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string()).unwrap(),
    );
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(
            mime_guess::from_path(&name)
                .first_or_octet_stream()
                .as_ref(),
        )
        .unwrap(),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"download\"; filename*=UTF-8''{encoded}"
        ))
        .unwrap(),
    );
    if partial {
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{size}")).unwrap(),
        );
    }
    Ok(response)
}

async fn websocket(
    State(core): State<Arc<ServerCore>>,
    headers: HeaderMap,
    Query(query): Query<TokenQuery>,
    upgrade: WebSocketUpgrade,
) -> ApiResult<impl IntoResponse> {
    verify_token(&core, &headers, query.token.as_deref())?;
    let device = query
        .device_id
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "缺少访问端身份"))?;
    Ok(upgrade.on_upgrade(move |socket| socket_loop(core, device, socket)))
}

async fn socket_loop(core: Arc<ServerCore>, device: String, socket: WebSocket) {
    if let Ok(mut online) = core.online_counts.lock() {
        *online.entry(device.clone()).or_default() += 1;
    }
    let _ = core.db.touch_device(&device);
    broadcast_refresh(&core, "device_online");
    let (mut sender, mut receiver) = socket.split();
    let mut events = core.events.subscribe();
    let send_task = tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            if sender.send(WsMessage::Text(event.into())).await.is_err() {
                break;
            }
        }
    });
    while let Some(Ok(message)) = receiver.next().await {
        if matches!(message, WsMessage::Close(_)) {
            break;
        }
    }
    send_task.abort();
    let became_offline = if let Ok(mut online) = core.online_counts.lock() {
        match online.get_mut(&device) {
            Some(count) if *count > 1 => {
                *count -= 1;
                false
            }
            Some(_) => {
                online.remove(&device);
                true
            }
            None => false,
        }
    } else {
        false
    };
    let _ = core.db.touch_device(&device);
    if became_offline && device != "host" {
        let name = core
            .db
            .list_devices(&[])
            .ok()
            .and_then(|items| items.into_iter().find(|item| item.id == device))
            .map(|item| item.name)
            .unwrap_or_else(|| "访问端".into());
        let _ = core
            .db
            .add_system_message(&device, "host", &format!("{name} 已离线"));
    }
    broadcast_refresh(&core, "device_offline");
}

pub fn build_router(core: Arc<ServerCore>, web_root: PathBuf) -> Router {
    let fallback =
        ServeDir::new(&web_root).not_found_service(ServeFile::new(web_root.join("index.html")));
    Router::new()
        .route("/api/bootstrap", get(bootstrap))
        .route("/api/devices", get(list_devices))
        .route("/api/devices/{id}", axum::routing::delete(remove_device))
        .route("/api/devices/me", patch(rename_me))
        .route(
            "/api/conversations/{peer}/messages",
            get(messages).post(send_text),
        )
        .route("/api/conversations/{peer}/files", post(upload_file))
        .route("/api/files/{id}/download", get(download_file))
        .route("/api/transfers", get(transfers))
        .route("/api/transfers/{id}/cancel", post(cancel_transfer))
        .route("/api/records", get(records))
        .route("/ws", get(websocket))
        .fallback_service(fallback)
        .layer(DefaultBodyLimit::disable())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_headers(Any)
                .allow_methods(Any),
        )
        .with_state(core)
}

pub async fn serve(
    core: Arc<ServerCore>,
    web_root: PathBuf,
    port: u16,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), String> {
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
        .await
        .map_err(|error| error.to_string())?;
    serve_listener(core, web_root, listener, shutdown).await
}

pub async fn serve_listener(
    core: Arc<ServerCore>,
    web_root: PathBuf,
    listener: tokio::net::TcpListener,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), String> {
    axum::serve(listener, build_router(core, web_root))
        .with_graceful_shutdown(async {
            let _ = shutdown.await;
        })
        .await
        .map_err(|error| error.to_string())
}

pub fn lan_ip() -> String {
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

pub fn make_core(
    settings: AppSettings,
    token: String,
    db_path: PathBuf,
    temp_dir: PathBuf,
) -> Result<Arc<ServerCore>, String> {
    if settings.cleanup_temp && temp_dir.exists() {
        for entry in std::fs::read_dir(&temp_dir)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            if entry.path().extension().and_then(|v| v.to_str()) == Some("part") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let db = Database::open(&db_path, &settings.host_name)?;
    let (events, _) = broadcast::channel(128);
    Ok(Arc::new(ServerCore {
        token,
        settings,
        db,
        online_counts: Mutex::new(HashMap::new()),
        events,
        temp_dir,
    }))
}

#[cfg(test)]
mod tests {
    use super::{build_router, make_core, parse_range};
    use crate::config::AppSettings;
    use futures_util::StreamExt;
    use reqwest::{multipart, Client, StatusCode};
    use serde_json::Value;
    use std::fs;
    use tokio::io::AsyncWriteExt;
    #[test]
    fn parses_http_ranges() {
        assert_eq!(parse_range(None, 10), Ok((0, 9, false)));
        assert_eq!(parse_range(Some("bytes=3-"), 10), Ok((3, 9, true)));
        assert_eq!(parse_range(Some("bytes=3-5"), 10), Ok((3, 5, true)));
        assert_eq!(parse_range(Some("bytes=-4"), 10), Ok((6, 9, true)));
        assert!(parse_range(Some("bytes=9-12"), 10).is_err());
    }

    #[tokio::test]
    async fn browser_chat_upload_and_range_download_work_end_to_end() {
        let root = tempfile::tempdir().unwrap();
        let web = root.path().join("web");
        let save = root.path().join("files");
        let temp = root.path().join("temp");
        fs::create_dir_all(&web).unwrap();
        fs::write(web.join("index.html"), "tong-net").unwrap();
        let settings = AppSettings {
            host_name: "测试主机".into(),
            port: 7878,
            save_dir: save,
            rotate_token: true,
            cleanup_temp: true,
        };
        let db_path = root.path().join("test.sqlite3");
        let core = make_core(
            settings.clone(),
            "test-token".into(),
            db_path.clone(),
            temp.clone(),
        )
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, build_router(core, web))
                .await
                .unwrap();
        });
        let client = Client::builder().no_proxy().build().unwrap();
        let base = format!("http://{address}");

        let unauthorized = client
            .get(format!("{base}/api/devices"))
            .send()
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let bootstrap: Value = client
            .get(format!("{base}/api/bootstrap?token=test-token"))
            .header("x-client-id", "browser-one")
            .header("x-device-name", "%E6%89%8B%E6%9C%BA")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let device_id = bootstrap["currentDevice"]["id"].as_str().unwrap();
        let original_device_id = device_id.to_string();
        let auth = |request: reqwest::RequestBuilder| {
            request
                .bearer_auth("test-token")
                .header("x-device-id", device_id)
        };

        let bad_ws = tokio_tungstenite::connect_async(format!(
            "ws://{address}/ws?token=wrong&deviceId={device_id}"
        ))
        .await;
        assert!(
            matches!(bad_ws, Err(tokio_tungstenite::tungstenite::Error::Http(response)) if response.status() == StatusCode::UNAUTHORIZED)
        );
        let (mut socket, _) = tokio_tungstenite::connect_async(format!(
            "ws://{address}/ws?token=test-token&deviceId={device_id}"
        ))
        .await
        .unwrap();

        auth(
            client
                .post(format!("{base}/api/conversations/host/messages"))
                .json(&serde_json::json!({"content":"你好"})),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(event.into_text().unwrap().contains("message_created"));

        let form = multipart::Form::new().part(
            "file",
            multipart::Part::bytes(b"0123456789".to_vec()).file_name("hello.txt"),
        );
        let uploaded: Value = auth(
            client
                .post(format!("{base}/api/conversations/host/files"))
                .multipart(form),
        )
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
        let file_id = uploaded["file"]["id"].as_str().unwrap();

        let messages: Vec<Value> =
            auth(client.get(format!("{base}/api/conversations/host/messages")))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json()
                .await
                .unwrap();
        assert_eq!(messages.len(), 3);
        assert!(messages.iter().any(|message| message["type"] == "system"));

        let full = client
            .get(format!(
                "{base}/api/files/{file_id}/download?token=test-token"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(full.status(), StatusCode::OK);
        assert_eq!(full.bytes().await.unwrap().as_ref(), b"0123456789");
        let partial = client
            .get(format!(
                "{base}/api/files/{file_id}/download?token=test-token"
            ))
            .header("range", "bytes=4-7")
            .send()
            .await
            .unwrap();
        assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(partial.headers()["content-range"], "bytes 4-7/10");
        assert_eq!(partial.bytes().await.unwrap().as_ref(), b"4567");

        let mut interrupted = tokio::net::TcpStream::connect(address).await.unwrap();
        let prefix = format!(
            "POST /api/conversations/host/files HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer test-token\r\nX-Device-Id: {device_id}\r\nContent-Type: multipart/form-data; boundary=tongnet\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n--tongnet\r\nContent-Disposition: form-data; name=\"file\"; filename=\"broken.bin\"\r\nContent-Type: application/octet-stream\r\n\r\npartial"
        );
        interrupted.write_all(prefix.as_bytes()).await.unwrap();
        interrupted.flush().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        drop(interrupted);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(fs::read_dir(temp).unwrap().all(|entry| entry
            .unwrap()
            .path()
            .extension()
            .and_then(|v| v.to_str())
            != Some("part")));
        let transfer_rows: Vec<Value> = auth(client.get(format!("{base}/api/transfers")))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(transfer_rows
            .iter()
            .any(|row| row["fileName"] == "broken.bin" && row["status"] == "failed"));
        socket.close(None).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let forbidden = auth(client.delete(format!("{base}/api/devices/{device_id}")))
            .send()
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
        let removed = client
            .delete(format!("{base}/api/devices/{device_id}"))
            .bearer_auth("test-token")
            .header("x-device-id", "host")
            .send()
            .await
            .unwrap();
        assert_eq!(removed.status(), StatusCode::NO_CONTENT);
        let active_devices: Vec<Value> = client
            .get(format!("{base}/api/devices"))
            .bearer_auth("test-token")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(!active_devices.iter().any(|item| item["id"] == device_id));

        let restored_response: Value = client
            .get(format!("{base}/api/bootstrap?token=test-token"))
            .header("x-client-id", "browser-one")
            .header("x-device-name", "%E6%89%8B%E6%9C%BA")
            .header(
                "x-client-source",
                "%E5%BE%AE%E4%BF%A1%E5%86%85%E7%BD%AE%E6%B5%8F%E8%A7%88%E5%99%A8",
            )
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(restored_response["currentDevice"]["id"], device_id);
        assert_eq!(
            restored_response["currentDevice"]["browserSource"],
            "微信内置浏览器"
        );
        task.abort();
        let _ = task.await;

        let reopened = crate::db::Database::open(&db_path, &settings.host_name).unwrap();
        let (restored, is_new) = reopened
            .register_device("browser-one", "手机", "微信内置浏览器")
            .unwrap();
        assert!(!is_new);
        assert_eq!(restored.id, original_device_id);
        assert_eq!(
            reopened.list_messages(&restored.id, "host").unwrap().len(),
            4
        );
    }

    #[tokio::test]
    async fn graceful_shutdown_stops_new_requests() {
        let root = tempfile::tempdir().unwrap();
        let web = root.path().join("web");
        fs::create_dir_all(&web).unwrap();
        fs::write(web.join("index.html"), "tong-net").unwrap();
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let settings = AppSettings {
            host_name: "测试主机".into(),
            port,
            save_dir: root.path().join("files"),
            rotate_token: true,
            cleanup_temp: true,
        };
        let core = make_core(
            settings,
            "shutdown-token".into(),
            root.path().join("db.sqlite3"),
            root.path().join("temp"),
        )
        .unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let service = tokio::spawn(super::serve(core, web, port, shutdown_rx));
        let client = Client::builder().no_proxy().build().unwrap();
        let url = format!("http://127.0.0.1:{port}/api/bootstrap?token=shutdown-token");
        let mut reachable = false;
        for _ in 0..20 {
            if client
                .get(&url)
                .header("x-client-id", "lifecycle")
                .send()
                .await
                .is_ok()
            {
                reachable = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(reachable);
        shutdown_tx.send(()).unwrap();
        assert!(service.await.unwrap().is_ok());
        assert!(client.get(&url).send().await.is_err());
    }
}
