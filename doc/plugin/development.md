# 插件开发与宿主施工

> 代码是事实标准，本文不逐次标注基线。契约细节见 [contracts.md](contracts.md)，架构见 [architecture.md](architecture.md)。

## 1. 插件作者快速开始

### 1.1 准备代码

复制整个 SDK 目录到本地开发目录：

- [`sdk/hinina.d.ts`](../modules/src-tauri/plugin/sdk/hinina.d.ts)
- [`sdk/scaffold/index.ts`](../modules/src-tauri/plugin/sdk/scaffold/index.ts)
- [`sdk/manifest.example.json`](../modules/src-tauri/plugin/sdk/manifest.example.json) → `manifest.json`

入口固定：

```ts
import type { Hinina, JsonValue } from './hinina'

export default async function activate(
  hinina: Hinina,
  config: Readonly<JsonValue>,
): Promise<void> {
  hinina.events.on('resync.required', async (event, envelope) => {
    // 根据当前状态重新同步；不要依赖事件重放。
  })
}
```

要求：

- 单个离线 ES module；TypeScript 在宿主外用 esbuild 等打包。
- 不使用 `node`/`fs`/`fetch`/`import` 运行时依赖；依赖必须打进 bundle。
- 所有宿主 Promise 必须 `await`，或进入 activate/handler 返回的 Promise 链；并发用 `await Promise.all`。
- `events.on` 与注销函数是同步的，不需要 `await`。
- 不要依赖 `void apiCall()`、未返回的 `.then()` 或游离异步任务。

### 1.2 构建

仓库根目录：

```powershell
npm run plugin:sdk:check
npm run plugin:scaffold:build
```

- `plugin:sdk:check`：TypeScript strict 类型检查与非法能力负例。
- `plugin:scaffold:build`：用 Vite 锁定的 esbuild 离线打包为 ES module，输出到 `src-tauri/target/plugin-scaffold.js`。
- 将产物复制到插件目录并命名为 `index.js`，与 `manifest.entry` 一致。
- 需要 Node.js ≥20.6（脚本使用 `import.meta.resolve`）。

独立工程可安装成熟 esbuild（例如 `npm install --save-dev esbuild`）。

### 1.3 放入目录并批准

```text
<base_dir>/
  plugins.json
  plugins/com.example.development/
    manifest.json
    index.js
  plugin-data/<插件ID的SHA-256>/
```

- 设置页可查看当前数据目录。
- 把 `plugins.example.json` 复制为数据目录的 `plugins.json`。
- 只有 `enabled=true` 且审批匹配的插件才加载；没有配置时不执行任何插件。
- 也可以在 `plugins.json` 顶层设置绝对 `codeRoot`，每个子目录一个插件。
- 修改清单权限不会自动获得能力；更新宿主批准后 `reload`。
- 网络 origin 必须同时出现在 `manifest.network.allowOrigins` 和宿主 `network` 列表；私网/HTTP 另需显式授权。
- **插件 HTTP 拒绝 loopback/私网地址**：不能用本地 mock server 做端到端验收——用公网 HTTPS echo 端点接收假数据，验收完即关（见 `sdk/examples/balloon/README.md` 的验收指南）。
- `options` 作为 activate 第二个参数传入并深冻结。

### 1.4 启动、调试、重载

```powershell
npm run tauri dev
```

桌面 WebView 开发控制台：

```js
await window.hininaPlugins.list()
await window.hininaPlugins.start('com.example.development')
await window.hininaPlugins.resync('com.example.development')
await window.hininaPlugins.reload('com.example.development')
await window.hininaPlugins.stop('com.example.development')
```

- `window.hininaPlugins` 仅在 DEV 提供；release 仍按批准配置自动启动插件（四个命令在 release 构建中依然注册，只是前端不暴露入口）。
- 已在运行的插件再次 `start` 会报错（重复 start 只拒绝命令、不写失败历史）。
- **`enabled` 只由 `plugins.json` 决定**：`stop` 是"本次停止"，不改变自动启用意图——重启应用后 enabled 插件仍会自动启动；持久禁用必须改 `plugins.json`。
- **`reload` 是开发期便利**（免于重启 `tauri dev`）：旧 worker 真正退出后才加载新源码。**发布形态的语义是"改配置/改代码后重启应用生效"**。
- **运行时不提供 `setTimeout` 或任何定时器**：所有节拍只能来自 Resync（周期默认 60 秒，`plugins.json` 的 `resyncIntervalSecs` 可调，最小 10 秒）。不要试图轮询或自造时钟；时间引用可用事件 `envelope.occurredAt`。
- 底层命令为 `plugin_list/start/stop/reload/resync`，不接收代码/权限/路径。
- 前端入口仍经过 Store → Service → Bridge。
- `list` 显示 `starting/running/stopped/failed` 和安全错误码。
- 没有 JS console 或逐步调试器；通过状态、私有存储、`hinina.log.*` 和获准通知做有限探针，勿输出敏感信息。

