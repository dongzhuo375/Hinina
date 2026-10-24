# 插件系统统一契约

> 基线：`main@8379772`。本文是宿主与插件作者之间的**统一契约**。实现细节见 [architecture.md](architecture.md)，模块级说明见 [`../modules/src-tauri/plugin/`](../modules/src-tauri/plugin/README.md)。
> 代码是事实标准；任何契约变更必须同时更新 Rust 类型、JSON 桥、`hinina.d.ts`、模块文档和契约测试。

## 1. 版本契约

| 版本 | 当前值 | 含义 | 破坏性变更 |
|---|---|---|---|
| `manifestVersion` | 2 | 清单格式 | 递增 `manifestVersion` |
| `apiVersion` | 2 | 宿主 API 表面 | 递增 `apiVersion` |
| 事件 `version` | 2 | 事件协议/信封 | 递增事件协议版本 |

三者语义独立，恰好都为 2 不代表必须一起升级。“插件系统 1.0”也不要求这些字段填 1。

规则：

- v1 清单/API 不再加载；没有静默兼容、默认扩权或 `runtimeVersion` 字段。
- 删除变体、改字段语义、改必填性都算破坏性变更。
- 插件据 `envelope.version` 决定是否继续处理。
- 当前 SDK 以 [`hinina.d.ts`](../modules/src-tauri/plugin/sdk/hinina.d.ts) 为准。

## 2. 发布物与入口契约

### 2.1 目录布局

```text
<base_dir>/
  plugins.json                         # 宿主审批，位于代码目录之外
  plugins/<plugin-id>/
    manifest.json
    index.js                           # 与 manifest.entry 一致
  plugin-data/<sha256(plugin-id)>/
    storage.json
```

- `codeRoot` 可由管理员指向其他开发根目录；每个子目录一个插件。
- 审批、代码、数据三者分离；插件不能指定真实文件路径。
- 没有 `plugins.json` 时默认不启用任何插件。

### 2.2 入口

- 单个**离线打包**的 ES module；TypeScript 在宿主外构建。
- 固定签名：

```ts
export default async function activate(
  hinina: Hinina,
  config: Readonly<JsonValue>,
): Promise<void>
```

- `entry` 必须是插件目录内的 `.js` 相对路径；拒绝 `..`、反斜杠、绝对路径、Windows ADS、空段、`.`/`..` 段、尾随点/空格；canonical 路径必须仍在插件目录内。
- 源码 ≤1MiB；`config` 是 `plugins.json` 的 `options`，JSON ≤64KiB，深冻结。
- 无 resolver/loader：不支持任意 `import`、原生模块、`fs`/`process`/`env`/`fetch`/`socket`/`shell`。
- **所有宿主 Promise 必须 await**，或进入 activate/handler 返回的 Promise 链；`events.on` 与注销函数是同步的。
- 不支持游离 Promise：入口结束后不 drain，不保证执行或报告拒绝。

## 3. Manifest v2

| 字段 | 规则 |
|---|---|
| `id` | 1..128 字节；小写字母开头的点分段；每段可含小写字母、数字、连字符；扫描中重复 ID 拒绝 |
| `name` | 非空，≤128 字节 |
| `version` | 插件自身 SemVer，由 `semver` 校验，≤128 字节 |
| `manifestVersion` | 必须为 `2` |
| `apiVersion` | 必须为 `2` |
| `runtime` | 必须为 `"js"` |
| `entry` | 见 §2.2 |
| `description` / `author` | 可选字符串 |
| `minAppVersion` | 可选 SemVer；高于宿主版本则不加载 |
| `permissions` | 能力声明；不得重复；声明不等于授权 |
| `events` | 事件订阅声明；不得重复；非空时必须声明 `events.subscribe` |
| `network.allowOrigins` | 精确规范化 origin；只允许域名；不含路径/认证/通配符/查询/片段；≤32 个 |
| `network.maxBodyBytes` | 1..65536 |
| `network.timeoutMs` | 1..5000 |
| `contributes` | 当前只接受 `[]` |
| `configSchema` | 当前只接受 `{}` |

未知字段、未知权限、未知事件在反序列化时拒绝。`network` 与 `network.request` 必须一致：声明该权限时至少一个 origin；有 origin 时必须声明该权限。

