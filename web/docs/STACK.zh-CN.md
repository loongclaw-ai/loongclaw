# LoongClaw Web 技术栈与目录结构

状态：已进入可用 MVP，持续迭代中  
最后更新：2026-03-20

## 1. 目标

`web/` 目录承载 LoongClaw 的本地优先 Web Console。

当前目标不是独立云端产品，而是基于现有 runtime 提供：
- Web Chat
- Web Dashboard
- 首次进入 / onboarding

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

### `web/src/features/dashboard/`

承载：
- runtime 摘要
- tools 摘要
- config 摘要
- connectivity 诊断
- provider 最小写入入口

### `web/src/features/onboarding/`

承载：
- onboarding 状态读取
- provider 最小写入
- validate 放行

### `web/src/contexts/`

当前主要管理：
- Web 会话连接状态
- token 状态
- onboarding 状态与放行

### `web/src/styles/`

当前已开始温和拆分：
- `variables.css`：设计 token
- `themes.css`：主题映射
- `dashboard.css`：dashboard 与 onboarding 表单样式
- `index.css`：全局、chat、共享布局样式

## 5. 运行约定

开发模式：
- 前端：`vite dev`
- 后端：`loongclaw web serve --bind 127.0.0.1:4317`

默认访问：
- 前端：`http://127.0.0.1:4173/`
- 后端：`http://127.0.0.1:4317/`

推荐脚本：
- `scripts/web/start-dev.ps1`
- `scripts/web/stop-dev.ps1`

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
- dashboard `Provider Settings` 写入

当前支持字段：
- provider kind
- model
- base_url / endpoint
- api key

### O3：验证与放行

已落地：
- `POST /api/onboard/validate`
- 先验证，再放行进入 Web

### O4：token / pairing 收口

已部分落地：
- token 输入已进入 onboarding 面板
- 顶部零散 token banner 已移除
- Web 会优先尝试一次轻自动配对
- 自动配对成功后，通过本地受信 cookie 建立当前浏览器会话的配对状态
- 自动配对失败时，再回退到手动输入 token

未完成部分：
- 安装态 / 同源态下更顺滑的自动配对
- 更长期的无感鉴权验证

## 8. 可选安装现状

当前还没有完整的安装形态。

现阶段只有：
- `loongclaw web serve`
- 开发态 / 本地 API 驱动的 Web Console

尚未落地：
- `web install`
- `web remove`
- `web status`
- 静态资源安装与托管闭环

长期方向上，如果同时考虑“可选安装”和“官方 host”，Web 产品态更适合向同源设计收敛；开发态则继续允许 `vite dev + 本地 API` 的分离结构。

## 9. Debug / Runtime Console 方向

后续 Web 可能会补一个面向观测与调试的控制台，但当前更推荐：

- 先做 Runtime / Debug Console
- 展示事件流、tool 调用、provider 诊断、session 元信息
- 暂不直接做完整浏览器终端

## 10. 接下来最适合的工作

推荐顺序：
1. 继续补齐 O4，把 token 流程做完整
2. 做 O2.5，把轻配置项补进 Web
3. 继续拆分大文件，降低维护成本
4. 之后再进入可选安装能力实现
