# submission：本人提交安全投影

`list(context, {contestId, problemId?, cursor?, limit})`：limit 为 1..100。返回 `{records, total: number|null, nextCursor: string|null}`。只有 nextCursor=null 才证明这次分页已到服务端可见历史的末尾；不能因为 records=[] 就停止，过滤后可能空页且仍有游标。

宿主强制 only_mine，并额外比对当前身份的用户名。只允许上下文当前比赛，problemId 为真实题号；宿主在安全元数据上过滤。投影只有 submissionId、contestId、problemId、problemDisplayId、status、submittedAt（UTC 毫秒）。不调用源代码详情/快照路径，不包含 Token、评测点、HTML 或原始响应。

游标绑定 context、比赛、题目和 limit；5 分钟失效，最多 32 个，单个保留页最多 1000 条/1MiB，最多 10000 个 native page。成功继续后消费旧游标；失效和超限均报错。宿主保留 native page 剩余条目，不丢弃超过 limit 的数据。HOJ 使用原生 total/pages；Hydro 总数为 null，继续到真实空 native page，不使用前端旧的假定页大小推断末尾。

`get(context, contestId, submissionId)` 在本人比赛历史中查找元数据，不访问含源码详情；最多扫描 100 次 API 页，且每个实际 native page 请求均消耗与比赛查询共用的 Provider 预算（单激活60次/分钟、并发2；全部插件共计120次/分钟、并发4，允许相应初始突发）。每次远端请求5秒超时，预算不足立即报 limit_exceeded，不排队且不伪装为不存在。大量历史建议自行遍历 list 并维护索引。

跨页没有事务快照，重判/新提交可能导致重复或移位；按 submissionId 去重，必要时重扫。用户不可见的历史不能变成全站真相。首次 AC 必须由插件自行确认身份、比赛、完整可见历史及稳定排序（同时间用 ID 作第二键）；事件 Accepted 本身不构成首次 AC 证据。