## 4. 宿主审批契约（`plugins.json`）

结构见 [`plugins.example.json`](../modules/src-tauri/plugin/sdk/plugins.example.json)。

```json
{
  "codeRoot": "D:/optional/dev/plugins",
  "plugins": {
    "com.example.development": {
      "enabled": true,
      "permissions": ["events.subscribe", "storage.read", "storage.write"],
      "network": [],
      "resyncIntervalSecs": 60,
      "options": {}
    }
  }
}
```

| 字段 | 规则 |
|---|---|
| `enabled` | 默认 `false`；只有显式 `true` 才尝试启动 |
| `permissions` | 宿主批准；与 Manifest 声明取交集；不能批准未声明能力 |
| `network` | 每个元素 `{origin, allowHttp?, privateCidrs?}`；origin 必须同时出现在 Manifest 中 |
| `resyncIntervalSecs` | 10..3600，默认 60 |
| `options` | 传给 activate 的第二个参数；≤64KiB；深冻结 |
| `codeRoot` | 可选；指向开发插件根目录 |

规则：

- 有效权限 = `manifest.permissions ∩ approval.permissions`。
- 清单修改、重载、更新代码都不会自动获得新权限；必须显式更新审批。
- 审批文件位于代码目录之外；默认不启用任何插件。
- 网络额外授权：明文 HTTP 需 `allowHttp: true`；私网 CIDR 需 `privateCidrs` 明确列出；loopback/link-local/保留地址即使写入 CIDR 也拒绝。
- 整体配置损坏或根字段非法会拒绝整份配置；单个插件审批非法只影响该项，并在 `invalid_plugins` 中报告。
- 目录扫描上限 128 个目录、1024 个条目；`plugins.json` 最多 128 项。

## 5. 权限与事件白名单

### 5.1 权限

| 权限 | 允许的能力 |
|---|---|
| `events.subscribe` | 注册事件 handler、接收初始/周期/Lagged Resync |
| `submission.read` | `submission.get` / `submission.list`；`submission.*` 事件载荷 |
| `contest.read` | `contest.current` / `contest.get` / `contest.problems`；`contest/announcement/problem` 事件载荷 |
| `contest.rank` | `contest.rank`（还需同时具备 `contest.read`）；全场榜单：其他队伍的显示名与学校 |
| `identity.read` | `identity.current` |
| `storage.read` | `storage.get`；`compareAndSet` 的读侧 |
| `storage.write` | `storage.set` / `delete`；`compareAndSet` 的写侧 |
| `network.request` | `http.request`，且 origin/地址获批 |
| `notification` | `notification.info/success/warn/error` |

### 5.2 事件

| 事件 | 载荷 | 额外权限 |
|---|---|---|
| `system.oj_switched` | `ojId` | 无 |
| `contest.selected` | `contestId` | `contest.read` |
| `announcement.changed` | `contestId`, `newCount` | `contest.read` |
| `problem.opened` | `contestId`, `problemId` | `contest.read` |
| `submission.created` | `submissionId` | `submission.read` |
| `submission.judged` | `submissionId`, `status` | `submission.read` |
| `workspace.saved` | `workspaceId`, `revision`, `automatic` | 无；仅元数据 |
| `system.config_changed` | 无 | 无 |
| `system.theme_changed` | 无 | 无 |
| `session.logged_in` | `ojId` | 无；不含用户标识 |
| `session.logged_out` | `ojId` | 无 |
| `session.expired` | `ojId` | 无 |
| `resync.required` | `lost` | 强制控制通知，无需声明 |

`TokenRotated` 不在协议内。事件不得承载源码、Token、认证头、内部路径、Session、原始 Provider 数据或内部错误。

## 6. Plugin API v2

真实方法以 [`hinina.d.ts`](../modules/src-tauri/plugin/sdk/hinina.d.ts) 为准。宿主注入的对象已冻结。

