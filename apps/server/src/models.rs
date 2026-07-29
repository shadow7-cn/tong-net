use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicInfo {
    pub initialized: bool,
    pub site_name: String,
    pub mode: String,
    pub version: String,
    pub minimum_desktop_version: String,
    pub public_host: String,
    pub web_port: u16,
    pub easytier_port: u16,
    pub shared_public_key: String,
}

#[derive(Clone, Debug)]
pub struct SiteSnapshot {
    pub initialized: bool,
    pub site_name: String,
    pub public_host: String,
    pub mode: String,
    pub shared_name: String,
    pub shared_secret: String,
    pub shared_private_key: String,
    pub shared_public_key: String,
    pub networks: Vec<NetworkSecret>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRecord {
    pub id: String,
    pub name: String,
    pub status: String,
    pub slot: i64,
    pub device_count: i64,
    pub online_count: i64,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct NetworkSecret {
    pub id: String,
    pub name: String,
    pub internal_name: String,
    pub internal_secret: String,
    pub private_key: String,
    pub status: String,
    pub slot: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRecord {
    pub membership_id: String,
    pub device_id: String,
    pub network_id: String,
    pub network_name: String,
    pub name: String,
    pub admin_note: String,
    pub platform: String,
    pub client_version: String,
    pub status: String,
    pub online: bool,
    pub virtual_ip: String,
    pub protocol: String,
    pub latency_ms: Option<i64>,
    pub rx_bytes: i64,
    pub tx_bytes: i64,
    pub last_seen_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecord {
    pub id: String,
    pub actor_type: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub result: String,
    pub ip_address: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EasyTierStatus {
    pub running: bool,
    pub healthy: bool,
    pub disabled_for_test: bool,
    pub shared_running: bool,
    pub manager_total: usize,
    pub manager_running: usize,
    pub last_error: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialResult {
    pub credential_id: String,
    pub credential_secret: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberSnapshot {
    pub id: String,
    pub hostname: String,
    pub ipv4: String,
    pub latency: String,
    pub protocol: String,
    pub rx_bytes: String,
    pub tx_bytes: String,
    pub version: String,
}
