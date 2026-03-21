# LoongClaw Web Installation Guide

## 1. 前置条件

在运行 Web Console 之前，请先准备：

- Rust 工具链
- Node.js + npm
- 已在仓库根目录构建 `loongclaw` daemon：`cargo build --bin loongclaw`

## 2. 安装前端依赖

进入 `web/` 后执行：

```bash
npm install
```

## 3. 开发分离模式

适合前端开发、热更新与联调。

### 启动

- Windows：`powershell -File scripts/web/start-dev.ps1`
- macOS / Linux：`bash scripts/web/start-dev.sh`

### 默认地址

- Web：`http://127.0.0.1:4173/`
- API：`http://127.0.0.1:4317/`

### 停止

- Windows：`powershell -File scripts/web/stop-dev.ps1`
- macOS / Linux：`bash scripts/web/stop-dev.sh`

## 4. 同源静态模式

适合验证更接近产品态的本地入口。该模式会由 daemon 同时托管静态资源和 API，并自动走同源 session。

### 构建前端

```bash
npm run build
```

### 启动

- Windows：`powershell -File scripts/web/start-same-origin.ps1`
- macOS / Linux：`bash scripts/web/start-same-origin.sh`

默认地址：

- Web UI + API：`http://127.0.0.1:4318/`

### 可选：启动时顺带构建

- Windows：`powershell -File scripts/web/start-same-origin.ps1 -Build`
- macOS / Linux：`BUILD=1 bash scripts/web/start-same-origin.sh`

### 停止

- Windows：`powershell -File scripts/web/stop-same-origin.ps1`
- macOS / Linux：`bash scripts/web/stop-same-origin.sh`

## 5. 环境变量

- `VITE_API_BASE_URL`：开发态显式指定 API 基地址；未设置时，前端会对 `4173 -> 4317` 做默认映射，并在同源模式下回退到当前 `origin`

## 6. 日志位置

日志默认写入用户目录：

- Windows：`%USERPROFILE%\.loongclaw\logs\`
- macOS / Linux：`~/.loongclaw/logs/`

常见日志文件：

- `web-dev.log`
- `web-dev.err.log`
- `web-api.log`
- `web-api.err.log`
- `web-same-origin.log`
- `web-same-origin.err.log`