| 能力 | 调用 | 权限 |
|---|---|---|
| 上下文 | `context.current()` | 无；无事件授权也可启动 |
| 身份 | `identity.current(context)` | `identity.read` |
| 比赛 | `contest.current(context)` / `contest.get(context, contestId)` | `contest.read` |
| 题目列表 | `contest.problems(context)` | `contest.read` |
| 榜单 | `contest.rank(context, {page, limit})` | `contest.rank` + `contest.read` |
| 提交 | `submission.get(context, contestId, submissionId)` / `submission.list(context, query)` | `submission.read` |
| 存储 | `storage.get/set/delete/compareAndSet` | `storage.read` / `storage.write`；CAS 两者都需 |
| HTTP | `http.request(context, request)` | `network.request` + origin/地址批准 |
| 通知 | `notification.info/success/warn/error(message)` | `notification` |
| 日志 | `log.debug/info/warn/error(message)` | 无；只写宿主日志文件，不读宿主数据 |
| 事件 | `events.on(name, handler)`，返回注销函数 | `events.subscribe` + 领域读取权限 |

当前**没有**可执行的题面、工作区或 UI 扩展 API；`api/problem.rs`、`api/workspace.rs`、`api/ui.rs` 只是远期草案占位。公告只有 `announcement.changed` 事件与数量，没有正文查询 API。

### 6.1 查询上下文

- `context.current()` 返回不透明随机句柄，绑定：本次授权、实际运行的 Provider endpoint、稳定用户 ID、当前比赛、`AccessRevision`。
- TTL 300 秒；最多同时保留 32 个。
- 同一 scope 与 revision 下，150 秒内的未过期句柄会被复用（返回同一句柄）；超龄、scope 或 revision 变化才新签发。
- 登录/登出/会话失效、切 OJ/比赛、连接配置变化会撤销旧句柄；切走再切回不会复活。
- 旧句柄上的查询返回 `context_expired`；遇到后停止本次操作，等下次 Resync 重查。
- 修改 OJ 实例 `baseUrl` 沿用“重启后生效”：查询继续绑定运行中的 endpoint；旧句柄仍因配置变更撤销，新句柄使用实际 endpoint。
- 实例被删除或禁用则拒绝访问。

### 6.2 提交查询

`submission.list(context, {contestId, problemId?, cursor?, limit})`：

- `limit` 1..100；返回 `{records, total: number|null, nextCursor: string|null}`。
- 只有 `nextCursor === null` 才证明到达服务端可见历史末尾；`records: []` 仍可能有 `nextCursor`。
- 强制本人；按稳定 `owner_id`（HOJ uid / Hydro rdoc.uid）校验，不用展示名；缺失/不匹配整页拒绝。
- 只允许当前上下文的比赛；`problemId` 是真实题号。
- 投影字段：`submissionId`、`contestId`、`problemId`、`problemDisplayId`、`status`、`submittedAt`（UTC 毫秒）。
- 不返回源码、Token、owner ID、评测点、HTML 或原始响应。
- 跨页没有事务快照；新提交/重判可能导致重复或移位，按 `submissionId` 去重。
- HOJ 使用原生 total/pages；Hydro `total=null`，继续到真实空 native page。

`submission.get(context, contestId, submissionId)`：

- 在本人比赛历史中查找元数据，不访问含源码详情。
- 最多 100 个 API 页，整体 8 秒；每个 native 页单独计费。
- 超预算返回 `limit_exceeded`，超时返回 `timeout`；只有确实到达末尾才返回 `null`。
- 大量历史应使用 `list` 分批并保存进度。

### 6.3 游标

- 不透明、单次使用、绑定 context + 比赛 + 题目 + limit。
- TTL 300 秒；最多 32 个；单个 native 页保留最多 1000 条/1MiB，最多 10000 个 native page；所有保留余项合计 ≤4MiB。
- 合法领取即消费旧游标；并发复用只有一个成功，其他收到 `context_expired`。
- 可重试失败（预算、provider 错误、超时、响应超限、游标配额）把游标按调用前快照归还，重试从同一 native 页继续。
- 只有 `context_expired` 不归还，因为作用域已撤销。
- 跨作用域或不同 query 参数的非法领取不会消费游标。

### 6.4 题目列表与榜单

`contest.problems(context)`：

- 需 `contest.read`；返回当前比赛的 `{problemId, displayId, title, color}`。
- `color` 为气球颜色，Hydro 无此字段（空串）；插件必须容忍空色，不能自己编颜色。
- `title`/`color` 按字节截断（≤256/≤64 UTF-8 字节，字符边界安全）：千题级比赛的
  列表仍可完整编码，而非被 1MiB 桥上限拒绝。