### 1.5 查询示例

```ts
const context = await hinina.context.current()
const contest = await hinina.contest.current(context)
if (contest) {
  let cursor: string | null = null
  do {
    const page = await hinina.submission.list(context, {
      contestId: contest.contestId,
      cursor,
      limit: 100,
    })
    // 用 submissionId 去重；page.total 可能为 null。
    cursor = page.nextCursor
  } while (cursor !== null)
}
```

实际插件需要：

- 设置扫描预算并保存可恢复状态；
- 处理 `context_expired`（停止本次操作，等下次 Resync）；
- 不让整个 handler 超过 30 秒；
- 空 `records` 仍可能有 `nextCursor`；
- 不把“当前用户可见历史”当成“全站所有队伍的首次 AC”。

### 1.6 日志

`hinina.log.debug/info/warn/error(message)` 把一条文本写进宿主日志文件
`logs/hinina.log`（设置页可查看路径）。无需权限，也不读任何宿主数据。

- 参数必须是字符串；对象自己 `JSON.stringify`，宿主不注入 `console`。
- 单条与速率上限见 `plugin/limits` 的 `PLUGIN_LOG_BYTES`/`PLUGIN_LOGS_PER_MINUTE`（[contracts.md §10](contracts.md)），超限抛 `limit_exceeded`，不静默丢弃。
- 换行/制表符写入前被拍平为空格，消息整体 Debug 转义；日志按行可 grep。
- `debug` 级跟随宿主日志级别：release 默认 `info` 会过滤 `debug` 级插件日志
  （被过滤的调用不消耗限流额度），开发构建或 `RUST_LOG=debug` 可见全部。
- 日志是排错通道，不是业务凭据；不要写 Token、用户名、源码或响应正文。

### 1.7 榜单与题目列表

```ts
const context = await hinina.context.current()
const contest = await hinina.contest.current(context)
if (contest && !contest.sealed) {
  const problems = await hinina.contest.problems(context)
  let page = 1
  for (;;) {
    const board = await hinina.contest.rank(context, { page, limit: 200 })
    for (const row of board.rows) {
      if (row.rank === -1) continue // 打星队
      for (const cell of Object.values(row.cells)) {
        if (cell.firstAc) { /* row.teamKey + cell.displayId 首A */ }
      }
    }
    if (!board.hasMore) break
    page += 1
  }
}
```

- `contest.rank` 需在 Manifest 声明**且** `plugins.json` 批准，同时还要有 `contest.read`。
- **先看 `contest.current` 的 `sealed`**：为 true 则整轮直接结束，不读榜单、不派发——
  封榜期间 `solved`/`firstAc` 恒 false，与真没通过不可区分；解封后不补派历史首 A。
  `sealedSince` 是封榜**开始**时刻（服务端不下发结束时刻，别拿它当解封时间）。
- 跨页无事务快照：按 `teamKey` 去重；**不提供 `total`**（HOJ 的含服务端前置条目且
  偏大）——行数以去重后非打星行为准。
- `teamKey` 与 `identity.current(context).userKey` 同算法：相等即自己那队。
- `displayName` 由宿主按比赛 `rankShowName` 解析（与 App 榜单同规则），无需自己拼。
- `limit` 是提示：Hydro 无视分页、整榜单页返回（行数可能超过 limit）；`page` 回显
  服务端实际页（Hydro 恒 1），`hasMore=false` 即整榜已取完。
- HOJ 榜单实时全量计算，较慢：**榜单查询有每激活 `PLUGIN_RANK_QUERIES_PER_MINUTE`
  次/分钟的独立预算**（突发等于配额覆盖分页同步；持续轮询建议 ≥30s，节拍只能来自
  Resync，运行时无 `setTimeout`）；每次调用另消耗 2 份通用 Provider 预算（元信息 + 榜单）。
- Hydro 单页返回全量：超过 200 行或总 cells 超限会得到 `limit_exceeded`，大榜单可能不可用。
- `color` 可能为空串（Hydro）；不能自己编颜色。
- 查询入参不带 `contestId`（多余字段直接拒绝）；未选中比赛返回 `context_expired`，等下次 Resync。

### 1.8 常见失败

