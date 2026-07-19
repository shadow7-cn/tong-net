use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub host_name: String,
    pub port: u16,
    pub save_dir: PathBuf,
    pub rotate_token: bool,
    #[serde(default = "default_allow_tokenless_access")]
    pub allow_tokenless_access: bool,
    pub cleanup_temp: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let save_dir = dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("同网互通");
        Self {
            host_name: default_host_name(),
            port: 7878,
            save_dir,
            rotate_token: true,
            allow_tokenless_access: true,
            cleanup_temp: true,
        }
    }
}

fn default_allow_tokenless_access() -> bool {
    true
}

fn default_host_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "本机电脑".to_string())
}

pub fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tong-net")
}

pub fn load_settings() -> AppSettings {
    let path = app_data_dir().join("settings.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let dir = app_data_dir();
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let value = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(dir.join("settings.json"), value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::AppSettings;

    #[test]
    fn existing_settings_enable_the_new_tokenless_default() {
        let settings: AppSettings = serde_json::from_str(
            r#"{"hostName":"主机","port":7878,"saveDir":"/tmp","rotateToken":true,"cleanupTemp":true}"#,
        )
        .unwrap();

        assert!(settings.allow_tokenless_access);
    }
}