- 未选中比赛 → `context_expired`；不自动选中比赛、不落缓存。

`contest.rank(context, {page, limit})`：

- 需 `contest.rank` **∩** `contest.read`，缺任一返回 `permission_denied` 并指认缺失项。
- 入参不带 `contestId`（`deny_unknown_fields`，多余字段拒绝）；未选中比赛 → `context_expired`。
- 不使用游标：榜单是实时视图，没有快照语义。`limit` 是**提示**：HOJ 按其分页，
  Hydro 无视分页参数、整榜单页返回（行数可能超过 limit，仍受边界约束）。
- 边界（可组合）：`page` 1..100、每页 ≤200 行、单行 ≤64 个题目列、**单页总 cells ≤6,000**；
  行文本（`displayName`/`school`）按**字节**截断（≤96 UTF-8 字节，字符边界安全），
  顶满名字的 200×30 页仍可完整编码；超出返回 `limit_exceeded`（Hydro 大榜单可能因此不可用），
  病态内容（超长 displayId、类型上限数字）由 1MiB 桥上限兜底拒绝而非截断。
- `page` 回显**服务端实际页**而非请求页：Hydro 恒为 1；`hasMore = current < pages`。
- 跨页无事务快照，需按 `teamKey` 去重；**不提供 `total`**（HOJ 的 total 含服务端
  前置条目且偏大）——行数以去重后非打星行为准。
- 投影：`rank`（-1 打星）、`teamKey`（与 `identity.userKey` 同算法，可直接比对自身）、
  `displayName`、`school`、`ac`、`totalScore`、`cells`（key = displayId）。
- `displayName` 由宿主按比赛 `rankShowName` 规则解析（与前端榜单同规则：
  realname/nickname/username 选字段，空则回退 username，再回退 uid 文本），
  保证插件榜与 App 榜显示一致。为此 `rank` 每次额外读取一次比赛元信息——
  仅用于名字解析，`sealed`/`contestType` 仍只来自 `contest.current`。
- 行 `uid` 为空（Hydro 无 `raw` 的 user cell）→ 整页 `provider_error`：
  空uid 无法生成稳定 `teamKey`，静默合并队伍比失败更糟。
- 不返回：`uid`（字段）、原始 `username`、`realname`、`nickname`、`avatar`、`gender`、
  `totalTime`、任何原始响应。
- 封榜态不来自榜单页：`PluginContest.sealed`（`seal_rank && seal_rank_time <= now`，
  与 RankView 同规则）解释"未 AC"是否可信；封榜期间 `solved`/`firstAc` 恒 false、
  `acTime` 恒 null。`sealedSince` 为封榜**开始**时刻（UTC 毫秒，仅封榜时非 null）；
  服务端不下发封榜结束时刻，故无 `sealedUntil`。
- HOJ 榜单为实时全量计算，较慢：**榜单查询另有每激活 6 次/分钟的独立预算**
  （突发 6 次覆盖分页同步，持续轮询 ≥10s，建议 ≥30s）；令牌在元信息校验通过后、
  榜单取数前扣——元信息失败不烧令牌，取数后的边界拒绝仍计费（OJ 已完成全榜重算）。
  每次调用另消耗 2 份通用 Provider 预算（元信息 + 榜单）；宿主不加缓存（缓存会给过期名次）。

## 7. 事件与 Resync 契约

信封：

```json
{
  "version": 2,
  "sequence": 2,
  "occurredAt": 1780000000000,
  "event": { "type": "submission.judged", "submissionId": "s1", "status": "Accepted" }
}
```

- `sequence` 在同一订阅内从 1 起；未通过白名单/声明/权限的事件不消耗序号；重新订阅从 1 开始，不能跨重启比较。
- `occurredAt` 是**转换时刻**，不是提交或终态实际发生时刻。
- 事件可能重复，不承诺 exactly-once；允许丢失。
- 每插件队列容量 128；满或关闭即注销订阅；慢插件不能令队列无限增长。
- 初始 Resync 先于其他投递；周期 Resync 默认 60 秒，范围 10..3600 秒。
- 账号/OJ/比赛变化、消费者 `Lagged(n)` 也会触发 Resync；`lost` 统计底层核心事件，不代表漏掉的 AC 数量。
- Resync 是“重新对账”提示，不是事件重放；查询当前状态才是真值。
- 单个 handler 抛错不会中断其他 handler 或后续事件；宿主记录脱敏失败计数，并主动入队一次 Resync 重新对账（Resync 自身失败不再触发，避免循环）。

