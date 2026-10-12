// 兼容网关：把本机各容器统一成知舟能直接填写的协议，只需一个地址加一个 Key。
//   POST /v1/documents/parse   知舟「通用 REST」文档处理协议 → MinerU（PDF / 图片 / Office）
//   POST /v1/embeddings        OpenAI 兼容 → Ollama（校验 dimensions，避免知舟因维度不符拒绝入库）
//   GET  /v1/models、POST /v1/chat/completions → Ollama（流式透传）
//   GET  /search?q=..&format=json  SearXNG 协议（Basic 认证，密码即 Key）
// 各容器只监听 127.0.0.1，局域网只暴露本网关；网关日志不记录请求内容与 Key。
use crate::settings::Settings;
use axum::{
    body::Body,
    extract::{Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;

#[derive(Clone)]
pub struct Shared {
    pub key: Arc<RwLock<String>>,
    pub settings: Arc<RwLock<Settings>>,
    client: reqwest::Client,
}

impl Shared {
    pub fn new(key: String, settings: Arc<RwLock<Settings>>) -> Self {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .expect("reqwest");
        Self { key: Arc::new(RwLock::new(key)), settings, client }
    }

    fn upstream(&self, id: &str) -> String {
        let port = self.settings.read().unwrap().host_port(id);
        format!("http://127.0.0.1:{port}")
    }
}

pub struct Gateway {
    shutdown: Option<oneshot::Sender<()>>,
    pub port: u16,
    pub error: Option<String>,
}

impl Gateway {
    pub fn idle() -> Self {
        Self { shutdown: None, port: 0, error: None }
    }

    pub async fn start(shared: Shared, port: u16) -> Self {
        let app = router(shared);
        match tokio::net::TcpListener::bind(("0.0.0.0", port)).await {
            Ok(listener) => {
                let (tx, rx) = oneshot::channel::<()>();
                tokio::spawn(async move {
                    let _ = axum::serve(listener, app)
                        .with_graceful_shutdown(async {
                            let _ = rx.await;
                        })
                        .await;
                });
                Self { shutdown: Some(tx), port, error: None }
            }
            Err(e) => Self { shutdown: None, port, error: Some(format!("端口 {port} 无法监听：{e}")) },
        }
    }

    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

fn router(shared: Shared) -> Router {
    let protected = Router::new()
        .route("/v1/documents/parse", post(parse))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/models", get(models))
        .route("/v1/chat/completions", post(chat))
        .route("/search", get(search))
        .layer(middleware::from_fn_with_state(shared.clone(), auth))
        .layer(axum::extract::DefaultBodyLimit::max(200 * 1024 * 1024));
    Router::new()
        .route("/health", get(|| async { Json(json!({ "ok": true, "name": "KnowArk Hub" })) }))
        .merge(protected)
        .with_state(shared)
}

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({ "success": false, "message": msg.into() }))).into_response()
}

fn openai_err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({ "error": { "message": msg.into(), "type": "knowark_hub" } }))).into_response()
}

async fn auth(State(s): State<Shared>, req: Request, next: Next) -> Response {
    let key = s.key.read().unwrap().clone();
    let header = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).unwrap_or("");
    let ok = if let Some(t) = header.strip_prefix("Bearer ") {
        t.trim() == key
    } else if let Some(b) = header.strip_prefix("Basic ") {
        base64::engine::general_purpose::STANDARD
            .decode(b.trim())
            .ok()
            .and_then(|v| String::from_utf8(v).ok())
            .map(|up| up.split_once(':').map(|(_, p)| p == key).unwrap_or(false))
            .unwrap_or(false)
    } else {
        false
    };
    if ok {
        next.run(req).await
    } else {
        let mut r = err(StatusCode::UNAUTHORIZED, "Key 不正确：请在知舟中填写服务台「连接知舟」页显示的 Key");
        if req.uri().path() == "/search" {
            r.headers_mut().insert(header::WWW_AUTHENTICATE, "Basic realm=\"KnowArk Hub\"".parse().unwrap());
        }
        r
    }
}

// ---------- 文档解析 ----------

#[derive(Deserialize)]
struct ParseReq {
    file: String,
    format: Option<String>,
    model: Option<String>,
    filename: Option<String>,
}


async fn parse(State(s): State<Shared>, Json(req): Json<ParseReq>) -> Response {
    let format = req.format.clone().unwrap_or_default().to_ascii_lowercase();
    let bytes = match base64::engine::general_purpose::STANDARD.decode(req.file.trim()) {
        Ok(b) => b,
        Err(_) => return err(StatusCode::BAD_REQUEST, "file 不是有效的 Base64"),
    };
    // PDF、图片、Office 统一交给 MinerU（其内置 OCR 模型可识别图片与扫描件）
    let _ = req.model;
    let result = mineru(&s, bytes, &format, req.filename.as_deref()).await;
    match result {
        Ok((markdown, pages)) => Json(json!({
            "success": true,
            "data": { "markdown": markdown, "pageCount": pages, "format": "markdown" }
        }))
        .into_response(),
        Err(e) => err(StatusCode::BAD_GATEWAY, e),
    }
}

