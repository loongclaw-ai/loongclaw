# LoongClaw Web API（当前状态）

本文档只记录当前 Web Console 已经落地、并且前端正在实际依赖的接口。

## 1. 基础接口

### `GET /healthz`

用于确认本地 Web API 是否在线。

### `GET /api/meta`

返回 Web 入口所需的基础元信息，例如：

- app version
- api version
- 当前 `web_install_mode`
- 本地鉴权方式
- token 文件路径 / 环境变量名

## 2. Onboarding 接口

### `GET /api/onboard/status`

用于首次进入状态聚合。

返回重点包括：

- `runtimeOnline`
- `tokenRequired`
- `tokenPaired`
- `configExists`
- `configLoadable`
- `providerConfigured`
- `providerReachable`
- `personality`
- `memoryProfile`
- `promptAddendum`
- `blockingStage`
- `nextAction`

这条接口是匿名可读的，用来回答：

> 为什么当前还不能进入 Web。

### `POST /api/onboard/provider`

用于最小 provider 配置写入。

当前只接收四类字段：

- `kind`
- `model`
- `baseUrlOrEndpoint`
- `apiKey`

这条接口已经被：

- onboarding 首屏
- Dashboard `Provider Settings`

共同复用。

### `POST /api/onboard/preferences`

用于 O2.5 的轻配置项写入。

当前只接收：

- `personality`
- `memoryProfile`
- `promptAddendum`

这条接口当前主要被 onboarding 首屏的“可选个性化设置”使用。

### `POST /api/onboard/validate`

用于 provider 基础验证与放行。

当前验证关注：

- endpoint 是否可达
- 凭证是否通过基础探测

返回重点包括：

- `passed`
- `endpointStatus`
- `credentialStatus`
- `status`（最新 onboarding status 快照）

### `POST /api/onboard/pairing/auto`

用于 O4 的轻自动配对。

当前行为：

- 仅允许本地受信 loopback origin 尝试
- 成功后不把 token 明文返回给前端
- 后端通过 HttpOnly cookie 为当前浏览器建立配对状态
- 失败时前端回退到手动 token 输入

### `POST /api/onboard/pairing/clear`

用于清除当前浏览器的自动配对 cookie。

配合手动 `clear token` 一起使用，避免浏览器刚清掉本地 token 又立刻被自动配对恢复。

## 3. Dashboard 接口

### `GET /api/dashboard/summary`

提供顶部摘要卡数据。

### `GET /api/dashboard/providers`

提供 provider 列表、当前激活项、模型、endpoint 与 key 配置状态。

### `GET /api/dashboard/runtime`

提供 runtime 运行态信息，例如：

- config path
- memory mode
- active provider / model

### `GET /api/dashboard/config`

提供 UI 关注的配置快照，例如：

- endpoint
- API key 是否已配置
- personality
- prompt mode
- prompt addendum 是否已配置
- memory profile
- sqlite path
- file root
- sliding window

### `GET /api/dashboard/connectivity`

提供 provider route / connectivity 诊断，例如：

- endpoint
- host
- DNS 结果
- probe 状态
- 是否命中 fake-ip
- 推荐修复方向

### `GET /api/dashboard/tools`

提供工具启用状态与策略摘要。

## 4. Chat 接口

### `GET /api/chat/sessions`

读取会话列表。

### `POST /api/chat/sessions`

创建会话。

### `DELETE /api/chat/sessions/{id}`

删除会话。

### `GET /api/chat/sessions/{id}/history`

读取会话历史。

### `POST /api/chat/sessions/{id}/turn`

创建 turn。

当前返回：

- `sessionId`
- `turnId`
- `status = accepted`

### `GET /api/chat/sessions/{id}/turns/{turn_id}/stream`

返回 NDJSON 流式事件。

当前事件集包括：

- `turn.started`
- `message.delta`
- `tool.started`
- `tool.finished`
- `turn.completed`
- `turn.failed`

## 5. 当前边界

当前 Web API 仍然有这些边界：

- token / pairing 已有轻自动化，但还不是安装态级别的无感配对
- provider 验证仍是最小探测，不是完整 doctor
- Dashboard 写入目前仍以最小 provider 配置为主
- O2.5 的轻配置项当前主要落在 onboarding 首屏，尚未形成完整 Dashboard 写入闭环