## 8. Runtime 执行契约

| 项 | 规则 |
|---|---|
| 入口 | `activate(hinina, config)`；activate 10 秒 |
| 事件交付 | 一次事件总计 30 秒，包含全部 handler |
| 启动等待 | 12 秒（activate + 2 秒宽限） |
| 停止 | 5 秒 |
| handler 数量 | 单插件最多 64 个 |
| handler 顺序 | 按注册顺序串行 await |
| 单 handler 异常 | 只终止该 handler 本次调用；worker 继续；失败计数回传 |
| 引擎级失败 | 超时、内存、撤销、基础设施失败使该插件 `failed`，不影响其他插件 |
| 内存 / 栈 | 16MiB / 256KiB |
| 在途桥调用 | 16 |
| 桥调用预算 | 每激活 600 次/分钟 |
| 宿主调用截止 | 当前入口硬截止前 1 秒返回可捕获的 `timeout` |
| Promise | 所有宿主 Promise 必须 await/返回链；入口结束后不 drain |
| 调试 | 无 DevTools 逐步调试器、无 source map 栈 |

`PluginRuntime` 端口只包含 `activate`、`deliver_event`、`shutdown`；`deliver_event` 返回 `DeliveryOutcome { failed_handlers }`，Manager 据此决定是否触发 Resync。

## 9. 错误契约

11 个稳定错误码：

`permission_denied`、`invalid_input`、`context_expired`、`unauthenticated`、`not_found`、`provider_error`、`limit_exceeded`、`timeout`、`unavailable`、`unsupported`、`script_error`。

Wire 形状**恰好**为：

```json
{ "code": "permission_denied", "permission": "submission.read" }
```

- `permission` 只出现在 `permission_denied`，且必须是字符串权限名。
- 不返回 Rust 错误文本、栈、路径、请求/响应正文或原始 Provider 数据。
- 不可见资源与不存在资源通常都返回 `null`（而不是 `not_found`）；`not_found` 用于明确的资源缺失场景。
- 脚本语法/运行时异常统一为 `script_error`；超时仍为 `timeout`；资源分配失败为 `limit_exceeded`。
- IPC 开发命令也返回同一形状；前端 `IpcError.code` 保留 `code` 和 `permission`，只在恰好 `{code, permission?}` 时识别为插件错误。
- 前端未知 code、非法 `permission` 类型或原型键必须走通用错误兜底，不能覆盖真实 message。

## 10. 资源上限

**本表是插件宿主全部数值上限的唯一事实表**：其他文档叙述行为时只引用常量名，不重复数值；各源文件的模块文档（`doc/modules/`）允许记录**本文件自身**的常量值。常量集中在 [`plugin/limits.rs`](../../src-tauri/src/plugin/limits.rs) 及所属模块，数值由测试锁定；本表改动必须与代码同 PR。

| 项 | 上限 |
|---|---|
| 激活 / 单次事件 / 启动等待 / 停止 | 10s / 30s / 12s / 5s |
| Provider 单请求 / `submission.get` 整体 | 5s / 8s（最多 100 页） |
| 桥输入 / operation 名 / 桥响应（含包络） | 140000 B / 64 B / 1MiB |
| native 页数 / native 投影页 / 保留游标余项 / 游标数 / 句柄数 | 10000 / 1MiB / 4MiB / 32 / 32 |
| 句柄与游标 TTL / 句柄复用年龄 | 300s / 150s |
| 在途桥调用 / 每激活桥调用 / 单插件 handler | 16 / 600 次每分钟 / 64 |
| Provider 预算（每激活） | 60 次/分钟，并发 2 |
| Provider 预算（全部插件合计） | 120 次/分钟，并发 4 |
| 通知 | 512 B，10 次/分钟 |
| 日志 | 1024 B，30 次/分钟（仅对宿主实际写入的行计费，被级别过滤的调用不扣额度）；换行/制表写入前拍平 |
| 榜单 | page 1..100、每页 ≤200 行、单行 ≤64 列、单页总 cells ≤6,000（可组合边界）；每激活 6 次/分钟独立预算；Hydro 超页返回 `limit_exceeded` |
| 存储 | 键 256 B、值 64KiB、文件 1MiB；每激活 60 次/分钟，工厂合计 240 次/分钟，在途 16 |
| HTTP | 请求/响应体 64KiB、总超时 ≤5s、每插件 60 次/分钟、并发 2；Client 缓存 32 |
| 周期 Resync / 事件队列 | 默认 60s（10–3600s）/ 128 |
| 插件目录扫描 | ≤128 个目录、≤1024 个条目；`plugins.json` ≤128 项 |
| 状态历史 | 128 项 |
| 内存 / 栈 | 16MiB / 256KiB |

