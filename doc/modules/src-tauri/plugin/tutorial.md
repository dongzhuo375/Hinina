# 插件开发教程（接口预留）

当前没有 JS 加载器、可安装插件或可执行气球示例。以下是未来实现时的接口用法和业务设计注意事项，不可复制后宣称已能运行。

## 目录与入口

未来插件目录约定 plugins/<id>/manifest.json + index.js；宿主只读取 JS，TypeScript 需要在插件项目离线构建。示例清单见 [manifest.example.json](sdk/manifest.example.json)。清单只能声明能力，宿主还需明确授予。

入口采用 ES module 默认导出的 activate；不提供 Node 全局或任意 import。下面只展示注册形式，不实现气球判断或网络派发：

```js
/**
 * @plugin-entry
 * @plugin-id com.example.balloon-first-ac
 * @runtime js
 * @api-version 1
 */
export default async function activate(hinina) {
  /**
   * @plugin-handler
   * @event submission.judged
   * @idempotent true
   * @resync-safe true
   */
  const stop = hinina.events.on('submission.judged', async (envelope) => {
    // 这是刷新提醒；先签发新上下文、查询本人当前比赛，再决定是否扫描完整历史。
    // 不把 envelope.event.status === 'Accepted' 等同于首次 AC。
  })
  // stop() 仅注销这个 handler；整个插件停止时 Runtime 自动清理全部 handler。
}
```

查询示意（TypeScript，只有接口声明）：

```ts
const context = await hinina.currentContext()
const contest = await hinina.contest.current(context)
const identity = await hinina.identity.current(context)
if (contest && identity) {
  const page = await hinina.submission.list(context, {
    contestId: contest.contestId, page: 1, pageSize: 50
  })
  // page.hasMore 表示还有下一页；此页不能证明首次 AC 或历史完整。
}
```

事件 handler 收到的是带 version/sequence 的信封，不是扁平 CoreEvent。监听 resync.required 时同样查询当前上下文；不要等待宿主重放。context_expired 后丢弃旧流程并重新获取上下文。

## 首次 AC 业务应由插件决定

需验证当前用户/比赛/题目归属，取相关题目的完整本人历史，处理分页、OJ 隐藏历史及重判。以 submittedAt（毫秒）排序，再按 submissionId 字符串序稳定打破同秒并列；这只是客户端可确定的顺序，若服务端无法证明真实先后而业务要求严格首条，则保守不派发。

未知历史或并发分页变化时不派发，先重新扫描并确认。事件可能重复；同题后续 AC、重复通知、启动 Resync 都不能产生第二次派发。当前提交事件没有 OJ/用户/比赛作用域，因此按其 ID 直接查询并派发不安全，应以当前上下文完整扫描结果为准。

派发幂等键绑定 ojId/userKey/contestId/problemId；使用规范 JSON 元组或长度前缀编码，不能简单用冒号拼接导致不同字段组合冲突。私有 storage 只负责本地记账；外部服务必须按同一幂等键去重。

## 失败、恢复与重试

参考方案的 CAS(key, null, ...) 存在两个问题：JSON null 与键缺失不明确，且写入 dispatch_failed 后再也不等于 null。本契约用 {state:'missing'} 与 {state:'value',value:...} 明确区别。重试需读取现有状态，CAS 期望该状态/修订号，而不是永远期待 Missing。

未来业务应有有限 dispatching 租约、dispatched 成功标记和带重试时间的失败状态。重启后检查过期租约；网络超时代表结果未知，不能立即换幂等键。外部服务成功但本地成功标记未落盘时，使用原幂等键恢复。500/超时采用有限退避及可人工恢复策略，不无限重试。已经被 server 判定重复的同一幂等请求仍应按成功收敛。

网络调用函数的标注应包括：

```js
/**
 * @plugin-network
 * @permission network.request
 * @https-only true
 * @allowlist manifest.network.allowHosts
 * @idempotent true
 */
```

不使用任意 fetch；未来只经 hinina.http.request(context, request)，Host 执行权限、HTTPS/DNS/白名单及体积/超时限制。失败通知使用纯文本，不向选手输出 token、内部路径或完整错误堆栈。

## 后续业务验收

外部服务 mock 应覆盖首 AC、非 AC、他人/其他比赛、相同事件重复、同题后续 AC、不是第一条、缺页/历史不可见、相同时间戳、重判、重启、Resync、500 重试、结果未知的超时、本地标记落盘失败、并发实例与幂等键冲突。

这些用例依赖尚未实现的存储/HTTP/Runtime，不是本轮已通过测试。
