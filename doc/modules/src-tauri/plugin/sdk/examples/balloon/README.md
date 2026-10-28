# 首次 AC 气球参考插件（阶段 C）

插件系统阶段 C 的参考业务插件：证明宿主能力组合（日志 + 作用域查询 +
题目列表/榜单 + 私有存储 CAS + 受限 HTTP）能完成"哪支队在哪道题上首 A →
派发气球"的目标工作流。**业务规则全部留在插件**，宿主不知道"气球"是什么。
它是文档的一部分，不进入 `src-tauri/`，不构建进应用。

## 行为

每次 `resync.required`（运行时无 `setTimeout`，节拍只能来自 Resync）：

1. `context.current` → `contest.current`；未选比赛 → 本轮安静结束；
2. **先看 `sealed`**：封榜期间整轮直接结束，不读榜单、不派发——封榜时
   `solved`/`firstAc` 恒 false，与真没通过不可区分；
3. `contest.problems`（题号 ↔ 气球颜色）→ `contest.rank` 逐页拉取（配额
   见下；每页行数按题数自适应——`min(RANK_MAX_ROWS, RANK_MAX_CELLS ÷ (题数+1))`——
   保证页在宿主 `MAX_RANK_ROWS` 行 / `MAX_RANK_CELLS` 格上限内可编码），
   跨页按 `teamKey` 去重，打星队（`rank === -1`）跳过；页游标跨轮推进、
   榜末回绕，同一页被**连续 `SKIP_AFTER_WINDOWS` 个窗口**以 `limit_exceeded`
   拒绝后才跳过（短暂配额抖动不丢页，游标也不会被钉死）；榜单列不在题目
   列表里时跳过该格并告警（没有题号/颜色的气球对发球无用），题目列表
   追上后自动补派；
4. `firstAc === true` 且未派发过 → 先写 `pending`（CAS）→ `http.request`
   POST 派发（带幂等键，不带 `content-type`——宿主黑名单拒绝该头，并为
   带体请求自行设置 `application/json`）→ **每个成功立即落盘 `sent`**
   （handler 硬截止 `EVENT_TIMEOUT` 随时可能杀掉本轮，攒在内存里的
   `sent` 标记会丢，导致已送达的气球永远停留在 `pending` 重发）；
5. 派发队列遇 `context_expired`/`limit_exceeded`/`timeout` 或配置类错误
   （`permission_denied`/`invalid_input`）立即停止，剩余保留 `pending`，
   下次 Resync 重试；榜单配额耗尽只结束翻页，**不中止本轮**——已取到的
   页照常认领、派发；
6. 全部日志经 `note()` 包装输出（日志出口限流 `PLUGIN_LOGS_PER_MINUTE`
   不会中断轮次）。

### 关键规则

| 规则 | 实现 |
|---|---|
| 不补派封榜期间的历史首 A（限插件观测到的窗口） | 存储记忆封榜窗口 `[sealedSince, revealedAt)`；AC 时间落在窗口内的首 A 永久跳过，`revealedAt` 取解封后首轮的事件 `envelope.occurredAt`（插件无时钟）。窗口只能靠观测封榜期间学得：插件在解封后才启动/重载时没有封榜记忆（宿主解封后不再下发 `sealedSince`），无从区分"封榜窗口内的首 A"与普通冷启动——此时按冷启动处理，并写一条 `no seal memory` 日志披露 |
| 幂等 | 本地：`pending` → CAS → 派发 → 每个成功立即 `sent` 落盘；远端：POST 体携带 `idempotencyKey = contestId:teamKey:displayId`，接收方按它去重。CAS 只保证本地单进程事务 |
| 跨比赛隔离 | 存储键带 `contestId`（`balloon.records:<contestId>`），换比赛不会因同队同题号碰撞漏派 |
| 启动即校验能力 | activate 用无副作用的探针校验 `contest.rank`/`contest.read`/storage/`network.request` 权限（哑上下文 + 不可能命中的 CAS）；有查询上下文时再对端点做一次 GET 探测——origin 审批与 URL 形状校验只存在于宿主 send 路径，哑上下文探测够不到，而 `plugins.json network` 与清单 `network.allowOrigins` 两处手工配置恰是最易配错的地方。缺权限/端点被拒时插件直接 `failed`：**宿主错误码一律 `script_error`**（宿主不读取插件抛出的错误码），具体原因由探针写进宿主日志（`balloon: activation failed …`）。无上下文（未登录启动）或探测网络失败时，校验推迟到首次派发 |
| 配置类错误不无限重试 | 派发遇 `permission_denied`（端点 origin 未同时进入清单 `network.allowOrigins` 与 `plugins.json network`）或 `invalid_input`（URL/请求形状被宿主拒绝）视为死区：写 error 日志、停住队列、剩余保留 `pending`，而不是每轮重发同一注定失败的请求 |
| 冷启动 | 参考插件不区分"插件启动前/后"的首 A：首次运行会把当前榜单上所有可见首 A 视为待派发。需要该语义的作者可自行记录 firstSeen |

