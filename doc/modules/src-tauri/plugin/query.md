# plugin/query

ScopedQueries是绑定PluginGrant的安全查询Adapter。QuerySource只提供scope、只读比赛和本人native历史，生产ServiceQueries调用最小Service读路径；不调用Command或含源码详情。

已有OJ实例修改baseUrl沿用设置页“重启后生效”的约定。scope绑定Registry实际运行的endpoint，不能拿待生效地址标记旧Provider；配置变更撤销旧句柄，新句柄仍可查询，身份键也使用实际endpoint。配置中实例被删除/禁用仍拒绝访问。缺endpoint、实例不可用及地址待重启均输出稳定类别的warn，不记录URL或凭据；同一AccessRevision内每个插件只记录一次（去重键为 revision+plugin_id，修订变更即清空），并带plugin_id/oj_id归属，既避免持久性配置错误被插件反复刷屏，也保证归因指向真正失败的插件而非该修订下首个撞上者。成功scope按revision缓存，正常热路径不重复输出待重启诊断，阻塞读取继承调用方tracing dispatch。

submission.get整体扫描最多8秒（含scope检查），同时保留100次API页和逐native页预算；超时返回可捕获的timeout，不误报null。内部continuation及list尚未发布的continuation由RAII清理，超时、取消或future被丢弃时不会泄漏游标。

分页失败不消耗调用方的游标：list 先取出游标再抓取，`advance` 先在局部变量里完成抓取、owner/条数校验与 1MiB 编码尺寸校验，全部通过后才一次性写回游标，因此任何失败都让游标停留在真正的调用前快照。若抓取/校验失败（预算耗尽、provider 错误、瞬时 IO、超限页），把该快照放回并返回原错误，重试从同一 native 页继续；回填的是已发布过的游标，不绕过 32 游标/4MiB 聚合上限，也不会把超限页连同陈旧 buffered_bytes 一起驻留；只有 context_expired 不回填，因为该作用域已撤销。

句柄TTL5分钟、最多32；异步前后及取消时检查有效scope，防止跨用户/OJ/比赛或ABA返回。游标最多32个，绑定context和query，保留native页余项；完整性/总数/错误的语义见[提交API](api/submission.md)。

单native页的安全投影JSON最多1MiB，所有保留游标余项JSON合计最多4MiB；编码流式计数，超限为limit_exceeded。返回给JS的整页（含nextCursor）在游标发布前先按桥包络预留空间校验，确保“游标已入库但响应无法编码”不会产生插件永远拿不到的孤儿游标；未通过校验时直接返回limit_exceeded且不占用游标配额。领取游标移动所有权，消费后释放余项预算，不克隆整页。失败或过期的游标仍按单次使用语义处理。

已有句柄复核时，`scope()` 读取失败统一返回 `ContextExpired`，JS 收到 `context_expired`；首次获取上下文失败仍保留来源错误。取消令牌检查放在 scope 读取之后，防止读取期间发生作用域切换却返回已撤销的令牌。

`check` 为异步接口：scope 快照先走无 I/O 的缓存探测（`QuerySource::cached_scope`），命中直接返回；未命中才通过 `spawn_blocking` 读取会话文件。快路径与慢路径共用同一 `scope_cache_hit` 谓词（revision 相等且缓存与当前快照的取消令牌均未撤销），单一事实来源使两条路径不可能漂移，撤销令牌在切 OJ/换账号/换比赛后不会从缓存命中。初次上下文、查询前后校验和 HTTP 校验均不在 Tokio 执行线程同步读盘，同时避免热路径上每次 check 都占用阻塞池线程。读取后仍验证取消令牌与插件停止状态。

生产 ServiceQueries 按 AccessRevision 缓存一个 scope；同一修订反复 check 不再读取会话文件。缓存命中只做一次克隆，会话/配置读取不在锁内执行，多个插件的 scope 计算不会因 I/O 互相串行。显式身份/OJ/比赛/配置变更撤销缓存，在途结果仍由取消栅栏拒绝；不把外部直接修改会话文件视为受支持的实时状态变更。current_context 在切换中与 check 一致返回 context_expired，可在下次 Resync 重试。提交页按 owner_id 而非展示用户名校验；游标在锁内一次性领取。

OJ 读取失败（contest.get / submission.history）在压缩为 provider_error 之前记录脱敏诊断：只含稳定操作名、当前 OJ 标识与 AppError 变体类别（network/serialization/auth/provider_not_found 等），不输出可能含 URL、凭据或响应正文的原始消息；scope 的会话身份读取失败同样以 scope.identity + 类别记录。插件侧仍只收到 provider_error / unavailable。

revision递增使用wrapping_add，缓存同时保留AccessSnapshot的取消令牌；即使数值回绕也不能命中已撤销缓存。回归测试阻塞旧会话读取，让新修订先完成，再释放旧读取，验证不串行阻塞、不接受过期结果。

比赛/历史查询共用每次激活 60 次/分钟、并发 2 的 Provider 预算；Manager 再统一限制所有插件合计 120 次/分钟、并发 4，重载或更换上下文不会重置共享预算。governor 令牌桶允许初始突发 60/120 次，并按对应速率补充。每次实际 Provider 调用（含 `submission.get` 的每一 native page）单独计费；读取已保留的页余项和身份不消耗远端预算。超限立即返回 `limit_exceeded`，不等待排队；每次 Provider 调用最多 5 秒，超时返回 `timeout`，完成/错误/取消均释放并发名额。分页查找失败会清理内部游标，不能把预算耗尽误报为不存在。
