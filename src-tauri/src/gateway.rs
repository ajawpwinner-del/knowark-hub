// 兼容网关：把本机各容器统一成知舟能直接填写的协议，只需一个地址加一个 Key。
//   POST /v1/documents/parse   知舟「通用 REST」文档处理协议 → MinerU（PDF / 图片 / Office）
//   POST /v1/embeddings        OpenAI 兼容 → Ollama（校验 dimensions，避免知舟因维度不符拒绝入库）
//   GET  /v1/models、POST /v1/chat/completions → Ollama（流式透传）
//   GET  /search?q=..&format=json  SearXNG 协议（Basic 认证，密码即 Key）
// 各容器只监听 127.0.0.1，局域网只暴露本网关；网关日志不记录请求内容与 Key。
use crate::settings::Settings;
use axum::{
    body::Body,
    extract::{OriginalUri, Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get, post},
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
    pub client: reqwest::Client,
    /// 向量模型实测维度缓存（模型名 → 维度）：首次实测要加载模型，之后直接返回
    dims: Arc<RwLock<BTreeMap<String, usize>>>,
}

impl Shared {
    /// 模型原生维度（实测一次后缓存；0 = 不能向量化，即对话模型）
    pub async fn native_dims(&self, port: u16, model: &str) -> usize {
        if let Some(d) = self.dims.read().unwrap().get(model).copied() {
            return d;
        }
        let d = crate::ollama::embed_dims(port, model).await.unwrap_or(0);
        self.dims.write().unwrap().insert(model.to_string(), d);
        d
    }

    /// 已下载模型中可向量化的那些，含原生 / 实际输出维度
    pub async fn embedding_models(&self) -> Vec<crate::ollama::EmbeddingModelInfo> {
        let settings = self.settings.read().unwrap().clone();
        let port = settings.host_port("ollama");
        let mut out = Vec::new();
        if let Ok(models) = crate::ollama::list(port).await {
            for m in models {
                let native = self.native_dims(port, &m.name).await;
                if native == 0 {
                    continue;
                }
                let truncatable = crate::ollama::truncatable(&m.name);
                out.push(crate::ollama::EmbeddingModelInfo {
                    dims: crate::ollama::effective_dims(&settings, &m.name, native),
                    options: crate::ollama::dim_options(native, truncatable),
                    model: m.name,
                    native_dims: native,
                    truncatable,
                });
            }
        }
        out
    }
}

