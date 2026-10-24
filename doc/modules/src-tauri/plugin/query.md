# plugin/query

## 职责
`ScopedQueries`——绑定 PluginGrant 的安全查询 Adapter：作用域句柄签发与复核、只读比赛/题目/榜单/本人提交历史的取数与投影、游标分页、Provider 速率与并发预算。JS 的一切数据读取都经过这里。

## 核心类型/函数
- **`QueryScope`** — 解析后的作用域快照：`oj`、`endpoint`、`user: Option<(String, String)>`、`contest: Option<String>`——**不含撤销栅栏**；取消令牌在内部 `Context`（scope/revision/cancellation/created）里，由 `check()` 返回
- **`QuerySource`**（trait）— 只提供 scope、只读比赛、题目列表、全场榜单和本人 native 历史；不调用 Command、不含源码详情
- **`OjInstanceConfig`**（trait）— 连接配置抽象（endpoint 与待重启语义）
- **`ServiceQueries`** — 生产 `QuerySource`：调用最小 Service 读路径；按 AccessRevision 缓存一个 scope
- **`ScopedQueries`** — 每次激活一个实例：
  - `new(grant, source, access, stopped)` / `with_shared_budget`
  - `require(permission)` — 权限检查（停止即 context_expired）
  - `check(handle) -> CancellationToken` — 句柄复核：TTL/revision/scope/取消四重校验，返回撤销栅栏
  - `current_context()` — 签发/复用句柄（同 scope+revision 150 秒内复用）
  - `identity` / `contest` / `problems` / `rank` / `list` / `submission` — 六个只读查询；`contest(handle, id)` 的可选 id 参数同时覆盖 contest.current（无 id）与 contest.get（有 id），没有独立的 get_contest 方法
  - （私有）`remote` — 预算获取 + 超时 + 前后校验的取数包装

## 直接依赖
- `plugin::api`（DTO/错误/权限）、`plugin::limits`（预算与句柄常量）
- `core::access::AccessRevision`、`core::provider::...`（经 QuerySource 抽象）
- `governor`（令牌桶）、`sha2`（`user_key` 身份/队伍键哈希）、`tokio_util::sync::CancellationToken`

## 被依赖
- `plugin::execution_api`（能力转发的取数层）
- `plugin::manager`（共享预算的宿主方）

## 逻辑流程
### 作用域与句柄
- 句柄 TTL 5 分钟、最多同时 32 个；同 scope+revision 下 150 秒内的未过期句柄由 `current_context` 复用（返回同一句柄），超龄或 scope/revision 变化才新签发——"每个 handler 重新获取 context"不会耗尽上限。异步前后及取消时检查有效 scope，防止跨用户/OJ/比赛或 ABA 返回
- 已有句柄复核时 `scope()` 读取失败统一返回 `context_expired`；首次获取上下文失败保留来源错误。取消令牌检查放在 scope 读取**之后**，防止读取期间发生作用域切换却返回已撤销的令牌
- `check` 先走无 I/O 的缓存探测（`QuerySource::cached_scope`），未命中才 `spawn_blocking` 读会话文件；快慢路径共用同一 `scope_cache_hit` 谓词（revision 相等且两侧取消令牌均未撤销），单一事实来源使撤销令牌在切 OJ/换账号/换比赛后不可能从缓存命中
- 生产 ServiceQueries 按 AccessRevision 缓存 scope：同一修订反复 check 不再读盘，会话/配置读取不在锁内执行；显式身份/OJ/比赛/配置变更撤销缓存，在途结果由取消栅栏拒绝；revision 递增用 `wrapping_add` 且缓存同时保留取消令牌，数值回绕也不能命中已撤销缓存
- 配置语义：OJ 实例改 baseUrl 沿用"重启后生效"——scope 绑定 Registry **实际运行**的 endpoint，配置变更撤销旧句柄、新句柄仍用实际 endpoint；实例被删除/禁用仍拒绝。缺 endpoint/实例不可用/地址待重启输出稳定类别 warn（去重键 revision+plugin_id，修订变更即清空），不记录 URL 或凭据

### 题目列表与榜单
- problems 只需 `contest.read`；rank 需 `contest.rank` ∩ `contest.read`（先查 contest.rank，错误指认第一个缺失项）。两者都要求上下文绑定的当前比赛（未选中返回 `context_expired`），经 remote 前后校验并按 Provider 预算计费
- rank 只调 `ContestService::get_rank` 取榜单数据；另读一次比赛元信息**仅用于按 `rank_show_name` 解析 displayName**（与前端 resolveDisplayName 同规则，保证插件榜与 App 榜一致；sealed/contestType 仍只来自 contest.current）——元信息读取同样走 remote 计费，**每次 contest.rank 共消耗 2 份通用 Provider 预算**
- 行投影用与 identity.userKey 同算法的 `user_key(oj, endpoint, uid)` 生成 teamKey；行 uid 为空 → 整页 `provider_error`（空 uid 会让此类行塌缩到同一 teamKey）
- 页边界可组合：page ≤100、行 ≤200、列 ≤64、**单页总 cells ≤6 000**，超限返回 `limit_exceeded` 而非被 1 MiB 桥上限截断；page 回显服务端实际页（Hydro 恒 1）
- 榜单独立预算 6 次/分钟：令牌在元信息校验通过后、榜单取数前扣——元信息失败不烧令牌，取数后的边界拒绝仍计费（OJ 已完成全榜重算，预算保护的是 OJ 而非插件调试体验）
- contest 投影含 contestType/sealed/sealedSince，只读 read_metadata 已有返回，不加缓存、不改时效、不新增取数路径（不调 get_rank）

### 分页与预算
- **分页失败不消耗调用方的游标**：list 先取出游标再抓取；`advance` 在局部变量完成抓取、owner/条数校验与 1 MiB 编码尺寸校验，全部通过才一次性写回。任何失败都让游标停留在调用前快照并返回原错误，重试从同一 native 页继续；只有 `context_expired` 不回填（作用域已撤销）。抓取成功后的失败（publish 超限/游标配额）同样回填；不绕过 32 游标/4 MiB 聚合上限
- 单 native 页投影 JSON ≤1 MiB、保留游标余项合计 ≤4 MiB（流式计数）；返回给 JS 的整页在游标发布前先按桥包络预留空间校验，杜绝"游标已入库但响应无法编码"的孤儿游标；领取移动所有权，消费后释放余项预算，失败/过期游标仍按单次使用语义处理
- `submission.get` 整体扫描最多 8 秒（含 scope 检查），保留 100 次 API 页与逐 native 页预算；超时返回可捕获的 `timeout`，不误报 null。内部 continuation 及 list 未发布的 continuation 由 RAII 清理，超时/取消/future 被丢弃都不泄漏游标
- **预算**：每激活 60 次/分钟并发 2；Manager 再限全部插件合计 120 次/分钟并发 4（跨重载保留）。两项预算都先取并发许可再扣速率令牌，并发拒绝不扣令牌；令牌不可退还，共享桶耗尽只损耗该激活自己的令牌。每次实际 Provider 调用（含 get 的每一 native page）单独计费，读取已保留页余项和身份不计费；单请求最多 5 秒，超限立即 `limit_exceeded` 不排队；分页查找失败清理内部游标，不能把预算耗尽误报为不存在
- **脱敏诊断**：OJ 读取失败（contest.get / submission.history）压缩为 provider_error 前记录脱敏诊断——只含稳定操作名、OJ 标识与 AppError 变体类别，不输出 URL/凭据/响应正文；插件侧只收到 provider_error / unavailable
