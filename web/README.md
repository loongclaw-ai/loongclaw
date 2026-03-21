# LoongClaw Web

LoongClaw 的本地优先 Web Console。

当前已提供：

- `Chat`
- `Dashboard`
- `Onboarding`
- `Debug Console`
- 中英文切换
- 明暗主题切换

## 快速开始

### 开发分离模式

适合前端开发与联调：

1. 在仓库根目录先构建 daemon：`cargo build --bin loongclaw`
2. 在 `web/` 安装依赖：`npm install`
3. 启动前端与本地 API：
   - Windows：`powershell -File scripts/web/start-dev.ps1`
   - macOS / Linux：`bash scripts/web/start-dev.sh`

默认地址：

- Web：`http://127.0.0.1:4173/`
- API：`http://127.0.0.1:4317/`

### 同源静态模式

适合验证更接近产品态的本地体验：

1. 在仓库根目录先构建 daemon：`cargo build --bin loongclaw`
2. 在 `web/` 安装依赖：`npm install`
3. 构建前端：`npm run build`
4. 启动同源服务：
   - Windows：`powershell -File scripts/web/start-same-origin.ps1`
   - macOS / Linux：`bash scripts/web/start-same-origin.sh`

默认地址：

- Web UI + API：`http://127.0.0.1:4318/`

## 相关文档

- `docs/STACK.zh-CN.md`：技术栈、目录与运行约定
- `docs/DESIGN.zh-CN.md`：产品形态、onboarding、调试台与专项 review 结论
- `docs/API.zh-CN.md`：当前前端实际依赖的 Web API
- `INSTALL.md`：更详细的安装与运行步骤
