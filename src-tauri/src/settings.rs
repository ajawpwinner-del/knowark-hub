// 偏好设置：只存非敏感项（JSON 文件）；网关 Key 在 secrets.rs，进 Windows 凭据管理器
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServiceOverride {
    pub host_port: Option<u16>,
    pub image: Option<String>,
    pub use_gpu: bool,
    /// 服务专属选项（SearXNG 引擎、MinerU 语言等），键值见前端 serviceOptions
    pub options: BTreeMap<String, String>,
}

impl Default for ServiceOverride {
    fn default() -> Self {
        Self { host_port: None, image: None, use_gpu: false, options: BTreeMap::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// system / light / dark
    pub theme: String,
    pub gateway_port: u16,
    pub data_dir: String,
    /// Docker Hub 镜像加速前缀，如 docker.m.daocloud.io
    pub registry_mirror: String,
    /// MinerU 构建用的 pip 源
    pub pip_index: String,
    pub autostart: bool,
    pub services: BTreeMap<String, ServiceOverride>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            gateway_port: 8765,
            data_dir: default_data_dir().to_string_lossy().into_owned(),
            registry_mirror: String::new(),
            pip_index: "https://pypi.tuna.tsinghua.edu.cn/simple".into(),
            autostart: false,
            services: BTreeMap::new(),
        }
    }
}

/// 程序所在目录（便携：装在哪个盘，配置和数据就在哪个盘）；不可写时退回 %LOCALAPPDATA%\KnowArkHub
fn home_dir() -> PathBuf {
    use std::sync::OnceLock;
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| {
        let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
        if let Some(dir) = exe_dir {
            let probe = dir.join(".write-test");
            if std::fs::write(&probe, b"").is_ok() {
                let _ = std::fs::remove_file(probe);
                return dir;
            }
        }
        std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir()).join("KnowArkHub")
    })
    .clone()
}

fn default_data_dir() -> PathBuf {
    home_dir().join("data")
}

fn settings_path() -> PathBuf {
    home_dir().join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(settings_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let p = settings_path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn service(&self, id: &str) -> ServiceOverride {
        self.services.get(id).cloned().unwrap_or_default()
    }

    pub fn option(&self, id: &str, key: &str, default: &str) -> String {
        self.service(id).options.get(key).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| default.to_string())
    }

    pub fn host_port(&self, id: &str) -> u16 {
        self.service(id)
            .host_port
            .or_else(|| crate::catalog::find(id).map(|s| s.host_port))
            .unwrap_or(0)
    }
}
