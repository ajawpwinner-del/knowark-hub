# 知舟服务台（KnowArk Hub）

Windows 桌面工具，用来一键部署、管理知舟可以对接的自建服务，并生成知舟里要填写的连接信息。

| 服务 | 引擎 | 知舟里的用途 |
|---|---|---|
| 本地模型 | Ollama | 向量模型（bge-m3 等）和可选的本地对话模型 |
| 网络搜索 | SearXNG | 网络搜索 → SearXNG |
| 文档解析 | MinerU（本机构建，pipeline 后端） | 文档处理 →「文档解析」 |

## 工作方式

- 每个服务跑在各自的 Docker 容器里，容器名 `knowark-<id>`。端口只绑定 `127.0.0.1`，局域网访问不到。
- 服务台内置一个**兼容网关**，默认端口 8765，是局域网里唯一开放的入口。它把各服务统一成知舟现有的协议，所以知舟只需填一个地址和一个 Key：
  - `POST /v1/documents/parse`：知舟的「通用 REST 桥接」。PDF、Office 交给 MinerU（扫描版 PDF 由其内置识别模型处理）。
  - `GET /v1/hub/info`：能力清单（运行中的服务、可向量化模型及实测维度），供知舟「连接服务台」扫码配对。
  - `POST /v1/embeddings`、`GET /v1/models`、`POST /v1/chat/completions`：OpenAI 兼容协议，转给 Ollama。网关会校验返回向量的维数是否等于请求里的 `dimensions`：
    - 支持截取维度的模型，会截取并重新归一化；
    - 其它模型直接报错，并提示正确的维数。
  - `GET /search?format=json`：SearXNG 协议，用 Basic 认证，密码就是 Key。
- Key 保存在 Windows 凭据管理器里，不写进设置文件，也不进日志。
- 健康检查与「试一试」都经过网关，按知舟实际的调用方式真实请求一次。
- 容器联网统一走 Docker Desktop 内置转发代理 `http.docker.internal:3128`（转给 Docker Desktop 中配置的代理，未配置时直连），Ollama 下载模型、SearXNG 访问境外引擎才能在国内网络下工作。
- Docker Hub 镜像拉取失败时自动依次改用国内镜像源；配置与数据默认放在程序所在目录（便携），可在设置中更改。
- 关闭窗口缩到系统托盘，网关继续为手机服务；托盘菜单「退出」才真正停止。

## 开发

需要 Node 20+、Rust（MSVC 工具链）、VS Build Tools「使用 C++ 的桌面开发」和 WebView2。

```powershell
npm install
npm run dev            # 只在浏览器里预览界面，使用演示数据
npm run tauri dev      # 桌面应用
npm run tauri build    # 生成 NSIS 安装包
```

设计令牌由 `scripts/gen-tokens.mjs` 从 `docs/brand/tools/brand-academy.mjs` 和 entry 的 `color.json` 生成，`dev` 和 `build` 会自动运行它。品牌改色后不用改本工具的代码。服务目录只有一份，在 `src/catalog/services.json`，前端和 Rust 端共用。
