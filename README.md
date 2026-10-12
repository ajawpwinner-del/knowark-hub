<p align="center"><img src="public/brand_mark.svg" width="96" alt="知舟服务台"></p>

<h1 align="center">知舟服务台 · KnowArk Hub</h1>

<p align="center">把文档解析、网络搜索、向量模型装进你自己的电脑，让知舟的知识库不出家门。</p>

---

知舟（KnowArk）是一款本地优先的 HarmonyOS 知识库应用：笔记存在手机里，检索与问答可以只用本机能力。但有几件事，手机自己做起来很吃力：

- 把 PDF、扫描件转成带标题、表格、公式的 Markdown；
- 用效果更好的向量模型给整个知识库建语义索引；
- 联网查资料，又不想把搜索词交给某家商业搜索 API。

**知舟服务台**是一个 Windows 小工具：点几下就在你自己的电脑上部署好这三项服务，再用知舟扫一下二维码，手机就能通过家里的局域网使用它们。**笔记、文档和搜索词只在你的手机和你的电脑之间往来**，不经过任何云服务（网络搜索会由你电脑上的 SearXNG 直接访问各搜索引擎，不经第三方中转）。

## 能做什么

| 服务 | 引擎 | 在知舟里的用途 |
|---|---|---|
| 文档解析 | [MinerU](https://github.com/opendatalab/MinerU) 4 | 导入 PDF / Office 时转成 Markdown，扫描件也能识别，标题、表格、公式都保留 |
| 网络搜索 | [SearXNG](https://github.com/searxng/searxng) | 问答与写作联网查资料；可自选必应、百度、搜狗、Google、arXiv 等引擎，无需 API Key |
| 本地模型 | [Ollama](https://ollama.com) | 知识库语义检索用的向量模型（qwen3-embedding、bge-m3 等），也可下载对话模型 |

另外：

- **一键部署**：每项服务跑在独立的 Docker 容器里，安装、启停、看日志、卸载都在界面里点；国内网络自动改用镜像源。
- **扫码配对**：知舟「设置 → 连接服务台」扫码，一次配好文档解析、网络搜索与（可选）向量模型。
- **一个地址、一个 Key**：内置兼容网关，局域网只开放一个端口；Key 保存在 Windows 凭据管理器。
- **试一试**：不打开知舟，就能在服务台里搜一次、解析一个 PDF，确认服务可用。
- **向量维度管理**：支持截取维度的模型可把 4096 维降到 1024 维，手机上省空间、检索更快。
- **托盘常驻**：关闭窗口缩到托盘，手机随时连得上。
- 界面沿用知舟「书院」设计语言，支持浅色 / 深色。

## 快速开始

> 详细步骤见 [使用指南](docs/user-guide.md)。

1. **准备环境**：Windows 10 2004 及以上（推荐 Windows 11），16 GB 内存更从容，磁盘留出 20—30 GB。安装 [Docker Desktop](https://www.docker.com/products/docker-desktop/)（会顺带启用 WSL 2），打开一次，等左下角显示 *Engine running*。
2. **安装服务台**：在 [Releases](../../releases) 下载 `知舟服务台_x.y.z_x64-setup.exe` 安装。
3. **部署服务**：首页「环境体检」全绿后，到「服务目录」点「安装」。
4. **连接知舟**：手机和电脑连同一个 Wi‑Fi。服务台「连接知舟」页点「允许访问」放行防火墙，然后在知舟「设置 → 连接服务台」点「扫码配对」。

## 隐私与本地化

- 各服务容器只监听 `127.0.0.1`，局域网里只有服务台网关（默认 `8765` 端口）可达，且每个请求都要带 Key。
- Key 只存在 Windows 凭据管理器；服务台不记录文档内容、搜索词或 Key。
- 镜像与模型存在 Docker Desktop 的磁盘镜像里，可在 Docker Desktop「Settings → Resources → Advanced」改到任意磁盘；服务台自身的配置默认放在程序所在目录。
- 文档解析与向量化完全在本机完成；网络搜索由你电脑上的 SearXNG 直接访问所选搜索引擎。
- 知舟一侧的数据去向见知舟隐私说明「知舟服务台配对」一节。

## 文档

- [使用指南](docs/user-guide.md)：从零开始的安装、部署、配对与常见问题
- [网关接口](docs/gateway-api.md)：服务台对外提供的 HTTP 接口（知舟与其他客户端都可对接）
- [架构说明](docs/architecture.md)：组成、数据流与设计取舍
- [开发指南](docs/development.md)：本地构建、打包与发布
- [更新记录](CHANGELOG.md)

## 致谢

服务台只是把这些优秀的开源项目装进同一个界面：[MinerU](https://github.com/opendatalab/MinerU)、[SearXNG](https://github.com/searxng/searxng)、[Ollama](https://github.com/ollama/ollama)、[Tauri](https://tauri.app)。各服务的模型与镜像遵循其各自的许可证。
