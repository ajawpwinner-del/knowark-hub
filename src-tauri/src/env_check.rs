// 环境体检：系统、WSL2、Docker Desktop、内存、磁盘、NVIDIA 显卡
use crate::proc;
use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvItem {
    pub id: String,
    pub label: String,
    /// ok / warn / missing
    pub status: String,
    pub detail: String,
    pub action_label: Option<String>,
    pub action_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvReport {
    pub items: Vec<EnvItem>,
    pub docker_ready: bool,
    pub has_nvidia: bool,
    pub ram_gb: f64,
}

fn item(id: &str, label: &str, status: &str, detail: String) -> EnvItem {
    EnvItem { id: id.into(), label: label.into(), status: status.into(), detail, action_label: None, action_url: None }
}

fn with_action(mut i: EnvItem, label: &str, url: &str) -> EnvItem {
    i.action_label = Some(label.into());
    i.action_url = Some(url.into());
    i
}

pub async fn check(data_dir: &str) -> EnvReport {
    let mut items = Vec::new();

    let os = System::long_os_version().unwrap_or_else(|| "Windows".into());
    let build: u32 = System::kernel_version().and_then(|k| k.parse().ok()).unwrap_or(0);
    items.push(if build == 0 || build >= 19041 {
        item("os", "操作系统", "ok", os)
    } else {
        item("os", "操作系统", "missing", format!("{os}：Docker Desktop 需要 Windows 10 2004（19041）及以上"))
    });

    let mut sys = System::new();
    sys.refresh_memory();
    let ram_gb = sys.total_memory() as f64 / 1024f64.powi(3);
    items.push(if ram_gb >= 15.0 {
        item("ram", "内存", "ok", format!("{ram_gb:.0} GB，可以同时运行全部服务"))
    } else if ram_gb >= 7.0 {
        item("ram", "内存", "warn", format!("{ram_gb:.0} GB，建议一次只开一两个大服务"))
    } else {
        item("ram", "内存", "warn", format!("{ram_gb:.0} GB，偏少：建议只用网络搜索与小型向量模型"))
    });

    let disks = Disks::new_with_refreshed_list();
    let drive = data_dir.chars().next().unwrap_or('C').to_ascii_uppercase();
    let free = disks
        .list()
        .iter()
        .find(|d| d.mount_point().to_string_lossy().to_ascii_uppercase().starts_with(drive))
        .map(|d| d.available_space() as f64 / 1024f64.powi(3));
    items.push(match free {
        Some(f) if f >= 30.0 => item("disk", "磁盘空间", "ok", format!("{drive}: 盘剩余 {f:.0} GB")),
        Some(f) => item("disk", "磁盘空间", "warn", format!("{drive}: 盘剩余 {f:.0} GB，全部服务约需 30 GB")),
        None => item("disk", "磁盘空间", "warn", "无法读取磁盘信息".into()),
    });

    let wsl = proc::run("wsl", &["--status"]).await;
    items.push(if wsl.ok {
        item("wsl", "WSL 2", "ok", "已启用".into())
    } else {
        with_action(
            item("wsl", "WSL 2", "missing", "未启用：以管理员身份打开终端运行 wsl --install，然后重启电脑".into()),
            "查看教程",
            "https://learn.microsoft.com/zh-cn/windows/wsl/install",
        )
    });

    let ver = proc::run("docker", &["--version"]).await;
    let ready = crate::docker::docker_ready().await;
    items.push(if !ver.ok {
        with_action(
            item("docker", "Docker Desktop", "missing", "未安装：下载安装后启动一次，等左下角显示 Engine running".into()),
            "下载 Docker Desktop",
            "https://www.docker.com/products/docker-desktop/",
        )
    } else if !ready {
        item("docker", "Docker Desktop", "warn", "已安装但没有运行：请打开 Docker Desktop 并等待启动完成".into())
    } else {
        item("docker", "Docker Desktop", "ok", ver.stdout.replace("Docker version", "版本"))
    });

    let gpu = proc::run("nvidia-smi", &["--query-gpu=name,memory.total", "--format=csv,noheader"]).await;
    let has_nvidia = gpu.ok && !gpu.stdout.is_empty();
    items.push(if has_nvidia {
        item("gpu", "NVIDIA 显卡", "ok", format!("{}，可为本地模型与文档解析加速", gpu.stdout.lines().next().unwrap_or("")))
    } else {
        item("gpu", "NVIDIA 显卡", "warn", "未检测到：服务将以 CPU 运行，速度较慢但可用".into())
    });

    EnvReport { items, docker_ready: ready, has_nvidia, ram_gb }
}
