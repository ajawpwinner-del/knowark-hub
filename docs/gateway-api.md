# 网关接口

服务台网关默认监听 `0.0.0.0:8765`，是局域网里唯一可达的入口。它把各服务统一成知舟已经支持的协议，其他客户端也可以直接对接。

## 鉴权

除 `GET /health` 外，所有接口都要带 Key（在服务台「连接知舟」页查看或更换）：

- `Authorization: Bearer <Key>`；
- 或 HTTP Basic，用户名任意、密码为 Key（供只支持 Basic 的 SearXNG 客户端使用）。

Key 不对时返回 `401`，正文为 `{"success": false, "message": "…"}`。

## 配对码

服务台「连接知舟」页的二维码与「导出配置文件」内容同为：

```json
{ "kind": "knowark-hub", "v": 1, "gateway": "http://192.168.1.23:8765", "key": "ka-…" }
```

导出的文件额外带一个 `services` 字段（各服务的手填参考值），客户端应忽略不认识的字段。

## 状态与能力

### `GET /health`

无需鉴权，用于确认地址上是服务台：

```json
{ "ok": true, "name": "KnowArk Hub" }
```

### `GET /v1/hub/info`

服务台此刻能提供的能力。首次请求会逐个加载已下载模型实测维度，可能需要十几秒，之后有缓存。

```json
{
  "name": "知舟服务台",
  "version": "0.1.0",
  "services": { "docParse": true, "search": true, "chat": false },
  "embeddingModels": [ { "model": "qwen3-embedding:8b", "dims": 1024 } ]
}
```

`dims` 是网关实际输出的维度（服务台里设置了截取维度时为设置值，否则为模型原生维度）。

## 文档解析（MinerU 4）

### 原生 V1 接口（推荐）

网关原样转发 MinerU 4 的 V1 接口，并把响应里 MinerU 自报的本机地址改写为客户端访问网关所用的地址：

| 方法 | 路径 |
|---|---|
| `POST` | `/v1/uploads` |
| `PUT` / `GET` / `POST` | `/v1/uploads/{id}`、`/v1/uploads/{id}/content`、`/v1/uploads/{id}/complete` |
| `POST` / `GET` | `/v1/parse/jobs`、`/v1/parse/jobs/{id}` |
| `GET` | `/v1/files/{id}`、`/v1/files/{id}/content` |
| `GET` | `/v1/tiers` |

流程：创建上传 → 按返回的 `upload_url` 上传字节 → 完成上传得到文件 ID → 创建解析任务（`tier` 取 `flash` / `basic` / `standard` / `advanced`）→ 轮询任务 → 下载结果。长文档不受单次请求超时限制。知舟的「MinerU 本地部署」处理器就是这样调用的。

### 通用 REST（兼容）

`POST /v1/documents/parse`，请求：

```json
{ "file": "<Base64>", "format": "pdf", "filename": "a.pdf" }
```

成功：

```json
{ "success": true, "data": { "markdown": "…", "pageCount": null, "format": "markdown" } }
```

网关在内部同步等待 MinerU 最长 1 小时；档位与文字识别方式取服务台「文档解析 → 设置」。客户端自身若有较短的请求超时，长文档请改用原生 V1 接口。

## 向量与对话（Ollama，OpenAI 兼容）

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/v1/models` | 已下载模型列表 |
| `POST` | `/v1/embeddings` | 向量化 |
| `POST` | `/v1/chat/completions` | 对话（支持流式） |

`/v1/embeddings` 的维度规则：

- 请求带 `dimensions` 时：模型输出维度与之相同则原样返回；更大且模型支持截取（qwen3-embedding、embeddinggemma、nomic-embed-text 等）则截取并重新归一化；否则返回 `400` 并提示正确维度。
- 请求不带 `dimensions` 时：按服务台为该模型设置的输出维度截取，未设置则返回原生维度。

## 网络搜索（SearXNG）

`GET /search?q=<关键词>&format=json`，可附 SearXNG 支持的其他查询参数（如 `time_range`、`engines`）。`format` 总会被设为 `json`。返回 SearXNG 原生结果：

```json
{ "results": [ { "title": "…", "url": "https://…", "content": "…", "engine": "bing" } ] }
```
