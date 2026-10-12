// 外部命令执行：Windows 下隐藏控制台窗口，统一 UTF-8 输出
use std::process::Stdio;
use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 定位 docker.exe：Docker Desktop 可装在任意盘，且刚装完时本进程的 PATH 尚未刷新，
/// 依次查 PATH、注册表 InstallLocation、默认目录；找不到时退回 "docker"（不缓存失败，装好后可再检测到）
fn docker_exe() -> String {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    static FOUND: Mutex<Option<String>> = Mutex::new(None);
    if let Some(p) = FOUND.lock().unwrap().clone() {
        return p;
    }
    let mut candidates: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join("docker.exe")).collect())
        .unwrap_or_default();
    for key in [
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Docker Desktop",
        r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Docker Desktop",
    ] {
        let mut c = std::process::Command::new("reg");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(CREATE_NO_WINDOW);
        }
        if let Ok(o) = c.args(["query", key, "/v", "InstallLocation"]).output() {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            if let Some(dir) = text.lines().find_map(|l| l.split_once("REG_SZ").map(|(_, v)| v.trim().to_string())) {
                candidates.push(Path::new(&dir).join(r"resources\bin\docker.exe"));
            }
        }
    }
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    candidates.push(Path::new(&pf).join(r"Docker\Docker\resources\bin\docker.exe"));
    match candidates.into_iter().find(|p| p.is_file()) {
        Some(p) => {
            let s = p.to_string_lossy().into_owned();
            *FOUND.lock().unwrap() = Some(s.clone());
            s
        }
        None => "docker".into(),
    }
}

pub fn command(program: &str) -> Command {
    let exe = if program == "docker" { docker_exe() } else { program.to_string() };
    let mut c = Command::new(&exe);
    // docker pull 需要同目录的 docker-credential-desktop.exe，补进子进程 PATH
    if let Some(dir) = std::path::Path::new(&exe).parent().filter(|d| !d.as_os_str().is_empty()) {
        let mut paths = vec![dir.to_path_buf()];
        if let Some(p) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&p));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            c.env("PATH", joined);
        }
    }
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c.stdin(Stdio::null());
    c
}

pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

pub async fn run(program: &str, args: &[&str]) -> Output {
    match command(program).args(args).output().await {
        Ok(o) => Output {
            ok: o.status.success(),
            stdout: String::from_utf8_lossy(&o.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&o.stderr).trim().to_string(),
        },
        Err(e) => Output { ok: false, stdout: String::new(), stderr: e.to_string() },
    }
}
