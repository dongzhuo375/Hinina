# api/submission

## 职责
本人提交历史的安全投影与分页：只读、强制 only_mine、稳定 owner 归属校验。

## 核心类型/函数
- `MAX_SUBMISSION_PAGE_SIZE: u32 = 100` — list 的 limit 上界
- `PluginSubmission` — submissionId、contestId、problemId、problemDisplayId、status、submittedAt（UTC ms）
- `PluginSubmissionQuery` — contestId、problemId?、cursor?、limit
- `PluginSubmissionPage` — records、total（number|null）、nextCursor
- `SubmissionApi`（trait）— `get(context, contestId, submissionId)` / `list(context, query)`

## 直接依赖
- `plugin::api`（错误/上下文）
- `async_trait`、`serde`

## 被依赖
- `plugin::execution_api`（能力转发）、`plugin::query`（取数、游标与预算实现）

## 逻辑流程
### list：分页与游标
- limit 1..100；返回 `{records, total, nextCursor}`。**只有 nextCursor=null 才证明到达服务端可见历史的末尾**；不能因 records=[] 就停止——过滤后可能空页且仍有游标
- 归属：宿主强制 only_mine，并额外比对当前身份的稳定 owner ID（HOJ uid、Hydro rdoc.uid），不使用展示名或 udict 判定归属；缺失/不匹配拒绝。`SubmissionRecord.owner_id` 不参与序列化（skip_serializing），本接口只能消费 Provider 直接返回的记录——经 JSON 缓存往返的页面丢失归属证据、整页拒绝；HOJ 部署未返回 uid 同样 fail closed，不退化为按展示名信任
- 范围：只允许上下文当前比赛，problemId 为真实题号；投影不含 Token、owner ID、评测点、HTML 或原始响应，不调用源代码详情/快照路径
- 游标绑定 context、比赛、题目和 limit；5 分钟失效、最多 32 个，单个保留页最多 1000 条/1 MiB、最多 10 000 个 native page。合法领取即消费旧游标，失效和超限均报错；宿主保留 native page 剩余条目，不丢弃超过 limit 的数据。HOJ 用原生 total/pages；Hydro 总数为 null，继续到真实空 native page，不用前端旧假定页大小推断末尾
- **单次使用语义**：合法继续请求在锁内原子领取，其他并发复用收到 `context_expired`。可重试失败（预算耗尽、provider 错误、超时、响应超限、游标配额）把游标按调用前快照放回，重试从同一 native 页继续；只有 context_expired 不回填（作用域已撤销）。跨作用域或不同 query 参数的非法领取不消费游标

### get：本人历史扫描
- 在本人比赛历史中查找元数据，不访问含源码详情；最多 100 次 API 页，每个实际 native page 请求均消耗与比赛查询共用的 Provider 预算（单激活 60 次/分钟并发 2；全部插件共计 120 次/分钟并发 4，允许相应初始突发）。每次远端请求 5 秒超时；预算不足立即 `limit_exceeded`，不排队且不伪装为不存在。大量历史建议自行遍历 list 并维护索引
- 整个 get 扫描（含 scope 检查及全部页面）另有 8 秒总期限，达到时返回 `timeout` 并清理内部游标；**只有确实到达可见历史末尾才返回 null**。Runtime 还会在当前激活/事件硬截止前 1 秒结束未完成的宿调用，让插件能捕获 timeout 并在剩余窗口内结束处理

### 一致性边界
- 跨页没有事务快照：重判/新提交可能导致重复或移位，按 submissionId 去重、必要时重扫；用户不可见的历史不能变成全站真相
- **首次 AC 必须由插件自行确认**身份、比赛、完整可见历史及稳定排序（同时间用 ID 作第二键）；事件 Accepted 本身不构成首次 AC 证据