字节预算限制的是 JSON 表示，不等同于进程 RSS 上限。超限统一返回 `limit_exceeded`。

## 11. 存储契约

- 数据位于 `<base_dir>/plugin-data/<sha256(plugin-id)>/storage.json`。
- 键是数据字符串，不是文件路径；1..256 UTF-8 字节，无控制字符。
- `get` 返回 `{state:"missing"}` 或 `{state:"value", value}`；JSON `null` 与不存在严格区分。
- `set` / `delete` 需 `storage.write`；`compareAndSet` 同时需读写权限。
- 值 JSON 编码 ≤64KiB；完整文件编码 ≤1MiB。
- fs4 文件锁 + atomicwrites；读取→比较→原子持久化在锁内完成；CAS 只有原子写成功后才返回 `true`。
- 锁等待最多 5 秒，可被停止取消；阻塞文件操作在 `spawn_blocking` 中执行。
- 损坏/不可读/异常超大文件报错并保留原件，不静默重置；停止、禁用、重载、卸载代码都不删除数据。
- 停止前已开始的原子写可能完成，不能保证回滚。
- 本地 CAS 只保证单 Host 进程的 JSON 事务，不代替外部服务幂等。

## 12. HTTP 契约

- 需 `network.request`，且 URL 的精确 origin 同时出现在 Manifest 与 `plugins.json` 中。
- 只接受域名 origin；IPv4/IPv6 字面量（含公网 IP）在清单/审批阶段拒绝。
- 默认只允许公开单播地址上的 HTTPS；明文 HTTP 需 `allowHttp: true`；RFC1918/ULA 需 `privateCidrs` 批准。
- 全部 DNS 结果都检查，任一不合规即拒绝；合规地址固定到连接；TLS 仍校验证书与 hostname。
- 不使用共享 OJ 客户端、Cookie、系统代理、重定向或隐式重试；不注入 Hinina 凭证。
- 请求头黑名单：`host`、`cookie`、`connection`、`upgrade`、`transfer-encoding`、`content-length`、`content-type`、`proxy-*`。
- 请求/响应体各 ≤64KiB；总超时 ≤5 秒（含 DNS、连接、流读取）；每插件 60 次/分钟、并发 2。
- 3xx/5xx 作为响应返回，不自动重试；响应头只暴露 `content-type` / `retry-after`。
- 作用域变更/停止会取消在途等待；请求已到外部服务后不能撤销远端副作用，插件必须使用外部幂等键。

## 13. 通知契约

- 纯文本，按文本插值渲染，不执行 HTML/脚本。
- 单条 ≤512 UTF-8 字节；10 次/分钟；级别 `info` / `success` / `warn` / `error`。
- 前端最多同时显示 5 条，6 秒后自动过期；可手动关闭。
- 通知是易失的，不保证前端已显示；不能作为业务成功依据。

## 14. 生命周期与状态契约

