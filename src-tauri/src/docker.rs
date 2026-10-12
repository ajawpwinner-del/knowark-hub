// Docker 管理：经 docker CLI 完成拉取/构建/运行/停止/卸载；容器统一命名 knowark-<id> 并打标签
use crate::catalog::{self, ServiceDef};
use crate::proc;
use crate::settings::Settings;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceState {
    pub id: String,
    /// not_installed / stopped / running / starting / error
    pub status: String,
    pub host_port: u16,
    pub image: String,
    pub use_gpu: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub id: String,
    pub stage: String,
    pub line: String,
    pub done: bool,
    pub error: Option<String>,
}

pub fn effective_image(def: &ServiceDef, s: &Settings) -> String {
    let o = s.service(&def.id);
    if let Some(img) = o.image.filter(|i| !i.trim().is_empty()) {
        return img;
    }
    if o.use_gpu {
        def.image_gpu.clone().unwrap_or_else(|| def.image.clone())
    } else {
        def.image.clone()
    }
}

/// 国内可直连的 Docker Hub 镜像源：直连失败（无代理、被墙）时依次重试
const FALLBACK_MIRRORS: [&str; 2] = ["docker.m.daocloud.io", "docker.1ms.run"];

/// 拉取候选：用户指定的加速源优先，其次直连，再依次国内镜像源
fn pull_candidates(image: &str, docker_hub: bool, s: &Settings) -> Vec<String> {
    if !docker_hub {
        // 非 Docker Hub 镜像没有替代源：大镜像偶发超时，原地重试
        return vec![image.to_string(); 10];
    }
    // Docker Hub 官方镜像在加速源下需带 library/ 前缀
    let path = if image.contains('/') { image.to_string() } else { format!("library/{image}") };
    let user = s.registry_mirror.trim().trim_end_matches('/').to_string();
    let mut out: Vec<String> = Vec::new();
    if !user.is_empty() {
        out.push(format!("{user}/{path}"));
    }
    out.push(image.to_string());
    for m in FALLBACK_MIRRORS {
        let c = format!("{m}/{path}");
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

pub async fn docker_ready() -> bool {
    proc::run("docker", &["info", "--format", "{{.ServerVersion}}"]).await.ok
}

pub async fn state(def: &ServiceDef, s: &Settings) -> ServiceState {
    let name = catalog::container_name(&def.id);
    let o = s.service(&def.id);
    let out = proc::run("docker", &["inspect", "-f", "{{.State.Status}}", &name]).await;
    let (status, detail) = if !out.ok {
        ("not_installed".to_string(), String::new())
    } else {
        let mut parts = out.stdout.split('|');
        let st = parts.next().unwrap_or("").to_string();
        match st.as_str() {
            "running" => ("running".into(), String::new()),
            "created" | "restarting" => ("starting".into(), st),
            "exited" | "paused" => ("stopped".into(), String::new()),
            "dead" => ("error".into(), "容器异常退出".into()),
            other => ("stopped".into(), other.to_string()),
        }
    };
    ServiceState {
        id: def.id.clone(),
        status,
        host_port: s.host_port(&def.id),
        image: effective_image(def, s),
        use_gpu: o.use_gpu,
        detail,
    }
}

fn emit(app: &AppHandle, id: &str, stage: &str, line: &str, done: bool, error: Option<String>) {
    let _ = app.emit(
        "task-progress",
        TaskProgress { id: id.into(), stage: stage.into(), line: line.into(), done, error },
    );
}

/// 运行命令并把输出逐行推给前端（docker pull/build 的进度）
async fn run_streaming(app: &AppHandle, id: &str, stage: &str, program: &str, args: &[String]) -> anyhow::Result<()> {
    let mut child = proc::command(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");
    let (app1, app2) = (app.clone(), app.clone());
    let (id1, id2, st1, st2) = (id.to_string(), id.to_string(), stage.to_string(), stage.to_string());
    let t1 = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            emit(&app1, &id1, &st1, &l, false, None);
        }
    });
    let t2 = tokio::spawn(async move {
        let mut last = String::new();
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            emit(&app2, &id2, &st2, &l, false, None);
            last = l;
        }
        last
    });
    let status = child.wait().await?;
    let _ = t1.await;
    let last_err = t2.await.unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!(if last_err.is_empty() { format!("{program} 退出码 {status}") } else { last_err })
    }
}

