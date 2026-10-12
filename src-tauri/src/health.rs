// 健康检查：经网关、按知舟实际调用的协议真实跑一次，结果即「知舟能不能用」
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub ok: bool,
    pub detail: String,
    pub ms: u128,
}


pub async fn probe(id: &str, gateway_port: u16, key: &str, upstream_port: u16) -> Health {
    let started = Instant::now();
    let base = format!("http://127.0.0.1:{gateway_port}");
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .expect("reqwest");
    let result: Result<String, String> = async {
        match id {
            "searxng" => {
                let v: Value = client
                    .get(format!("{base}/search"))
                    .query(&[("q", "知舟 KnowArk")])
                    .basic_auth("knowark", Some(key))
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|_| "返回的不是 JSON：检查 settings.yml 是否开启 json 格式".to_string())?;
                let n = v["results"].as_array().map(|a| a.len()).unwrap_or(0);
                if n == 0 {
                    Err("搜索可用但没有结果：可能是搜索引擎在当前网络下不可达".into())
                } else {
                    Ok(format!("搜到 {n} 条结果"))
                }
            }
            "ollama" => {
                let tags: Value = client
                    .get(format!("http://127.0.0.1:{upstream_port}/api/tags"))
                    .send()
                    .await
                    .map_err(|_| "服务未响应".to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
                let models = tags["models"].as_array().cloned().unwrap_or_default();
                // 逐个试已下载模型，能向量化的就是向量模型（不依赖内置清单，新模型也能识别）
                for name in models.iter().filter_map(|m| m["name"].as_str()) {
                    let v: Value = match client
                        .post(format!("{base}/v1/embeddings"))
                        .bearer_auth(key)
                        .json(&json!({ "model": name, "input": "知舟" }))
                        .send()
                        .await
                    {
                        Ok(r) => r.json().await.unwrap_or(Value::Null),
                        Err(e) => return Err(e.to_string()),
                    };
                    let d = v["data"][0]["embedding"].as_array().map(|a| a.len()).unwrap_or(0);
                    if d > 0 {
                        return Ok(format!("{name} 向量化正常（{d} 维）"));
                    }
                }
                Ok(format!("运行中，已有 {} 个模型；还没有向量模型", models.len()))
            }
            "mineru" => {
                let r = client
                    .get(format!("http://127.0.0.1:{upstream_port}/docs"))
                    .send()
                    .await
                    .map_err(|_| "服务未响应：首次启动需要加载模型，请稍候再试".to_string())?;
                if r.status().is_success() {
                    Ok("解析服务已就绪".into())
                } else {
                    Err(format!("服务返回 {}", r.status()))
                }
            }
            _ => Err("未知服务".into()),
        }
    }
    .await;
    let ms = started.elapsed().as_millis();
    match result {
        Ok(detail) => Health { ok: true, detail, ms },
        Err(detail) => Health { ok: false, detail, ms },
    }
}
