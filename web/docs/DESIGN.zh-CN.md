# LoongClaw Web 设计进度

## 1. 当前定位

LoongClaw Web 现在已经不是一个纯壳子，而是一个可实际使用的本地 Web Console：

- 可以进入 `chat / dashboard`
- 已接入真实 runtime
- 已接入 onboarding 首屏检查
- 已支持最小 provider 配置写入
- 已支持最小验证与放行
- 已支持 token 轻自动配对
- 已开始补齐 CLI 轻配置项
- 已有一个可用的 Dashboard Debug Console 原型

但它仍然处于 **开发态优先** 阶段，还不是完整的“开箱即用 Web 产品入口”。

## 2. 架构方向

### 当前开发态

当前仍然采用：

- 前端 dev server
- 本地 API
- 本地受保护 runtime

也就是“分离式前端 + 本地 API”的开发结构。这样做的原因是：

- 前端迭代快
- 联调成本低
- Vite 热更新体验好

### 长期产品方向

如果后续要做：

- 可选安装
- 官方 host
- 更顺滑的首次进入体验

那么 Web 更适合逐步收敛到 **同源设计**：

- 开发态：继续允许分离，保持开发效率
- 产品态：优先同源托管，减少 token / pairing 的显式心智负担

一句话：

> 现在分离，是为了开发快；以后同源，是为了产品顺。

## 3. Onboarding 进度

### O1：首次进入状态检测

已完成。

当前已有：

- `GET /api/onboard/status`
- runtime / token / config / provider 状态聚合
- 首屏 onboarding 状态面板
- ready 状态确认进入

它已经能回答一个核心问题：

> 为什么当前还不能进入 Web？

### O2：最小可写配置

已完成第一版。

当前 Web 可写的最小配置项为：

- provider kind
- model
- base_url / endpoint
- api key

这条写入链已经被：

- onboarding 首屏
- Dashboard `Provider Settings`

共同复用。

### O3：验证与放行

已完成第一版。

当前已有：

- `POST /api/onboard/validate`
- `POST /api/onboard/provider/apply`
- provider 配置“应用并验证”原子路径

当前验证关注的最小问题是：

- endpoint 是否可达
- 凭证是否通过基础探测

Dashboard 里现在也不会再因为 `Apply` 把用户踢回 onboarding，而是：

- 留在当前页
- 弹出“正在验证 / 验证成功 / 验证失败”的短时反馈
- 验证失败时回到修改前状态

### O4：token / pairing 收口

当前属于 **部分完成，且已有轻自动化**。

已完成：

- token 配对已收进 onboarding 面板
- 不再依赖单独的顶部 token banner
- Web 会优先尝试一次轻量自动配对
- 自动配对成功后，通过本地受信 cookie 建立当前浏览器的配对状态
- 自动配对失败时，会回退到手动输入 token
- 手动 token 输入框不会因为自动配对尝试而消失

当前边界：

- 还不是安装态级别的无感配对
- 仍然会在必要时暴露 token 文件路径
- 开发态仍然需要处理本地 API token 的概念

### O2.5：轻配置项补齐

已完成第一版落地。

当前已支持：

- onboarding 首屏新增“可选个性化设置”折叠区
- 可在首次进入时按需设置：
  - `personality`
  - `memory_profile`
  - `prompt addendum`

当前边界：

- `system_prompt` 仍不可直接修改
- 这些轻配置项主要先落在 onboarding 首屏
- Dashboard 当前仍以只读展示为主

## 4. Dashboard / Chat 分工

### Chat

当前更聚焦“这轮对话时最该看到的信息”：

- 当前模型
- 记忆窗口
- 生成中状态
- 流式输出
- 会话列表 / 历史消息

补充：

- 输入交互已改为：`Enter` 发送，`Shift + Enter` 换行
- Chat 历史现在按**可见消息**计数，不让内部记录占掉消息泡额度
- 多 session 已可用；会话上下文彼此独立，但底层运行配置仍是全局共享

### Dashboard

当前更聚焦“本地实例按什么配置在跑”：

- provider 状态
- runtime 状态
- connectivity 诊断
- 本地配置快照
- Provider Settings 最小写入
- 工具运行态概览
- Debug Console 入口

## 5. Debug / Runtime Console

### 当前状态

已落地一个 **Dashboard 内嵌的只读 Debug Console 原型**。

它不是完整浏览器终端，也不是可交互 CLI，而是：

- 只读
- 终端风格
- 面向观测和排障

当前可以看到：

- runtime snapshot
- 最近几次操作块
  - 对话 turn
  - provider apply / validate
  - preferences apply
  - token pairing
- 简化过的 process output

当前设计重点已经从“把卡片塞进终端皮肤”调整为：

- 一次操作一段反馈
- 更像只读 CLI 输出块
- 内容在窗口内滚动，不拉长整个页面

### 还没做到的

- 真正的 CLI stdout 原样镜像
- 完整的连续事件流
- 多 session 并行调试视图
- 更细的 turn / provider / tool 历史筛选

## 6. Provider / Tool / Routing 诊断

最近这轮开发已经证明，很多问题不能简单当成“Web bug”。

### Provider transport

尤其是 Volcengine / Ark 这类 host，在代理 / TUN / fake-ip 环境下会出现：

- 短请求偶发成功
- 稍长 completion 更容易失败
- Web 和 CLI 都会继承同一条 provider transport 问题

因此当前已经补上：

- provider host DNS 解析检查
- fake-ip 命中判断
- endpoint 基础 probe
- route guidance

### Tools

当前还存在一个重要产品/运行时问题：

- `tool.search` 对中文和泛化工具意图的召回不足
- 用户即使明确说“请使用 shell / file 工具”，模型也常常并没有真的发起工具调用
- Debug Console 现在已经能明确显示：
  - 本轮有没有真实 tool call
  - 还是模型只是口头说“我检索过了”

这部分当前更像 runtime / tools 侧问题，而不是单纯 Web 问题。

## 7. 近期新增事项

这段时间新增且值得记录的事项：

- Dashboard `Provider Settings` 已接到真实写入接口，不再只是壳子
- provider apply 改成“当前页验证”，不再强制回 onboarding
- Dashboard 工具区已对齐上游新增能力：
  - `web_search`
  - `browser_companion` 运行态
  - `file_tools` 聚合项（覆盖 `file.edit`）
- Mac 端已补 `start-dev.sh / stop-dev.sh`
- Debug Console 已支持更像“按操作分段”的展示
- Chat 历史显示已修正为按**可见消息**计数

## 8. 当前已知边界

当前仍未完成：

- 所有 provider 路径的统一真流式
- cancel / reconnect / resume
- 完整 tool trace 面板
- 更完整的 memory / tools / prompt Web 写入
- 更完整的 Dashboard 受控写入
- 安装态级别的自动 token 配对
- 更像真实 CLI 的连续输出流 Debug Console
- `tool.search` 的中文 / 泛化意图召回问题

## 9. 下一步建议

当前最适合继续推进的是：

1. 继续收 Debug Console 的内容噪音和分段体验
2. 继续补齐 Dashboard 的轻配置项写入
3. 针对 `tool.search` 中文 / 泛化召回问题推动后端修复
4. 继续温和拆分大文件，降低长期维护成本
5. 再进入可选安装和同源产品态能力实现
