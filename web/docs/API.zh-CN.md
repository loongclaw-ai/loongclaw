# LoongClaw Web API（当前状态）

本文档只记录当前 Web Console 已经落地、且前端正在实际依赖的接口。

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

重点字段包括：

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

> 为什么当前还不能进入 Web？

### `POST /api/onboard/provider`

用于最小 provider 配置写入。

当前接收字段：

- `kind`
- `model`
- `baseUrlOrEndpoint`
- `apiKey`

这条接口已经被：

- onboarding 首屏
- Dashboard `Provider Settings`

共同复用。

### `POST /api/onboard/provider/apply`

用于“应用并验证” provider 配置。

行为是：

- 先拿候选 provider 配置做最小验证
- 只有验证通过才真正落盘
- 返回验证结果与最新 onboarding 状态

这条接口当前主要被 Dashboard `Apply` 使用。

### `POST /api/onboard/preferences`

用于 O2.5 的轻配置项写入。

当前接收：

- `personality`
- `memoryProfile`
- `promptAddendum`

当前主要被 onboarding 首屏的“可选个性化设置”使用。

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
- 不把 token 明文返回给前端
- 通过 HttpOnly cookie 建立当前浏览器的配对状态
- 自动失败时由前端回退到手动 token 输入

### `POST /api/onboard/pairing/clear`

用于清除当前浏览器的自动配对 cookie。

配合手动清理 token 状态使用，避免浏览器刚清掉本地 token 又被自动配对恢复。

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
- fake-ip 命中判断
- 推荐修复方向

### `GET /api/dashboard/tools`

提供工具启用状态与策略摘要。

当前已覆盖的重点项包括：

- `shell_policy`
- `sessions`
- `messages`
- `delegate`
- `browser`
- `browser_companion`
- `web_fetch`
- `web_search`
- `file_tools`
- `external_skills`

### `GET /api/dashboard/debug-console`

提供 Dashboard 内只读 Debug Console 的数据。

当前返回结构包含：

- `generatedAt`
- `command`
- `blocks`

其中 `blocks` 为按操作分段的控制台块，例如：

- runtime snapshot
- 最近一次 turn
- 最近一次 provider apply / validate
- 最近一次 preferences apply
- 最近一次 token pairing
- process output

## 4. Chat 接口

### `GET /api/chat/sessions`

读取会话列表。

### `POST /api/chat/sessions`

创建会话。

### `DELETE /api/chat/sessions/{id}`

删除会话。

### `GET /api/chat/sessions/{id}/history`

读取会话历史。

当前行为已调整为：

- 优先按**可见消息**计数
- 不让内部 assistant 记录占掉前端消息泡额度

### `POST /api/chat/sessions/{id}/turn`

创建 turn。

当前返回：

- `sessionId`
- `turnId`
- `status = accepted`

### `GET /api/chat/sessions/{id}/turns/{turn_id}/stream`

返回 NDJSON 流式事件。

当前事件集合包括：

- `turn.started`
- `message.delta`
- `tool.started`
- `tool.finished`
- `turn.completed`
- `turn.failed`

## 5. 当前边界

当前 Web API 仍有这些边界：

- token / pairing 已有轻自动化，但还不是安装态级别的无感配对
- provider 验证仍是最小探测，不是完整 doctor
- Dashboard 写入目前仍以最小 provider 配置为主
- O2.5 的轻配置项当前主要落在 onboarding 首屏
- Debug Console 还是只读观测面，不是完整 CLI 镜像