## 构建

仓库根目录（Node.js ≥22.14，即 package.json `engines` 的要求；`plugin:scaffold:build` 脚本自身另要求 ≥20.6 以使用 `import.meta.resolve`）：

```powershell
npm run plugin:sdk:check      # 类型检查（含本示例）
npm run plugin:balloon:build  # esbuild 离线打包 → src-tauri/target/plugin-balloon.js
```

把产物复制到插件目录并命名为 `index.js`（与 `manifest.entry` 一致）。

## 安装与批准

```text
<数据目录>/
  plugins.json
  plugins/com.example.balloon/
    manifest.json     # 由 manifest.example.json 复制，改 network.allowOrigins 为实际派发端点
    index.js
```

`plugins.json`（`options.endpoint` 是派发目标，必须是 http(s) URL；明文
HTTP origin 需在 `network` 审批里显式加 `"allowHttp": true`，RFC1918/ULA
私网目标需 `privateCidrs` 批准，loopback 地址无审批路径一律拒绝；URL 不能
带 userinfo 或 fragment，宿主一律拒绝）：

```json
{
  "plugins": {
    "com.example.balloon": {
      "enabled": true,
      "permissions": [
        "events.subscribe", "contest.read", "contest.rank",
        "storage.read", "storage.write", "network.request"
      ],
      "network": [{ "origin": "https://httpbin.org" }],
      "resyncIntervalSecs": 60,
      "options": { "endpoint": "https://httpbin.org/post" }
    }
  }
}
```

赛场内网的明文 HTTP 派发后端（如局域网接收端）把 `network` 与 `options`
换成（清单 `network.allowOrigins` 同步改）：

```json
"network": [{ "origin": "http://balloon.internal:8080", "allowHttp": true, "privateCidrs": ["10.10.0.0/16"] }],
"options": { "endpoint": "http://balloon.internal:8080/post" }
```

公网 HTTPS 端点两项额外批准都不需要。明文 HTTP 仅建议用于可信内网——气球
体（队伍名/校名）经明文传输，配置前自行评估。

- 榜单查询有**每激活 `PLUGIN_RANK_QUERIES_PER_MINUTE` 次/分钟的硬配额**（宿主
  实现为 governor GCRA：突发等于配额、按每分钟配额个逐个回补）。插件本地
  以**相同参数的 GCRA** 记账（模块态、随激活重置；时钟取事件信封时间——
  它就是适配时刻的宿主墙钟），与宿主限流器的等价由构造保证；预算跨轮
  连续回补、共享使用（无每轮上限），不足一个整令牌的轮次一页不取、
  安静跳过（日志含 `rank budget exhausted`）。本地记账看不到的偏差
  （共享 provider 预算、信封入队到处理之间的排队延迟等）表现为瞬时拒绝，
  由重试路径吸收——同一页连续 `SKIP_AFTER_WINDOWS` 个窗口（60 秒计窗）
  被拒才会跳页。
- 激活时（已登录、可取得查询上下文）插件会对端点**真实发送 1 次 GET**
  探测，用于提前暴露 origin 审批/URL 配置错误——按请求计费或无状态的
  webhook 端点请知悉：每次启动/重载都有这一次出站请求。
- 改 `plugins.json` / 清单 / 代码后重启应用生效（`reload` 是开发期便利）。

## 端到端验收（需真实环境，人工执行）

> **约束**：插件 HTTP 一律拒绝 loopback 地址，**不能用本地 mock server**；
> 私网目标需 `privateCidrs` 显式批准后可用内网 HTTP(S) 端点。推荐用公网
> HTTPS echo 端点（如 httpbin.org）接收假数据，验收完即关；
> 验收数据一律使用假队伍名，不涉及真实选手。

| # | 场景 | 操作 | 预期 |
|---|---|---|---|
| 1 | 自动启动 | `enabled=true` 后重启应用 | 插件自动启动，`window.hininaPlugins.list()`（DEV）显示 running |
| 2 | 权限未批准 | 清单声明 `contest.rank` 但 `plugins.json` 未批准，重启 | 状态 `failed`（宿主错误码 `script_error`）；`logs/hinina.log` 出现 `balloon: activation failed at contest.rank probe: permission_denied{contest.rank}` 指认缺失项 |
| 3 | 端点 origin 未批准 | 已登录状态下从 `plugins.json network` 移除端点 origin，重启 | 激活即失败，日志指认 `endpoint probe (permission_denied)`；若启动时无查询上下文，则首轮派发写 error 并停队列（不无限重试） |
| 4 | 跨页去重 | 榜单 ≥2 页的比赛 | 每个首 A 只派发一次，无漏派；日志出现 `dispatched N` |
| 5 | 失败重试 | 断网或端点返回 5xx | 该气球保留 `pending`，下次 Resync 重试；`hinina.log.warn` 可查 |
| 6 | 封榜 | 进入封榜窗口 | 不派发；`logs/hinina.log` 出现 `sealed` |
| 7 | 解封后启动 | 封榜解封后才启动/重载插件 | 日志出现 `no seal memory`；可见首 A（含封榜期间隐藏的）按冷启动全部派发 |
| 8 | 榜单配额 | 多页榜单 + `resyncIntervalSecs` 10 | 部分轮次日志 `rank budget exhausted`、安静跳过；已取到的页照常派发，无 `round aborted` |
| 9 | 作用域失效 | 切 OJ / 登出 / 换比赛 | 旧句柄 `context_expired`，本轮停止，不误派 |
| 10 | 退出 | 关闭应用 | 退出不阻塞；下次启动无残留 worker |

