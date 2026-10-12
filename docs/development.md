# 开发指南

## 环境

- Node.js 20+
- Rust（stable，MSVC 工具链）：`rustup default stable-msvc`
- Visual Studio Build Tools，勾选「使用 C++ 的桌面开发」
- WebView2（Windows 11 自带）
- Docker Desktop（调试服务部署时需要）

## 常用命令

```powershell
npm install
npm run dev            # 只在浏览器里预览界面，使用演示数据（http://localhost:1420）
npm run tauri dev      # 桌面应用，前端热更新
npx tsc -b             # 前端类型检查
cd src-tauri; cargo check
npm run tauri build    # 生成 NSIS 安装包：src-tauri/target/release/bundle/nsis/
npx tauri build --no-bundle   # 只生成 exe（便携运行，配置写在 exe 同目录）
```

## 设计令牌

`src/styles/tokens.css` 与 `public/brand_*.svg` 来自知舟的品牌源（书院色板）。在知舟主仓库内运行 `node scripts/gen-tokens.mjs` 会重新生成；独立仓库中脚本会跳过并沿用已提交的文件。页面只经 CSS 变量取色，不写颜色字面量。

## 新增一项服务

1. 在 `src/catalog/services.json` 添加条目（镜像、端口、卷、环境变量、GPU 选项、资源需求）。需要本机构建的镜像放在 `src-tauri/docker/<名称>.Dockerfile`，条目写 `"build": "<名称>"`。
2. 健康检查写在 `src-tauri/src/health.rs`，「试一试」写在 `src-tauri/src/trial.rs` 与 `src/pages/ServiceTabs.tsx`。
3. 需要对外提供协议的，在 `src-tauri/src/gateway.rs` 加路由（默认挂在鉴权中间件之后）。
4. 使用说明写在 `src/catalog/guides.tsx`，「连接知舟」页的手填值写在 `src/pages/Connect.tsx`。

## 发布

1. 同步 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 里的版本号，更新 `CHANGELOG.md`。
2. `npm run tauri build`。
3. 打标签并上传安装包：

```powershell
git tag v0.1.0; git push origin v0.1.0
# GitHub 会丢掉资源文件名里的中文，上传前改成英文名
Copy-Item "src-tauri/target/release/bundle/nsis/知舟服务台_0.1.0_x64-setup.exe" KnowArkHub_0.1.0_x64-setup.exe
gh release create v0.1.0 KnowArkHub_0.1.0_x64-setup.exe --notes-file CHANGELOG.md
```

安装包未做代码签名，首次运行 Windows SmartScreen 可能提示「未知发布者」，选择「仍要运行」即可。
