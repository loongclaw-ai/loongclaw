# LoongClaw Web 目录结构与技术选型

状态：Phase 1 提议  
范围：`web/` 初始模块布局与前端技术栈  
最后更新：2026-03-17

## 1. 目标

本文回答两个问题：

- `web/` 初始应该怎么放？
- 对于本地优先的 Web Console，什么技术栈最合适？

当前视觉参考仓库：

- `E:\GitDesktop\loongclaw-website`

## 2. Phase 1 推荐技术栈

推荐组合：

- React
- TypeScript
- Vite
- React Router
- TanStack Query
- Zod
- CSS Variables + 模块化样式

## 3. 这样选的原因

### React

- 很适合承载 chat + dashboard 双界面
- 组件复用成本低
- 团队上手成本可控

### TypeScript

- API 协议需要强类型约束
- 能降低 chat 和 dashboard 客户端代码漂移

### Vite

- 本地开发启动快
- 静态资源构建简单
- 很适合可选安装的静态资源包模式
- 不依赖服务端渲染

### React Router

- 足够承载 `/chat` 和 `/dashboard`
- 轻量、可预测

### TanStack Query

- 很适合 dashboard 的读取、缓存和重拉
- 可以统一加载态和错误态
- 能把 API 调用逻辑从页面组件里抽出来

### Zod

- 可以做 API payload 的运行时校验
- 在后端协议还在演进时很有价值

### CSS Variables + 模块化样式

- 更适合后续做主题和 token 管理
- 避免一上来就绑死大型 UI kit
- 包体和样式模型都更可控

## 4. Phase 1 明确不优先选择的方案

首批交付不推荐：

- Next.js 或 SSR 前端
- WebSocket-first 架构
- 重型图表/大盘框架
- Electron 或 Tauri 打包
- 先天强绑定的大型组件库

原因：

- phase 1 是本地优先、静态资源友好的场景
- SSR 对本地控制台价值有限
- 当前 API 和信息架构还在快速变化

## 5. 初始目录结构

```text
web/
  public/
  src/
    app/
      chat/
        components/
        pages/
      dashboard/
        components/
        pages/
      shell/
        App.tsx
        routes.tsx
    components/
      layout/
      feedback/
      status/
    lib/
      api/
        client.ts
        types.ts
        validators.ts
      auth/
        token-store.ts
      state/
      utils/
    styles/
      tokens.css
      base.css
  package.json
  tsconfig.json
  vite.config.ts
  DESIGN.md
  DESIGN.zh-CN.md
  API.md
  API.zh-CN.md
  STACK.md
  STACK.zh-CN.md
```

## 6. 页面模型

### `/chat`

职责：

- 会话列表
- 消息历史
- 输入框
- 回复状态
- 连接和 runtime 状态提示

### `/dashboard`

职责：

- runtime 摘要
- provider 摘要
- memory 摘要
- tool 摘要
- 配置摘要
- 警告和健康状态区域

## 7. 前端模块边界

建议保持这些边界清晰：

- `app/`
  - 只做路由级组合
- `components/`
  - 放共享 UI 组件
- `lib/api/`
  - 放所有 HTTP client 逻辑
- `lib/auth/`
  - 只处理 token
- `styles/`
  - 放 token 和全局样式基础

不要把 fetch 逻辑直接散落在页面组件里。

## 8. 构建与发布模型

开发阶段：

- `web/` 独立运行开发服务器
- 通过代理或直接地址连接本地 daemon API

生产阶段：

- 只构建静态资源
- 输出适合可选安装包的产物

这和以下目标一致：

- 同仓开发
- 可选安装
- 后续支持托管前端分发

## 9. 设计系统方向

Phase 1 的界面原则：

- chat 和 dashboard 必须属于同一套视觉系统
- 避免“通用后台模板”味道过重
- chat 和 dashboard 看起来要像同一个产品
- 优先用 theme token，而不是在页面里写死颜色决策

如果有现成的前端参考库，`web/` 应优先继承：

- spacing 体系
- typography scale
- color token
- 组件密度
- 交互模式

当前分支的主要视觉参考仓库为：

- `E:\GitDesktop\loongclaw-website`

首版 Web 实现应有意识地继承该仓库的这些特征：

- 基于 CSS variables 的 token 驱动主题体系
- 深色默认主题，整体语言偏终端感，但不过度赛博装饰
- 浅色主题采用偏暖色而不是纯白后台模板风格
- 字体体系以等宽正文、中文回退和独立展示字体组合为核心
- 使用细腻渐变、边框主导的面板和低饱和度界面，而不是通用后台模板视觉
- chat 和 dashboard 共用同一套视觉系统，而不是各做各的

## 10. 未来扩展位

这个结构需要为后续扩展留出空间：

- `/settings`
- `/runtime`
- `/providers`
- 托管模式连接引导
- 更丰富的 trace / diagnostics 界面

这也是为什么整个模块应命名为 `web`，而不是 `webchat`。
