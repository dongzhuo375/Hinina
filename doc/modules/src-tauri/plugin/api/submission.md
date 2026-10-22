# submission：本人提交安全投影

`list(context, {contestId, problemId?, cursor?, limit})`：limit 为 1..100。返回 `{records, total: number|null, nextCursor: string|null}`。只有 nextCursor=null 才证明这次分页已到服务端可见历史的末尾；不能因为 records=[] 就停止，过滤后可能空页且仍有游标。

宿主强制 only_mine，并额外比对当前身份的稳定 owner ID（HOJ uid、Hydro rdoc.uid），不使用展示名或 udict 判定归属。缺失/不匹配的 owner ID 拒绝。只允许上下文当前比赛，problemId 为真实题号；宿主在安全元数据上过滤。投影只有 submissionId、contestId、problemId、problemDisplayId、status、submittedAt（UTC 毫秒）。不调用源代码详情/快照路径，不包含 Token、owner ID、评测点、HTML 或原始响应。

游标绑定 context、比赛、题目和 limit；5 分钟失效，最多 32 个，单个保留页最多 1000 条/1MiB，最多 10000 个 native page。合法领取即消费旧游标；失效和超限均报错。宿主保留 native page 剩余条目，不丢弃超过 limit 的数据。HOJ 使用原生 total/pages；Hydro 总数为 null，继续到真实空 native page，不使用前端旧的假定页大小推断末尾。

游标是单次使用：合法继续请求在锁内原子领取，其他并发复用收到 context_expired；领取后的远端失败也不能再次复用同一游标，应重新查询。跨作用域或不同 query 参数的非法领取不会消费游标。

`SubmissionRecord.owner_id` 不参与序列化（`skip_serializing`），因此本接口只能消费 Provider 直接返回的记录；任何经 JSON 缓存往返的页面都会丢失归属证据并被整页拒绝。HOJ 部署若未返回 `uid` 同样如此（fail closed，不会退化为按展示名信任）。

`get(context, contestId, submissionId)` 在本人比赛历史中查找元数据，不访问含源码详情；最多扫描 100 次 API 页，且每个实际 native page 请求均消耗与比赛查询共用的 Provider 预算（单激活60次/分钟、并发2；全部插件共计120次/分钟、并发4，允许相应初始突发）。每次远端请求5秒超时，预算不足立即报 limit_exceeded，不排队且不伪装为不存在。大量历史建议自行遍历 list 并维护索引。

整个get扫描（含scope检查及全部页面）另有8秒总期限（单页最坏5秒之上留出余量），达到时返回timeout并清理内部游标；只有确实到达可见历史末尾才返回null。Runtime还会在当前激活/事件硬截止前1秒结束未完成的宿主调用，让插件能捕获timeout并在剩余窗口内结束处理。大量历史或慢OJ优先使用list分批保存进度，避免单次get反复全量扫描。

跨页没有事务快照，重判/新提交可能导致重复或移位；按 submissionId 去重，必要时重扫。用户不可见的历史不能变成全站真相。首次 AC 必须由插件自行确认身份、比赛、完整可见历史及稳定排序（同时间用 ID 作第二键）；事件 Accepted 本身不构成首次 AC 证据。
