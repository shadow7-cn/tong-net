use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub web_port: u16,
    pub easytier_port: u16,
    pub data_dir: PathBuf,
    pub web_dir: PathBuf,
    pub easytier_core: PathBuf,
    pub easytier_cli: PathBuf,
    pub internal_easytier_host: String,
    pub easytier_disabled: bool,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            web_port: parse_port("TONGNET_WEB_PORT", 17280)?,
            easytier_port: parse_port("TONGNET_EASYTIER_PORT", 11010)?,
            data_dir: std::env::var("TONGNET_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/data")),
            web_dir: std::env::var("TONGNET_WEB_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/opt/tong-net/web")),
            easytier_core: std::env::var("TONGNET_EASYTIER_CORE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/easytier-core")),
            easytier_cli: std::env::var("TONGNET_EASYTIER_CLI")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/usr/local/bin/easytier-cli")),
            internal_easytier_host: std::env::var("TONGNET_INTERNAL_EASYTIER_HOST")
                .unwrap_or_else(|_| "127.0.0.1".into()),
            easytier_disabled: matches!(
                std::env::var("TONGNET_EASYTIER_DISABLED").as_deref(),
                Ok("1" | "true")
            ),
        })
    }

    pub fn ensure_directories(&self) -> Result<(), String> {
        for path in [
            self.data_dir.join("db"),
            self.data_dir.join("keys"),
            self.data_dir.join("easytier/shared"),
            self.data_dir.join("easytier/networks"),
            self.data_dir.join("logs"),
        ] {
            std::fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            set_private_directory(&path)?;
        }
        Ok(())
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("db/tong-net-server.sqlite3")
    }
}

fn parse_port(name: &str, fallback: u16) -> Result<u16, String> {
    match std::env::var(name) {
        Ok(value) => value
            .parse::<u16>()
            .map_err(|_| format!("{name} 必须是 1-65535 的端口")),
        Err(_) => Ok(fallback),
    }
}

#[cfg(unix)]
fn set_private_directory(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn set_private_directory(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}
