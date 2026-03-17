# LoongClaw Web API 草案

状态：Phase 1 草案  
范围：本地优先的 Web Chat + Web Dashboard  
最后更新：2026-03-17

## 1. 目的

本文定义 Web 前端消费的本地 HTTP API 初稿。它明确只服务于 phase 1，
主要优化目标是：

- 单一本地 runtime
- 单一本地操作者
- Web 资源可选安装
- 不带托管优先假设

## 2. 协议规则

- Base URL：`http://127.0.0.1:<port>`
- 默认绑定：`127.0.0.1:4317`
- 所有 `/api/*` 路由返回 JSON
- `GET /healthz` 不鉴权
- 其余 `/api/*` 路由都要求 `Authorization: Bearer <token>`
- 响应体包含 `schema_version`，用于未来兼容

Phase 1 不要求浏览器登录流程。本地 token 由 CLI / install 流程提供，
前端只负责使用。

## 3. 通用响应结构

### 3.1 成功响应

```json
{
  "ok": true,
  "schema_version": "2026-03-17.phase1",
  "data": {}
}
```

### 3.2 错误响应

```json
{
  "ok": false,
  "schema_version": "2026-03-17.phase1",
  "error": {
    "code": "unauthorized",
    "message": "missing or invalid token",
    "retryable": false
  }
}
```

建议错误码：

- `unauthorized`
- `forbidden_origin`
- `not_found`
- `invalid_request`
- `turn_failed`
- `runtime_unavailable`
- `web_assets_not_installed`

## 4. 接口列表

### 4.1 `GET /healthz`

用途：

- 进程存活检查

响应：

```json
{
  "ok": true,
  "schema_version": "2026-03-17.phase1",
  "data": {
    "status": "ok"
  }
}
```

### 4.2 `GET /api/meta`

用途：

- 前端启动时拉取基础信息
- 获取 runtime 能力
- 获取当前安装模式

响应：

```json
{
  "ok": true,
  "schema_version": "2026-03-17.phase1",
  "data": {
    "product": "loongclaw",
    "version": "0.1.2",
    "mode": "local",
    "web": {
      "install_mode": "local_assets",
      "static_assets_present": true
    },
    "surfaces": {
      "chat": true,
      "dashboard": true
    },
    "auth": {
      "mode": "local_token"
    }
  }
}
```

### 4.3 `GET /api/chat/sessions`

用途：

- 列出最近可见的会话

响应：

```json
{
  "ok": true,
  "schema_version": "2026-03-17.phase1",
  "data": {
    "sessions": [
      {
        "id": "web:browser-123",
        "title": "Default",
        "last_message_at": "2026-03-17T14:55:00Z",
        "message_count": 8
      }
    ]
  }
}
```

### 4.4 `POST /api/chat/sessions`

用途：

- 创建或恢复 Web 会话

请求：

```json
{
  "client_session_id": "browser-123",
  "title": "Default"
}
```

响应：

```json
{
  "ok": true,
  "schema_version": "2026-03-17.phase1",
  "data": {
    "session": {
      "id": "web:browser-123",
      "title": "Default"
    }
  }
}
```

### 4.5 `GET /api/chat/sessions/:id/history?limit=50`

用途：

- 加载最近历史，用于 chat 页面初始化

响应：

```json
{
  "ok": true,
  "schema_version": "2026-03-17.phase1",
  "data": {
    "session_id": "web:browser-123",
    "messages": [
      {
        "id": "turn-user-1",
        "role": "user",
        "content": "hello"
      },
      {
        "id": "turn-assistant-1",
        "role": "assistant",
        "content": "hi"
      }
    ]
  }
}
```

### 4.6 `POST /api/chat/sessions/:id/turn`

用途：

- 提交一轮用户输入

请求：

```json
{
  "message": "Summarize this repository.",
  "client_turn_id": "ui-turn-001",
  "stream": false
}
```

响应：

```json
{
  "ok": true,
  "schema_version": "2026-03-17.phase1",
  "data": {
    "session_id": "web:browser-123",
    "turn_id": "turn-0001",
    "message": {
      "id": "turn-assistant-0001",
      "role": "assistant",
      "content": "This repository is ..."
    },
    "runtime": {
      "provider": "deepseek-chat",
      "status": "completed"
    }
  }
}
```

Phase 1 默认只返回最终文本，不强制首版就上流式输出。

### 4.7 `GET /api/dashboard/summary`

用途：

- dashboard 首页概览

响应：

```json
{
  "ok": true,
  "schema_version": "2026-03-17.phase1",
  "data": {
    "runtime": {
      "status": "ready"
    },
    "provider": {
      "active": "deepseek-chat"
    },
    "memory": {
      "enabled": true,
      "backend": "sqlite"
    },
    "tools": {
      "available_count": 6
    },
    "web": {
      "install_mode": "local_assets",
      "static_assets_present": true
    }
  }
}
```

### 4.8 `GET /api/dashboard/providers`

用途：

- provider 列表与 active 状态

### 4.9 `GET /api/dashboard/tools`

用途：

- tool 可用性摘要，不做完整执行 trace

### 4.10 `GET /api/dashboard/runtime`

用途：

- runtime readiness 与诊断摘要

### 4.11 `GET /api/dashboard/config`

用途：

- 返回适合 UI 展示的安全配置摘要
- 不返回密钥或敏感字段

## 5. 流式输出后续方案

如果在 phase 1.5 增加流式输出，建议接口为：

- `GET /api/chat/sessions/:id/stream/:turn_id`

优先使用：

- 先上 SSE
- 只有在后续确实需要更强交互控制时，再考虑 WebSocket

## 6. 安全说明

Phase 1 假设：

- 仅绑定回环地址
- 仅用 bearer token
- 单操作者模型
- 不使用浏览器 cookie 鉴权
- 不支持公网直出

## 7. 兼容规则

- 新字段必须是增量添加
- 现有字段在当前分支线内必须保持兼容
- 破坏性路由调整应发生在 Web GA 之前，而不是之后
