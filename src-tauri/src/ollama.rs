// Ollama 模型管理：列出、下载（进度推送）、删除、探测向量维度
use crate::docker::TaskProgress;
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub name: String,
    pub size_bytes: u64,
    pub family: String,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().expect("reqwest")
}

pub async fn list(port: u16) -> anyhow::Result<Vec<LocalModel>> {
    let v: Value = client().get(format!("http://127.0.0.1:{port}/api/tags")).send().await?.json().await?;
    Ok(v["models"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|m| LocalModel {
            name: m["name"].as_str().unwrap_or("").to_string(),
            size_bytes: m["size"].as_u64().unwrap_or(0),
            family: m["details"]["family"].as_str().unwrap_or("").to_string(),
        })
        .collect())
}

pub async fn pull(app: &AppHandle, port: u16, model: &str) -> anyhow::Result<()> {
    let task = format!("model:{model}");
    let resp = client()
        .post(format!("http://127.0.0.1:{port}/api/pull"))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await?;
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk?);
        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let Ok(v) = serde_json::from_slice::<Value>(&line) else { continue };
            if let Some(e) = v["error"].as_str() {
                anyhow::bail!("{e}");
            }
            let status = v["status"].as_str().unwrap_or("");
            let text = match (v["completed"].as_u64(), v["total"].as_u64()) {
                (Some(c), Some(t)) if t > 0 => format!("{:.0}%|{status}", c as f64 * 100.0 / t as f64),
                _ => status.to_string(),
            };
            let _ = app.emit(
                "task-progress",
                TaskProgress { id: task.clone(), stage: "pull".into(), line: text, done: false, error: None },
            );
        }
    }
    let _ = app.emit(
        "task-progress",
        TaskProgress { id: task, stage: "pull".into(), line: "下载完成".into(), done: true, error: None },
    );
    Ok(())
}

pub async fn delete(port: u16, model: &str) -> anyhow::Result<()> {
    let r = client()
        .delete(format!("http://127.0.0.1:{port}/api/delete"))
        .json(&json!({ "model": model }))
        .send()
        .await?;
    if !r.status().is_success() {
        anyhow::bail!("删除失败：{}", r.status());
    }
    Ok(())
}

/// 实际跑一次向量化，返回维度（知舟需要填写的值）
pub async fn embed_dims(port: u16, model: &str) -> anyhow::Result<usize> {
    let v: Value = client()
        .post(format!("http://127.0.0.1:{port}/api/embed"))
        .json(&json!({ "model": model, "input": "知舟" }))
        .send()
        .await?
        .json()
        .await?;
    if let Some(e) = v["error"].as_str() {
        anyhow::bail!("{e}");
    }
    Ok(v["embeddings"][0].as_array().map(|a| a.len()).unwrap_or(0))
}

// ---------- 在线模型库（ollama.com 无公开 API，解析其搜索页；失败时前端退回内置清单） ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryModel {
    pub name: String,
    pub description: String,
    /// 可选规格，如 ["270m", "570m"]；为空表示只有默认标签
    pub sizes: Vec<String>,
    pub capabilities: Vec<String>,
    pub pulls: String,
}

/// kind: embedding（按最新）/ chat（按热门）
pub async fn library(kind: &str) -> anyhow::Result<Vec<LibraryModel>> {
    let url = match kind {
        "embedding" => "https://ollama.com/search?c=embedding&o=newest",
        _ => "https://ollama.com/search?o=popular",
    };
    // 走系统代理（reqwest 默认读取 Windows 代理设置）
    let html = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let mut out = parse_library(&html);
    if kind != "embedding" {
        out.retain(|m| !m.capabilities.iter().any(|c| c == "embedding"));
    }
    out.truncate(16);
    if out.is_empty() {
        anyhow::bail!("模型库页面结构已变化，未解析到模型");
    }
    Ok(out)
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">")
}

fn parse_library(html: &str) -> Vec<LibraryModel> {
    use regex::Regex;
    use std::sync::OnceLock;
    static RE: OnceLock<[Regex; 4]> = OnceLock::new();
    let [name_re, desc_re, chip_re, pulls_re] = RE.get_or_init(|| {
        [
            Regex::new(r#"href="/library/([A-Za-z0-9._\-]+)""#).unwrap(),
            Regex::new(r#"<p class="[^"]*"[^>]*title="([^"]*)""#).unwrap(),
            Regex::new(r#"<span\s+class="([^"]*rounded-md[^"]*)"[^>]*>([^<]+)</span>"#).unwrap(),
            Regex::new(r#"title="([\d,\.]+[KMB]?) downloads""#).unwrap(),
        ]
    });
    html.split("<li")
        .skip(1)
        .filter_map(|block| {
            let name = name_re.captures(block)?.get(1)?.as_str().to_string();
            let description = desc_re.captures(block).map(|c| unescape(&c[1])).unwrap_or_default();
            let mut sizes = Vec::new();
            let mut capabilities = Vec::new();
            for c in chip_re.captures_iter(block) {
                let text = c[2].trim().to_string();
                if c[1].contains("ddf4ff") || c[1].contains("blue") {
                    sizes.push(text);
                } else {
                    capabilities.push(text);
                }
            }
            let pulls = pulls_re.captures(block).map(|c| c[1].to_string()).unwrap_or_default();
            Some(LibraryModel { name, description, sizes, capabilities, pulls })
        })
        .collect()
}

// ---------- 向量维度（服务台管理输出维度，v0.51 走查） ----------

/// 支持截取维度（Matryoshka）的模型：取前 N 维并重新归一化，效果损失小、体积大减
pub fn truncatable(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    ["qwen3-embedding", "nomic-embed-text", "embeddinggemma", "snowflake-arctic-embed2", "mxbai-embed-large"]
        .iter()
        .any(|p| m.starts_with(p))
}

/// 可选输出维度：原生维度及其下的常用档位
pub fn dim_options(native: usize, truncatable: bool) -> Vec<usize> {
    let mut out = vec![native];
    if truncatable {
        for d in [2048, 1536, 1024, 768, 512, 256] {
            if d < native {
                out.push(d);
            }
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelInfo {
    pub model: String,
    pub native_dims: usize,
    /// 知舟实际拿到的维度（用户设置的输出维度，未设置时为原生维度）
    pub dims: usize,
    pub truncatable: bool,
    pub options: Vec<usize>,
}

/// 设置里记的输出维度（键 dims:<模型名>）；非法或不可截取时回落原生维度
pub fn effective_dims(settings: &crate::settings::Settings, model: &str, native: usize) -> usize {
    let configured: usize = settings.option("ollama", &format!("dims:{model}"), "0").parse().unwrap_or(0);
    if configured > 0 && configured < native && truncatable(model) { configured } else { native }
}
