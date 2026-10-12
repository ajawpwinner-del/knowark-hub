// 「试一试」：经网关、按知舟实际协议调用，让小白不打开知舟就能确认服务可用
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub content: String,
    pub engine: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResult {
    pub markdown: String,
    pub page_count: Option<u64>,
    pub ms: u128,
    pub truncated: bool,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().timeout(std::time::Duration::from_secs(900)).build().expect("reqwest")
}

pub async fn search(gateway_port: u16, key: &str, q: &str) -> anyhow::Result<Vec<SearchHit>> {
    let v: Value = client()
        .get(format!("http://127.0.0.1:{gateway_port}/search"))
        .query(&[("q", q)])
        .basic_auth("knowark", Some(key))
        .send()
        .await?
        .json()
        .await?;
    if let Some(m) = v["message"].as_str() {
        anyhow::bail!("{m}");
    }
    Ok(v["results"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .take(10)
        .map(|r| SearchHit {
            title: r["title"].as_str().unwrap_or("").into(),
            url: r["url"].as_str().unwrap_or("").into(),
            content: r["content"].as_str().unwrap_or("").into(),
            engine: r["engine"].as_str().unwrap_or("").into(),
        })
        .collect())
}

/// engine：保留参数，当前统一由 MinerU 处理
pub async fn parse(gateway_port: u16, key: &str, path: &str, engine: &str) -> anyhow::Result<ParseResult> {
    let started = std::time::Instant::now();
    let bytes = tokio::fs::read(path).await?;
    let p = std::path::Path::new(path);
    let format = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let filename = p.file_name().map(|f| f.to_string_lossy().into_owned());
    let body = json!({
        "file": base64::engine::general_purpose::STANDARD.encode(&bytes),
        "format": format,
        "filename": filename,
        "model": if engine == "auto" { "" } else { engine },
    });
    let v: Value = client()
        .post(format!("http://127.0.0.1:{gateway_port}/v1/documents/parse"))
        .bearer_auth(key)
        .json(&body)
        .send()
        .await?
        .json()
        .await?;
    if v["success"].as_bool() != Some(true) {
        anyhow::bail!("{}", v["message"].as_str().unwrap_or("解析失败"));
    }
    let md = v["data"]["markdown"].as_str().unwrap_or("").to_string();
    // 预览只回传前 2 万字，避免超大文档卡住界面
    let truncated = md.chars().count() > 20000;
    let markdown = if truncated { md.chars().take(20000).collect() } else { md };
    Ok(ParseResult { markdown, page_count: v["data"]["pageCount"].as_u64(), ms: started.elapsed().as_millis(), truncated })
}