impl Shared {
    pub fn new(key: String, settings: Arc<RwLock<Settings>>) -> Self {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .expect("reqwest");
        Self { key: Arc::new(RwLock::new(key)), settings, client, dims: Arc::new(RwLock::new(BTreeMap::new())) }
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
        .route("/v1/hub/info", get(hub_info))
        // MinerU 4 V1 原生接口透传（知舟 open-mineru 处理器：上传 → 解析任务 → 轮询 → 取结果，长文档不受单次请求超时限制）
        .route("/v1/uploads", any(mineru_v1))
        .route("/v1/uploads/*rest", any(mineru_v1))
        .route("/v1/parse/jobs", any(mineru_v1))
        .route("/v1/parse/jobs/*rest", any(mineru_v1))
        .route("/v1/files/*rest", any(mineru_v1))
        .route("/v1/tiers", get(mineru_v1))
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
    // MinerU 4：POST /v1/file_parse（原始字节 + 查询参数），同步等待最长 1 小时；结果在 files[0].content.markdown
    let name = filename.map(str::to_string).unwrap_or_else(|| format!("document.{}", if format.is_empty() { "pdf" } else { format }));
    let settings = s.settings.read().unwrap().clone();
    let tier = settings.option("mineru", "tier", "standard");
    let ocr_mode = settings.option("mineru", "ocr_mode", "auto");
    let mut url = url::Url::parse(&format!("{}/v1/file_parse", s.upstream("mineru"))).unwrap();
    url.query_pairs_mut()
        .append_pair("filename", &name)
        .append_pair("output_formats", "markdown")
        .append_pair("response_format", "json")
        .append_pair("tier", &tier)
        .append_pair("ocr_mode", &ocr_mode)
        .append_pair("wait_timeout", "3600");
    let resp = s
        .client
        .post(url)
        .header(header::CONTENT_TYPE.as_str(), "application/octet-stream")
        .body(bytes)
        .send()
        .await
        .map_err(|_| "文档解析服务未运行：请在服务台启动 MinerU".to_string())?;
    let status = resp.status();
    let v: Value = resp.json().await.map_err(|e| format!("MinerU 返回无法解析：{e}"))?;
    if status.as_u16() == 202 {
        return Err("文档较长，解析超过 1 小时仍未完成；建议在知舟改用「MinerU 本地部署」（扫码配对会自动设置）".into());
    }
    if !status.is_success() {
        let msg = v["error"]["message"].as_str().or(v["detail"].as_str()).unwrap_or("解析失败");
        return Err(format!("MinerU：{msg}"));
    }
    let file = &v["files"][0];
    if let Some(e) = file["error"]["message"].as_str() {
        return Err(format!("MinerU：{e}"));
    }
    let md = file["content"]["markdown"].as_str().ok_or("MinerU 未返回 Markdown")?;
    Ok((md.to_string(), None))
}

// ---------- 向量 ----------

async fn embeddings(State(s): State<Shared>, Json(body): Json<Value>) -> Response {
    let model = body["model"].as_str().unwrap_or("").to_string();
    // 知舟未指定维度（服务商默认）时，按服务台「本地模型」里为该模型设的输出维度截取
    let want = match body["dimensions"].as_u64() {
        Some(d) => Some(d as usize),
        None => {
            let port = s.settings.read().unwrap().host_port("ollama");
            let native = s.native_dims(port, &model).await;
            let settings = s.settings.read().unwrap().clone();
            let eff = crate::ollama::effective_dims(&settings, &model, native);
            if native > 0 && eff < native { Some(eff) } else { None }
        }
    };
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
            if vec.len() > d && crate::ollama::truncatable(&model) {
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

// ---------- 能力清单（知舟扫码配对用，v0.51 FR-009） ----------

/// 返回服务台此刻能提供的能力：哪些服务在运行、有哪些可向量化的模型及实测维度
async fn hub_info(State(s): State<Shared>) -> Response {
    let settings = s.settings.read().unwrap().clone();
    let mut running = BTreeMap::new();
    for def in crate::catalog::services() {
        let st = crate::docker::state(def, &settings).await;
        running.insert(def.id.clone(), st.status == "running");
    }
    let mut embedding = Vec::new();
    let mut chat = false;
    if running.get("ollama").copied().unwrap_or(false) {
        let infos = s.embedding_models().await;
        for m in &infos {
            embedding.push(json!({ "model": m.model, "dims": m.dims }));
        }
        let port = settings.host_port("ollama");
        if let Ok(models) = crate::ollama::list(port).await {
            chat = models.len() > infos.len();
        }
    }
    Json(json!({
        "name": "知舟服务台",
        "version": env!("CARGO_PKG_VERSION"),
        "services": {
            "docParse": running.get("mineru").copied().unwrap_or(false),
            "search": running.get("searxng").copied().unwrap_or(false),
            "chat": chat
        },
        "embeddingModels": embedding
    }))
    .into_response()
}

// ---------- MinerU 4 V1 透传 ----------

/// 原样转发到 MinerU；JSON 响应里 MinerU 自报的本机地址（如 upload_url 的 http://127.0.0.1:8800）
/// 改写成手机访问网关用的地址，知舟才会把文件传到网关、并把 Key 带上（同源才带）
async fn mineru_v1(State(s): State<Shared>, OriginalUri(uri): OriginalUri, req: Request) -> Response {
    let method = req.method().clone();
    let headers = req.headers().clone();
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let body = match axum::body::to_bytes(req.into_body(), 200 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return err(StatusCode::PAYLOAD_TOO_LARGE, "文件超过 200 MB"),
    };
    let upstream = s.upstream("mineru");
    let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    let mut rb = s.client.request(reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
        format!("{upstream}{path}"));
    for name in [header::CONTENT_TYPE, header::ACCEPT] {
        if let Some(v) = headers.get(&name).and_then(|v| v.to_str().ok()) {
            rb = rb.header(name.as_str(), v);
        }
    }
    let resp = match rb.body(body.to_vec()).send().await {
        Ok(r) => r,
        Err(_) => return err(StatusCode::BAD_GATEWAY, "文档解析服务未运行：请在服务台启动 MinerU"),
    };
    let is_json = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("json"))
        .unwrap_or(false);
    if !is_json || host.is_empty() {
        return stream_back(resp);
    }
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let text = resp.text().await.unwrap_or_default();
    let port = s.settings.read().unwrap().host_port("mineru");
    let gateway = format!("http://{host}");
    let mut rewritten = text;
    for origin in [format!("http://127.0.0.1:{port}"), format!("http://localhost:{port}"),
        "http://127.0.0.1:8000".to_string(), "http://0.0.0.0:8000".to_string(), "http://localhost:8000".to_string()] {
        rewritten = rewritten.replace(&origin, &gateway);
    }
    (status, [(header::CONTENT_TYPE, "application/json")], rewritten).into_response()
}
