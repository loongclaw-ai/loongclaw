# LoongClaw Web 设计进度

## 1. 当前定位

LoongClaw Web 现在已经不是纯壳子，而是一个可实际使用的本地 Web Console：

- 可进入 chat / dashboard
- 已接入真实 runtime
- 已接入 onboarding 首屏检查
- 已支持最小 provider 配置写入
- 已支持最小验证与放行
- 已支持 token 轻自动配对
- 已开始补入 CLI 轻配置项

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

## 3. 当前 onboarding 进度

### O1：首次进入状态检测

已完成。

当前已有：

- `GET /api/onboard/status`
- runtime / token / config / provider 四项状态聚合
- 首屏 onboarding 状态面板

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
- 写完 provider 配置后做最小验证
- 只有验证通过，才允许从 onboarding 首屏进入 Web

当前验证关注的最小问题是：

- endpoint 是否可达
- key 是否通过基础探测

### O4：token / pairing 收口

当前属于 **部分落地，但已经有轻自动化**。

已完成：

- token 配对已收进 onboarding 面板
- 不再依赖独立顶部 token banner
- Web 会优先尝试一次轻量自动配对
- 自动配对成功后，通过本地受信 cookie 建立当前浏览器的配对状态
- 自动配对失败时，回退到手动输入本地 token

当前这版的目标不是“把 token 明文交给前端”，而是：

- 优先自动完成本地配对
- 保留手动兜底
- 不把 token 公开暴露给页面脚本

仍未完成：

- 安装态 / 同源态下更顺滑的自动配对
- 更少暴露底层 token 文件路径概念
- 更长期的无感 pairing 体验

### O2.5：轻配置项补齐

当前已开始落地第一版。

已完成：

- onboarding 首屏新增“可选个性化设置”折叠区
- 可在首次进入时按需设置：
  - `personality`
  - `memory_profile`
  - `prompt addendum`
- 默认收起，不阻塞首次进入

当前边界：

- `system_prompt` 仍然不可直接修改
- 轻配置项目前优先放在 onboarding 首屏
- Dashboard 仍以只读展示为主，后续再视情况补写入

## 4. Dashboard / Chat 当前分工

### Chat

当前更聚焦“正在使用这轮对话时最该看到的信息”：

- 当前模型
- 记忆窗口
- 生成中状态
- 流式输出

### Dashboard

当前更聚焦“本地实例到底按什么配置在跑”：

- provider 状态
- runtime 状态
- connectivity 诊断
- 本地配置快照
- Provider Settings 最小写入

CLI 的轻配置项当前已开始进入 Web，但交互仍优先落在 onboarding 首屏。

## 5.1 Debug / Runtime Console 方向

后续 Web 很可能需要一个更适合观测和调试的控制台视图，但当前不建议直接在浏览器里做“完整 CLI / 完整终端”。

更适合的方向是：

- 优先做只读的 Runtime / Debug Console
- 展示 turn 事件、tool 调用、provider 诊断、session 元信息
- 在确认边界清楚后，再逐步补少量受控调试动作

这条路线的目标是：

- 提升 Web 内的观测与 debug 能力
- 帮助团队排查 onboarding、provider、tool、streaming 问题
- 避免一开始就把浏览器页面做成高风险的任意命令终端

## 6. Provider route / transport 诊断

最近这轮开发已经证明，provider transport 问题不能简单当成“Web bug”。

尤其是 Volcengine / Ark 这类 host，在代理 / TUN / fake-ip 环境下会出现：

- 短请求偶发成功
- 稍长 completion 更容易失败
- Web 和 CLI 都会继承同一条 provider transport 链问题

因此当前设计里已经补上：

- provider host DNS 解析检查
- fake-ip 命中判断
- endpoint 基础 probe
- route guidance

这层能力的目标不是自动改用户网络，而是：

- 更快定位问题
- 降低把 transport 问题误判成 Web 问题的概率
- 为后续 `doctor` / onboarding 提供基础诊断能力

## 7. 当前已知边界

当前仍未完成：

- 所有 provider 路径的统一真流式
- cancel / reconnect / resume
- 完整 tool trace 面板
- 更完整的 memory / tools / prompt Web 写入
- 更完整的 dashboard 受控写入
- 安装态级别的自动 token 配对

## 8. 可选安装（当前状态）

### 目前有没有实现

还没有完整实现“可选安装”。

当前真实状态仍然是：

- Web API 运行形态仍是 `loongclaw web serve`
- `web_install_mode` 当前基本仍是 `api_only`
- 还没有完整的：
  - `web install`
  - `web remove`
  - `web status`
  - 静态资源安装与本地托管闭环

所以现在更像：

- 开发态 / 本地控制台
- 而不是面向普通用户的一键安装产品形态

### 之后能不能实现

可以，而且方向已经比较清楚。

如果后续要做可选安装，通常会包括：

- 安装目录约定
- `web install / remove / status`
- 静态资源托管
- 与 onboarding 配合的首次进入体验
- 逐步向同源产品态收敛

如果后续同时支持：

- 本地可选安装
- 官方 host

那么产品态会更适合朝“同源设计”收敛；开发态仍可继续保留当前分离式结构，以保持迭代效率。

## 9. 下一步建议

当前最适合继续推进的是：

1. 继续补齐 O2.5，把轻配置项从 onboarding 延伸到更稳定的长期入口
2. 完成 O4 剩余收尾，让 token / pairing 更顺滑
3. 继续温和拆分大文件，降低长期维护成本
4. 再进入可选安装的产品化实现