日志位置：`logs/hinina.log`（设置页可查数据目录路径）。

## 已知边界

> 本文不重复宿主数值：宿主上限一律用**宿主常量名**（定义于
> `src-tauri/src/plugin/limits.rs` 与 `infra/plugin_storage.rs`，数值见
> [contracts.md §10](../../../../../../plugin/contracts.md)），插件侧逻辑用
> `index.ts` 顶部的本地别名。两套名字的镜像关系由契约测试锁定
> （`__tests__/balloon.spec.ts` 解析两侧源码逐对比对），宿主数值另有
> Rust 侧测试与 [contracts.md](../../../../../../plugin/contracts.md) 守住。
>
> | 宿主常量（limits.rs / plugin_storage.rs） | 插件别名（index.ts） |
> |---|---|
> | `MAX_RANK_ROWS` | `RANK_MAX_ROWS` |
> | `MAX_RANK_CELLS` | `RANK_MAX_CELLS` |
> | `PLUGIN_RANK_QUERIES_PER_MINUTE` | `RANK_QUERIES_PER_MINUTE` |
> | `MAX_RANK_PAGE` | `MAX_RANK_PAGES` |

- 榜单分页按 OJ 而异：**HOJ** 按请求分页，页游标以每分钟至多
  `PLUGIN_RANK_QUERIES_PER_MINUTE` 页的速度跨轮推进、榜末回绕，全榜轮转
  周期约为「榜单页数 ÷ 配额」分钟——深榜单不会漏扫，只是新首 A 的检测
  延迟受该周期限制；页数不超过配额的榜单每分钟至少完整扫一遍；轮转
  中途加题会改变每页行数、页号与队伍错位，部分队伍要等下一轮转才被
  扫到。**Hydro** 不支持分页：整榜单页返回、忽略请求的 limit 与页码，
  每轮 1 次查询即拿全榜（窗口配额大部分闲置），且单页须同时满足宿主
  `MAX_RANK_ROWS` 行与 `MAX_RANK_CELLS` 格两个硬限、均不受插件请求
  调节（题数 × 队数超过 `MAX_RANK_CELLS` 即超）——超限的 Hydro 比赛
  每次查询都被拒，插件无法派发（日志按跳页报告）。契约页数上限
  `MAX_RANK_PAGE`，可覆盖行数 = `MAX_RANK_PAGE` × 每页自适应行数
  （随题数增多而减少）。
- 页被连续 **`SKIP_AFTER_WINDOWS` 个窗口**（60 秒计窗）以 `limit_exceeded`
  拒绝后才跳过：一两个窗口的配额/共享预算抖动不会丢页；真正的超限页
  （超过 `MAX_RANK_ROWS` 行或 `MAX_RANK_CELLS` 格）与持续的预算干旱都会
  走到跳页，游标继续推进（不会钉死）、回绕后重扫。超限页上的队伍始终
  收不到气球，这是宿主页上限下的硬边界；被误跳的健康页只是延迟一个
  轮转周期，不丢数据。
- 存储为单文件 `storage.json`（整文件上限 `MAX_STORAGE_BYTES`；get/set/
  CAS 共享每激活一份 GCRA 操作配额）：记录按比赛分键，长期使用多场
  比赛后接近上限时写入会 `limit_exceeded`。冷启动每个气球约 1 次写
  （CAS 认领每轮一次批量），首轮可落盘数量 ≈ 配额减去固定开销（探针
  CAS 与每轮 get/CAS），超出即队列停下、下轮继续、多轮收敛——正常行为
  不是故障（精确收敛行为由测试锁定）；若落盘失败发生在 POST 成功之后，
  该气球保持 `pending`、下轮以同一幂等键重发（远端去重），属
  at-least-once 的正常行为。需要时可停插件后清理 `plugin-data/` 下本
  插件目录（会丢失去重记录，远端幂等键仍可兜底）。
- 榜单是实时视图：跨页无事务快照，去重靠 `teamKey`；解封瞬间与首轮之间
  （至多一个 Resync 间隔）的新 AC 会被保守地跳过。
- 日志出口限流（`PLUGIN_LOGS_PER_MINUTE`）：一连串失败的突发期（每失败
  一条 warn）超出预算的行会被 `note()` 静默丢弃——排障时若日志缺失，
  先怀疑限流而非无事件；正常轮次每轮仅 1 行摘要，不会触顶。
- 派发语义是 at-least-once + 远端幂等去重；接收方必须按 `idempotencyKey` 去重。