- 状态只有 `starting` / `running` / `stopped` / `failed`。
- `list()` 返回 `PluginResult<Vec<PluginStatus>>`；缺失 `plugins.json` 表示没有配置插件；文件损坏或配置验证失败返回安全错误，不能伪装为空列表。
- `PluginStatus` 当前只有 `{id, state, error: {code, permission?} | null}`。
- 启动阶段失败保留 `failed` 与稳定错误码；成功重启后的状态覆盖旧诊断。
- 状态历史按最近写入保留最多 128 个 ID；列表读取时清除已从配置移除的历史记录。
- **`enabled` 只由 `plugins.json` 决定**：`stop` 是"本次停止"，不改变自动启用意图——重启应用后 enabled 插件仍会自动启动；持久禁用必须改 `plugins.json`。
- `stop` 先撤销权限与订阅，再等待 worker 释放；超时保持 `failed/timeout` 并由 reaper 回收。
- **`reload` 是开发期便利**（免于重启 `tauri dev`）：先停旧实例、确认退出、再按当前代码/配置/批准启动。**发布形态的语义是"改配置/改代码后重启应用生效"**。
- 旧 worker 未结束前拒绝同 ID 重启；迟到退出不能覆盖已锁定的失败诊断。
- 应用退出只做同步撤销，不等待插件，不干预工作区保存。

## 15. 前端 IPC 契约

| 命令 | 参数 | 返回 |
|---|---|---|
| `plugin_list` | 无 | `PluginStatus[]` |
| `plugin_start` | `{ pluginId }` | `void` |
| `plugin_stop` | `{ pluginId }` | `void` |
| `plugin_reload` | `{ pluginId }` | `void` |
| `plugin_resync` | `{ pluginId }` | `void` |

- 事件：`plugin-notification`，载荷 `{ pluginId, level, message }`。
- 命令不接收权限、代码、URL 或文件路径。
- 前端分层：View → Store → Service → Bridge；通知校验、5 项上限、6 秒过期、单一活动订阅/迟到订阅卸载都在 `plugin.service`；Store 只应用 add/dismiss。
- 四个命令在 release 构建中依然注册，但前端只在 DEV 暴露入口：`window.hininaPlugins` 仅 DEV 提供；release 按批准配置自动启动插件，无运行时管理入口。

## 16. 对内分层契约

- `core/`、`service/` 不依赖 plugin；唯一例外是组合根 `core/context.rs`。
- `plugin/` 非测试代码不引用 `crate::infra`；存储/HTTP/配置通过端口注入。
- `infra → plugin` 单向；业务 Service 不导入插件专用 Infra。
- 前端禁止跨层；Bridge 是唯一 `@tauri-apps/api` 入口。
- 安全依赖显式 `AccessRevision`，不依赖事件；转换期间 `snapshot()` 为 `None`。
- 日志只含稳定操作名、`plugin_id/version`、错误类别；禁止 URL、凭据、正文、路径、JS 栈、panic payload。唯一例外是 `operation="plugin.log"` 行：携带经校验（≤1024B、拒控制字符）、换行/制表拍平且 Debug 转义的插件 message，是日志中唯一允许的插件自由文本，仍禁止凭据等敏感内容。
- 文档与代码不一致视为缺陷；文件树/职责变更同步 `doc/Architecture.md` 与 `doc/modules/`。

## 17. 契约变更流程

1. 定义/修改 Rust DTO、trait、错误映射和资源上限。
2. 修改 `execution_api` 的 operation 名与参数形状。
3. 修改 `bootstrap.js` 的 JS 表面。
4. 修改 `hinina.d.ts`、Manifest 示例和模块文档。
5. 加契约测试：Rust 序列化/权限/边界 + 真实引擎 + 前端错误形状。
6. 破坏性变更递增 `manifestVersion` / `apiVersion` / 事件版本，并写明迁移方式；不做静默兼容。
7. 更新 [contracts.md](contracts.md) 与 [roadmap.md](roadmap.md)。

新增权限、API 方法或事件是**增量**变更，不递增 `manifestVersion` / `apiVersion` / 事件版本，旧插件继续可用；只有删除变体、改变字段语义或必填性才递增。

## 18. 尚未定义的契约

以下内容尚未形成稳定契约，继续开发前应先定：

- 管理读模型/写模型（inventory、失败阶段、配置修订、原子写入结果）
- 持久启用意图 vs 本次运行状态
- 发布物摘要、安装/更新/回滚、能力差异
- 配置 schema 子集与系统凭据引用
- Rust DTO ↔ 桥 ↔ TypeScript 的自动化契约测试
- 业务语义（首次 AC 定义、历史完整性、外部幂等键、重试/租期）
- handler 级独立超时
- 跨平台构建与 CI 门槛

详见 [roadmap.md](roadmap.md)。