# LoongClaw Web 技术栈与目录结构

状态：已进入可用 MVP，持续迭代中  
最后更新：2026-03-21

## 1. 目标

`web/` 目录承载 LoongClaw 的本地优先 Web Console。

当前目标不是独立云端产品，而是基于现有 runtime 提供：

- Web Chat
- Web Dashboard
- 首次进入 / onboarding
- 本地诊断与调试入口

## 2. 当前技术栈

前端主栈：

- React
- TypeScript
- Vite
- React Router
- i18next / react-i18next
- CSS Variables + 自定义主题样式

后端承接：

- `crates/daemon/src/web_cli.rs`
- `crates/daemon/src/web_cli/onboarding.rs`
- Axum 本地 API

## 3. 当前目录

```text
web/
  docs/
    API.zh-CN.md
    DESIGN.zh-CN.md
    STACK.zh-CN.md
  public/
  src/
    app/
    assets/
      locales/
        en/
        zh-CN/
    components/
      layout/
      status/
      surfaces/
    contexts/
    features/
      chat/
      dashboard/
      onboarding/
    hooks/
    lib/
      api/
      auth/
      config/
      utils/
    styles/
      variables.css
      themes.css
      dashboard.css
      index.css
    main.tsx
```

## 4. 目录职责

### `web/src/features/chat/`

承载：

- session 列表
- history
- turn 创建
- turn 流式读取
- 生成中状态
- 轻量消息渲染

### `web/src/features/dashboard/`

承载：

- runtime 摘要
- tools 摘要
- config 摘要
- connectivity 诊断
- provider 最小写入入口
- Debug Console 入口

### `web/src/features/onboarding/`

承载：

- onboarding 状态读取
- provider 最小写入
- preferences 轻配置写入
- validate 放行
- token pairing 流程

### `web/src/contexts/`

当前主要管理：

- Web 会话连接状态
- token / pairing 状态
- onboarding 状态与放行

### `web/src/styles/`

当前已开始温和拆分：

- `variables.css`：设计 token
- `themes.css`：主题映射
- `dashboard.css`：Dashboard 与 Debug Console
- `index.css`：全局、Chat 与共享布局样式

## 5. 运行约定

开发模式：

- 前端：`vite dev`
- 后端：`loongclaw web serve --bind 127.0.0.1:4317`

默认访问：

- 前端：`http://127.0.0.1:4173/`
- 后端：`http://127.0.0.1:4317/`

推荐脚本：

- Windows
  - `scripts/web/start-dev.ps1`
  - `scripts/web/stop-dev.ps1`
- macOS / Linux
  - `scripts/web/start-dev.sh`
  - `scripts/web/stop-dev.sh`

## 6. 日志位置

运行日志统一落在用户目录，不再写回仓库：

- `%USERPROFILE%\\.loongclaw\\logs\\web-dev.log`
- `%USERPROFILE%\\.loongclaw\\logs\\web-dev.err.log`
- `%USERPROFILE%\\.loongclaw\\logs\\web-api.log`
- `%USERPROFILE%\\.loongclaw\\logs\\web-api.err.log`

## 7. 当前已落地链路

### O1：首次进入状态检测

已落地：

- `GET /api/onboard/status`
- 首屏状态面板
- ready 状态确认进入

### O2：最小 provider 可写配置

已落地：

- `POST /api/onboard/provider`
- onboarding 表单
- Dashboard `Provider Settings`

当前支持字段：

- provider kind
- model
- base_url / endpoint
- api key

### O3：验证与放行

已落地：

- `POST /api/onboard/validate`
- `POST /api/onboard/provider/apply`
- 应用后在当前页验证，而不是强制回 onboarding

### O4：token / pairing 收口

已部分落地：

- token 输入已进入 onboarding
- 顶部散落 token banner 已移除
- 轻自动配对 + 手动兜底

### O2.5：轻配置项

已部分落地：

- `personality`
- `memory_profile`
- `prompt addendum`

当前优先落在 onboarding 首屏的“可选个性化设置”。

### Debug Console

已落地第一版：

- Dashboard 内嵌切换
- 只读
- 终端风格
- 以“操作块”形式展示最近事件

## 8. 当前仍未完成

- 完整的 Dashboard 轻配置项写入闭环
- 更像真实 CLI 的连续输出流
- 更完整的 tool trace / event timeline
- 安装态 / 同源态能力
- `web install / remove / status`
- 更强的 provider / tool doctor

## 9. 近期值得关注的新事项

这段时间新增且会影响后续开发的事项：

- Debug Console 已从“卡片拼接”转向“操作块分段”展示
- Chat 历史已改成按**可见消息**计数
- Dashboard 工具区已对齐更多 runtime 能力：
  - `web_search`
  - `browser_companion`
  - `file_tools`
- macOS 启停脚本已补齐
- provider apply 已收成当前页验证流程

## 10. 接下来最适合继续做什么

推荐顺序：

1. 继续打磨 Debug Console 的输出质量和分段可读性
2. 把更多轻配置项稳妥接到 Dashboard
3. 推动后端修复 `tool.search` 的中文 / 泛化召回
4. 继续温和拆分大文件，降低长期维护成本
5. 再进入可选安装与同源产品态实现