async fn mineru(s: &Shared, bytes: Vec<u8>, format: &str, filename: Option<&str>) -> Result<(String, Option<usize>), String> {
    let name = filename.map(str::to_string).unwrap_or_else(|| format!("document.{}", if format.is_empty() { "pdf" } else { format }));
    let part = reqwest::multipart::Part::bytes(bytes).file_name(name);
    let settings = s.settings.read().unwrap().clone();
    let opt = |k: &str, d: &str| settings.option("mineru", k, d);
    let form = reqwest::multipart::Form::new()
        .part("files", part)
        .text("return_md", "true")
        .text("backend", "pipeline")
        .text("lang_list", opt("lang", "ch"))
        .text("formula_enable", opt("formula", "true"))
        .text("table_enable", opt("table", "true"));
    let resp = s
        .client
        .post(format!("{}/file_parse", s.upstream("mineru")))
        .multipart(form)
        .send()
        .await
        .map_err(|_| "文档解析服务未运行：请在服务台启动 MinerU".to_string())?;
    let status = resp.status();
    let v: Value = resp.json().await.map_err(|e| format!("MinerU 返回无法解析：{e}"))?;
    if !status.is_success() {
        return Err(format!("MinerU：{}", v["detail"].as_str().or(v["error"].as_str()).unwrap_or("解析失败")));
    }
    let md = v["results"]
        .as_object()
        .and_then(|m| m.values().next())
        .and_then(|r| r["md_content"].as_str())
        .or_else(|| v["md_content"].as_str())
        .ok_or("MinerU 未返回 Markdown")?;
    Ok((md.to_string(), None))
}

// ---------- 向量 ----------

/// 支持截取维度（Matryoshka）的模型：输出维度大于所需时截取并重新归一化
fn supports_truncation(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.starts_with("qwen3-embedding") || m.starts_with("nomic-embed-text")
}

async fn embeddings(State(s): State<Shared>, Json(body): Json<Value>) -> Response {
    let want = body["dimensions"].as_u64().map(|d| d as usize);
    let model = body["model"].as_str().unwrap_or("").to_string();
    let resp = match s.client.post(format!("{}/v1/embeddings", s.upstream("ollama"))).json(&body).send().await {
        Ok(r) => r,
        Err(_) => return openai_err(StatusCode::BAD_GATEWAY, "本地模型服务未运行：请在服务台启动 Ollama"),
    };
    let status = resp.status();
    let mut v: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => return openai_err(StatusCode::BAD_GATEWAY, format!("Ollama 返回无法解析：{e}")),
    };
    if !status.is_success() {
        return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY), Json(v)).into_response();
    }
    if let (Some(d), Some(data)) = (want, v["data"].as_array_mut()) {
        for item in data.iter_mut() {
            let vec: Vec<f64> = item["embedding"].as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).collect()).unwrap_or_default();
            if vec.len() == d {
                continue;
            }
            if vec.len() > d && supports_truncation(&model) {
                let cut = &vec[..d];
                let norm = cut.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-12);
                item["embedding"] = json!(cut.iter().map(|x| x / norm).collect::<Vec<_>>());
            } else {
                return openai_err(
                    StatusCode::BAD_REQUEST,
                    format!("模型 {model} 输出 {} 维，与请求的 {d} 维不符：请在知舟中把维度改为 {}", vec.len(), vec.len()),
                );
            }
        }
    }
    Json(v).into_response()
}

async fn models(State(s): State<Shared>) -> Response {
    forward_get(&s, &format!("{}/v1/models", s.upstream("ollama")), "本地模型服务未运行").await
}

async fn chat(State(s): State<Shared>, Json(body): Json<Value>) -> Response {
    match s.client.post(format!("{}/v1/chat/completions", s.upstream("ollama"))).json(&body).send().await {
        Ok(r) => stream_back(r),
        Err(_) => openai_err(StatusCode::BAD_GATEWAY, "本地模型服务未运行：请在服务台启动 Ollama"),
    }
}

// ---------- 搜索 ----------

async fn search(State(s): State<Shared>, Query(mut q): Query<BTreeMap<String, String>>) -> Response {
    q.insert("format".into(), "json".into());
    let mut url = url::Url::parse(&format!("{}/search", s.upstream("searxng"))).unwrap();
    url.query_pairs_mut().extend_pairs(q.iter());
    forward_get(&s, url.as_str(), "网络搜索服务未运行：请在服务台启动 SearXNG").await
}

// ---------- 通用转发 ----------

async fn forward_get(s: &Shared, url: &str, down_msg: &str) -> Response {
    match s.client.get(url).header(header::ACCEPT, "application/json").send().await {
        Ok(r) => stream_back(r),
        Err(_) => err(StatusCode::BAD_GATEWAY, down_msg),
    }
}

fn stream_back(r: reqwest::Response) -> Response {
    let status = StatusCode::from_u16(r.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut headers = HeaderMap::new();
    if let Some(ct) = r.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        if let Ok(v) = ct.parse() {
            headers.insert(header::CONTENT_TYPE, v);
        }
    }
    (status, headers, Body::from_stream(r.bytes_stream())).into_response()
}