pub const SEARXNG_DEFAULT_ENGINES: &str = "bing,baidu,sogou,360search,wikipedia,duckduckgo";

/// 按选项生成 SearXNG 配置：只保留所选引擎、开启 JSON 输出（知舟靠它）
fn searxng_settings(secret: &str, s: &Settings) -> String {
    let engines = s.option("searxng", "engines", SEARXNG_DEFAULT_ENGINES);
    let keep: String = engines.split(',').map(str::trim).filter(|e| !e.is_empty()).map(|e| format!("      - {e}
")).collect();
    let lang = s.option("searxng", "language", "zh-CN");
    let safe = s.option("searxng", "safe_search", "0");
    format!(
        "# 由知舟服务台生成，请在服务台「我的服务 → 网络搜索 → 设置」中修改
use_default_settings:
  engines:
    keep_only:
{keep}server:
  secret_key: \"{secret}\"
  limiter: false
  image_proxy: false
search:
  safe_search: {safe}
  default_lang: \"{lang}\"
  formats:
    - html
    - json
"
    )
}

/// 写入（或按新选项重写）SearXNG 配置；沿用已有 secret_key
pub fn write_searxng_config(s: &Settings) -> anyhow::Result<()> {
    let dir = Path::new(&s.data_dir).join("searxng");
    std::fs::create_dir_all(&dir)?;
    let file = dir.join("settings.yml");
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    let secret = old
        .lines()
        .find_map(|l| l.trim().strip_prefix("secret_key:").map(|v| v.trim().trim_matches('"').to_string()))
        .unwrap_or_else(|| {
            use rand::{distributions::Alphanumeric, Rng};
            rand::thread_rng().sample_iter(&Alphanumeric).take(32).map(char::from).collect()
        });
    std::fs::write(file, searxng_settings(&secret, s))?;
    Ok(())
}

fn resource_dir(app: &AppHandle) -> PathBuf {
    app.path().resource_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn prepare_binds(def: &ServiceDef, s: &Settings) -> anyhow::Result<()> {
    if def.id == "searxng" {
        write_searxng_config(s)?;
    }
    Ok(())
}

/// 安装：拉取或构建镜像，然后创建并启动容器
pub async fn install(app: &AppHandle, def: &ServiceDef, s: &Settings) -> anyhow::Result<()> {
    let image = effective_image(def, s);
    let o = s.service(&def.id);
    if let Some(build) = &def.build {
        emit(app, &def.id, "build", "开始在本机构建镜像（首次约需 10–20 分钟）", false, None);
        let dir = resource_dir(app).join("docker");
        let dockerfile = dir.join(format!("{build}.Dockerfile"));
        let args = vec![
            "build".to_string(),
            "-t".into(),
            image.clone(),
            "-f".into(),
            dockerfile.to_string_lossy().into_owned(),
            "--build-arg".into(),
            format!("PIP_INDEX={}", s.pip_index),
            dir.to_string_lossy().into_owned(),
        ];
        run_streaming(app, &def.id, "build", "docker", &args).await?;
    } else {
        let candidates = pull_candidates(&image, def.docker_hub, s);
        let mut last_err = None;
        for (i, src) in candidates.iter().enumerate() {
            let note = if i == 0 { format!("正在下载镜像 {src}") } else if candidates[i - 1] == *src { format!("下载中断，第 {i} 次重试（已下载的部分会保留）") } else { format!("上一个源连不上，改用 {src}") };
            emit(app, &def.id, "pull", &note, false, None);
            match run_streaming(app, &def.id, "pull", "docker", &["pull".into(), src.clone()]).await {
                Ok(()) => {
                    if src != &image {
                        // 标回原名：容器、状态显示与后续升级都只认原镜像名
                        let _ = proc::run("docker", &["tag", src, &image]).await;
                    }
                    last_err = None;
                    break;
                }
                Err(e) => last_err = Some(e),
            }
        }
        if let Some(e) = last_err {
            anyhow::bail!("所有下载源都失败了：{e}。可在「设置 → 下载加速」换一个镜像源，或在 Docker Desktop 设置里配置代理");
        }
    }
    prepare_binds(def, s)?;
    // 已有同名容器（如改了端口重装）先移除，数据卷保留
    let _ = proc::run("docker", &["rm", "-f", &catalog::container_name(&def.id)]).await;

    let port = s.host_port(&def.id);
    let mut args: Vec<String> = vec![
        "run".into(),
        "-d".into(),
        "--name".into(),
        catalog::container_name(&def.id),
        "--label".into(),
        "knowark.hub=1".into(),
        "--restart".into(),
        "unless-stopped".into(),
        "-p".into(),
        format!("127.0.0.1:{port}:{}", def.container_port),
    ];
    for v in &def.volumes {
        let src = match (&v.name, &v.bind) {
            (Some(n), _) => n.clone(),
            (None, Some(b)) => Path::new(&s.data_dir).join(b).to_string_lossy().into_owned(),
            _ => continue,
        };
        args.push("-v".into());
        args.push(format!("{src}:{}", v.target));
    }
    for (k, v) in &def.env {
        let v = if k == "SEARXNG_BASE_URL" { format!("http://localhost:{port}/") } else { v.clone() };
        args.push("-e".into());
        args.push(format!("{k}={v}"));
    }
    // 容器联网走 Docker Desktop 内置转发代理：它会转给 Docker Desktop 里配置的代理，未配置时直连。
    // 否则 Ollama 下载模型、SearXNG 访问境外引擎在国内网络下会失败
    for (k, v) in [
        ("HTTP_PROXY", "http://http.docker.internal:3128"),
        ("HTTPS_PROXY", "http://http.docker.internal:3128"),
        ("NO_PROXY", "localhost,127.0.0.1,host.docker.internal"),
    ] {
        args.push("-e".into());
        args.push(format!("{k}={v}"));
    }
    if o.use_gpu && def.gpu != "none" {
        args.push("--gpus".into());
        args.push("all".into());
    }
    if def.id == "mineru" {
        args.push("--shm-size".into());
        args.push("2g".into());
    }
    args.push(image);
    args.extend(def.command.iter().cloned());
    emit(app, &def.id, "run", "正在启动服务", false, None);
    run_streaming(app, &def.id, "run", "docker", &args).await?;
    emit(app, &def.id, "run", "服务已启动", true, None);
    Ok(())
}

pub async fn start(id: &str) -> anyhow::Result<()> {
    simple(&["start", &catalog::container_name(id)]).await
}

pub async fn stop(id: &str) -> anyhow::Result<()> {
    simple(&["stop", &catalog::container_name(id)]).await
}

pub async fn restart(id: &str) -> anyhow::Result<()> {
    simple(&["restart", &catalog::container_name(id)]).await
}

/// 卸载：删容器；remove_data 为真时连同数据卷与绑定目录一起删除
pub async fn uninstall(def: &ServiceDef, s: &Settings, remove_data: bool) -> anyhow::Result<()> {
    let _ = proc::run("docker", &["rm", "-f", &catalog::container_name(&def.id)]).await;
    if remove_data {
        for v in &def.volumes {
            if let Some(n) = &v.name {
                let _ = proc::run("docker", &["volume", "rm", n]).await;
            }
            if let Some(b) = &v.bind {
                let _ = std::fs::remove_dir_all(Path::new(&s.data_dir).join(b));
            }
        }
    }
    Ok(())
}

pub async fn logs(id: &str, tail: u32) -> String {
    let t = tail.to_string();
    let o = proc::run("docker", &["logs", "--tail", &t, "--timestamps", &catalog::container_name(id)]).await;
    // docker logs 把容器 stderr 写到 stderr，两路合并展示
    let mut s = o.stdout;
    if !o.stderr.is_empty() {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(&o.stderr);
    }
    s
}

async fn simple(args: &[&str]) -> anyhow::Result<()> {
    let o = proc::run("docker", args).await;
    if o.ok {
        Ok(())
    } else {
        anyhow::bail!(if o.stderr.is_empty() { "Docker 命令失败".to_string() } else { o.stderr })
    }
}