| 现象 | 排查 |
|---|---|
| 插件未加载 | `plugins.json` 是否存在、`enabled`、ID 是否匹配、清单/版本/入口是否合法 |
| `permission_denied` | Manifest 是否声明、`plugins.json` 是否批准、事件载荷是否还需领域读取权限 |
| `context_expired` | 登录/登出、切 OJ/比赛、连接配置变化；重新 `context.current()` |
| `limit_exceeded` | 分页/扫描/存储/HTTP/通知/日志/榜单/桥调用预算；缩小批量、保存进度 |
| `timeout` | activate `ACTIVATION_TIMEOUT`、事件 `EVENT_TIMEOUT`、单请求 `PROVIDER_TIMEOUT`、`submission.get` `SUBMISSION_LOOKUP_TIMEOUT`；拆分工作 |
| `unsupported` | 调用了未实现的 API（题面/工作区/UI 等占位能力） |
| `script_error` | 语法错误、未捕获异常、内存分配失败以外的脚本失败；看宿主日志的稳定码，并在关键步骤用 `hinina.log.*` 输出阶段 |
| 启动失败但无栈 | 设计如此；用状态、存储、通知和宿主 tracing 定位 |

### 1.9 参考业务插件

[`sdk/examples/balloon/`](../modules/src-tauri/plugin/sdk/examples/balloon/README.md)
是完整的首次 AC 气球派发参考实现（阶段 C 交付）：Resync 驱动、封榜门控、
跨页去重、`pending`→`sent` 幂等派发、封榜窗口不补派、`context_expired` 停轮、
`hinina.log.*` 全程可观测。构建用 `npm run plugin:balloon:build`；安装、批准与
端到端验收步骤见其 README。业务规则（首次 AC 判定、幂等、恢复）留在插件，
核心 Service 不参与。

## 2. 插件作者契约清单

- [ ] 入口是单个离线 ES module，`export default async function activate(hinina, config)`。
- [ ] 所有宿主 Promise 都 `await` 或返回链；没有游离 Promise。
- [ ] 事件 handler 串行、幂等、可重入；不依赖事件重放。
- [ ] 每个 handler 重新获取 context；遇到 `context_expired` 停止本次操作。
- [ ] 分页用 `nextCursor` 判断末尾，不靠 `records` 是否为空；按 `submissionId` 去重。
- [ ] 大量历史用 `list` 分批，保存进度；不要用 `submission.get` 反复全量扫描。
- [ ] 存储用 `missing` 与 JSON `null` 区分状态；CAS 只保证本地，不保证远端 exactly-once。设计可恢复状态时注意：`CAS(null)` 不能把 `dispatch_failed` 再变回 `dispatching`，崩溃后的 `dispatching` 也会卡住——显式设计状态机、租期/重试预算，并让外部服务通过幂等键去重；本地原子 CAS 不能撤回远端已成功但客户端超时的派发。
- [ ] HTTP 使用外部幂等键；超时/500 由插件自行重试，核心不重试派发。
- [ ] 不把通知当成功凭据；不输出 Token、用户名、源码、路径或响应正文。
- [ ] 排错用 `hinina.log.*` 输出阶段与计数；不写敏感内容，不依赖日志做业务判断。
- [ ] 业务规则（首次 AC、可见历史完整性、稳定排序、派发恢复）由插件实现。

## 3. 宿主施工步骤

以下顺序来自已交付的实现，可作为继续开发时的施工模板。

### 阶段 0：审计与引擎可行性

- 先核对真实代码，不把文档里的计划当已实现。
- 确认关键副作用：`AuthService.get_session` 会恢复凭证、比赛加载会自动选赛、提交详情含源码、Hydro 分页不能证明历史完整。
- 验证引擎能力：ES module、Rust Future/JS Promise、死循环中断、内存限制、无 ambient I/O。
- 当前选择 `rquickjs`（std + futures）；构建需要 C 编译器（Windows MSVC / Linux gcc / clang）。

### 阶段 1：访问作用域与只读 Service

- 新增通用 `AccessRevision`，显式接入登录/登出/会话失效、OJ/比赛/连接配置变化。
- 转换期间 `snapshot()` 返回 `None`；revision 回绕安全；在途调用可取消。
- 为插件新增只读 Service 方法：身份、比赛元数据、本人提交历史；不读源码、不读详情。
- 实际 endpoint 来自 Provider 注册元数据，不来自可变配置。
- 新增变更入口必须接入同一 fence，并补契约测试。

### 阶段 2：协议、安全投影与 API Adapter

