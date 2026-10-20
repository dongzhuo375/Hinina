# plugin/query

ScopedQueries是绑定PluginGrant的安全查询Adapter。QuerySource只提供scope、只读比赛和本人native历史，生产ServiceQueries调用最小Service读路径；不调用Command或含源码详情。

句柄TTL5分钟、最多32；异步前后及取消时检查有效scope，防止跨用户/OJ/比赛或ABA返回。游标最多32个，绑定context和query，保留native页余项；完整性/总数/错误的语义见[提交API](api/submission.md)。

已有句柄复核时，`scope()` 读取失败统一返回 `ContextExpired`，JS 收到 `context_expired`；首次获取上下文失败仍保留来源错误。取消令牌检查放在 scope 读取之后，防止读取期间发生作用域切换却返回已撤销的令牌。