- 定义 Manifest/API v2、事件协议 v2、点分事件名。
- 定义安全 DTO、权限 trait、错误白名单。
- 查询上下文绑定实际 endpoint + 稳定用户 + 比赛 + revision。
- 游标不透明、单次使用、绑定 context/query；可重试失败回填。
- 提交查询强制本人，使用宿主私有 `owner_id`，不读源码详情。
- 只返回必要字段；不序列化内部实体、Token、路径或原始响应。

### 阶段 3：存储、HTTP、通知

- 存储：目录名为插件 ID 的 SHA-256；fs4 锁 + atomicwrites；`missing`/`null` 区分；配额与限流。
- HTTP：独立 reqwest；精确 origin；全部 DNS 结果校验并固定；无 Cookie/代理/跳转/重试；私网/HTTP 额外批准。
- 通知：纯文本、长度/频率限制；前端经 Service 校验与过期。
- 所有实现放在 `infra/`，通过端口注入 `plugin/`。

### 阶段 4：仓库、Runtime、生命周期

- `PluginRepository` 位于 `plugin/registry`；文件实现位于 `infra/fs_plugin_repo`。
- 仓库校验清单、版本、重复 ID、入口 canonical 边界、源码大小；逐项隔离坏条目。
- 每插件独立 worker/引擎；使用 Tauri 持久 Tokio handle，不创建第二个 runtime。
- 生命周期串行化；激活结果在发布 ready 前分类；stop 超时保持 `failed/timeout` 并由 reaper 回收。
- worker panic 用 `catch_unwind` 隔离；`Persistent` 先于 runtime 释放。
- 单 handler 异常隔离；引擎级失败才使插件失败。

### 阶段 5：开发入口、装配、Resync

- 提供 `plugin_list/start/stop/reload/resync` 命令；不接收代码/权限/路径。
- `AppContext` 只持有 Send 控制器；JS 值留在 worker。
- 仅实际获批 `events.subscribe` 才建立事件队列并接收 Resync。
- 周期 Resync 默认 60 秒；初始/Lagged/账号/OJ/比赛变化也可触发。
- 前端按 View → Store → Service → Bridge 接入；通知策略在 Service。

### 阶段 6：验证与文档

- Rust：`cargo test --manifest-path src-tauri/Cargo.toml`
- 前端：`npm test`、`npm run build`
- SDK：`npm run plugin:sdk:check`
- 脚手架：`npm run plugin:scaffold:build`
- 真实引擎覆盖 Promise、中断、内存、有界桥、存储/CAS、事件、handler 隔离、worker 取消。
- HTTP 传输测试用本地 mock server，不访问外网。
- 同步 `doc/Architecture.md`、`doc/modules/` 和本目录文档。

## 4. 新增一个插件能力的施工顺序

以“新增一个只读能力”为例：

1. **契约**：在 `plugin/api/` 定义安全 DTO、trait、权限和错误映射；确定是否需要新的 `PluginPermission`。
2. **端口**：需要 I/O 时定义端口，不在 `plugin/` 引用具体 infra。
3. **实现**：Service 提供只读方法；Infra 实现端口；组合根注入。
4. **桥**：在 `execution_api::dispatch` 增加 operation，参数/响应保持 camelCase，检查字节上限。
5. **JS 表面**：在 `runtime/bootstrap.js` 增加冻结方法；在 `hinina.d.ts` 增加声明和 `@permission` 注释。
6. **权限**：更新 Manifest 校验、审批交集、事件载荷权限（如适用）。
7. **测试**：Rust DTO/权限/边界测试 + 真实 QuickJS 测试 + 前端错误/通知测试（如涉及）。
8. **文档**：更新 `contracts.md`、模块文档、SDK 示例；破坏性变更递增版本。

禁止：

- 在 `plugin/` 直接构造文件/网络客户端；
- 把内部 `AppContext`、Service、Provider、`CoreEvent`、`Receiver`、Session、Token 暴露给 JS；
- 用事件作为安全控制路径；
- 为插件业务在核心 `SubmissionService` 中加入首次 AC/派发规则。

## 5. 验证命令

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
npm test
npm run build
npm run plugin:sdk:check
npm run plugin:scaffold:build
```

只改文档时无需跑上述命令，但要检查链接与文件树是否同步。

## 6. 当前不做

- ~~气球插件、首次 AC 判定、外部派发/重试~~ 参考实现已交付（`sdk/examples/balloon/`，见 §1.9）；真实业务的首次 AC 定义与派发策略由插件作者实现，不进核心
- 管理 UI、配置写入、安装/更新/市场
- 暂停/恢复、WASM、UI 扩展、自定义页面
- 远程代码热更新、依赖求解
- OS 级进程沙箱

这些属于后续路线，见 [roadmap.md](roadmap.md)。