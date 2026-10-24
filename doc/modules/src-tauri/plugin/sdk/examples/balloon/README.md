# 首次 AC 气球参考插件（阶段 C）

施工计划（`doc/plugin/plan.md` §5）的参考业务插件：证明宿主能力组合
（日志 + 作用域查询 + 题目列表/榜单 + 私有存储 CAS + 受限 HTTP）能完成
"哪支队在哪道题上首 A → 派发气球"的目标工作流。**业务规则全部留在插件**，
宿主不知道"气球"是什么。它是文档的一部分，不进入 `src-tauri/`，不构建进应用。

## 行为

每次 `resync.required`（运行时无 `setTimeout`，节拍只能来自 Resync）：

1. `context.current` → `contest.current`；未选比赛 → 本轮安静结束；
2. **先看 `sealed`**：封榜期间整轮直接结束，不读榜单、不派发——封榜时
   `solved`/`firstAc` 恒 false，与真没通过不可区分；
3. `contest.problems`（题号 ↔ 气球颜色）→ `contest.rank` 逐页拉取，
   跨页按 `teamKey` 去重，打星队（`rank === -1`）跳过；
4. `firstAc === true` 且未派发过 → 先写 `pending`（CAS）→ `http.request`
   POST 派发（带幂等键）→ 成功标 `sent`；失败保留 `pending`，下次 Resync 重试；
5. 遇到 `context_expired` 立即停止本轮，等下次 Resync；
6. 全程 `hinina.log.*` 输出阶段、`sealed` 状态与计数。

### 关键规则

| 规则 | 实现 |
|---|---|
| 不补派封榜期间的历史首 A | 存储记忆封榜窗口 `[sealedSince, revealedAt)`；AC 时间落在窗口内的首 A 永久跳过。`revealedAt` 取解封后首轮的事件 `envelope.occurredAt`（插件无时钟） |
| 幂等 | 本地：`pending` → CAS → 派发 → `sent`；远端：POST 体携带 `idempotencyKey = contestId:teamKey:displayId`，接收方按它去重。CAS 只保证本地单进程事务 |
| 跨比赛隔离 | 存储键带 `contestId`（`balloon.records:<contestId>`），换比赛不会因同队同题号碰撞漏派 |
| 启动即校验能力 | activate 用无副作用的探针（哑上下文 + 不可能命中的 CAS）检查 `contest.rank`/`contest.read`/storage/`network.request`；缺权限时插件直接 `failed` 并指认缺失项，而不是每轮空转 |
| 冷启动 | 参考插件不区分"插件启动前/后"的首 A：首次运行会把当前榜单上所有可见首 A 视为待派发。需要该语义的作者可自行记录 firstSeen |

## 构建

仓库根目录（Node.js ≥20.6）：

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

`plugins.json`（`options.endpoint` 是派发目标，必须是 HTTPS URL——插件 HTTP
拒绝 loopback/私网，明文 HTTP 需另行显式批准）：

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

- `resyncIntervalSecs` 建议 ≥30：榜单查询有每激活 6 次/分钟的独立预算，
  多页榜单 + 过快节拍会让部分轮次 `limit_exceeded`（会被安全跳过并重试）。
- 改 `plugins.json` / 清单 / 代码后重启应用生效（`reload` 是开发期便利）。

## 端到端验收（需真实环境，人工执行）

> **约束**：插件 HTTP 明确拒绝 loopback/私网地址，**不能用本地 mock server**。
> 推荐用公网 HTTPS echo 端点（如 httpbin.org）接收假数据，验收完即关；
> 验收数据一律使用假队伍名，不涉及真实选手。

按 `doc/plugin/plan.md` §5.3 清单逐项执行：

| # | 场景 | 操作 | 预期 |
|---|---|---|---|
| 1 | 自动启动 | `enabled=true` 后重启应用 | 插件自动启动，`window.hininaPlugins.list()`（DEV）显示 running |
| 2 | 审批不足 | 清单声明 `contest.rank` 但 `plugins.json` 未批准，重启 | 状态 `failed` + `permission_denied{contest.rank}`（activate 探针指认） |
| 3 | 跨页去重 | 榜单 ≥2 页的比赛 | 每个首 A 只派发一次，无漏派；日志出现 `dispatched N` |
| 4 | 失败重试 | 断网或端点返回 5xx | 该气球保留 `pending`，下次 Resync 重试；`hinina.log.warn` 可查 |
| 5 | 封榜 | 进入封榜窗口 | 不派发；`logs/hinina.log` 出现 `sealed` |
| 6 | 作用域失效 | 切 OJ / 登出 / 换比赛 | 旧句柄 `context_expired`，本轮停止，不误派 |
| 7 | 退出 | 关闭应用 | 退出不阻塞；下次启动无残留 worker |

日志位置：`logs/hinina.log`（设置页可查数据目录路径）。

## 已知边界

- 存储为单文件 `storage.json`（整文件 ≤1MiB）：记录按比赛分键，长期使用
  多场比赛后接近上限时写入会 `limit_exceeded`；需要时可停插件后清理
  `plugin-data/` 下本插件目录（会丢失去重记录，远端幂等键仍可兜底）。
- 榜单是实时视图：跨页无事务快照，去重靠 `teamKey`；解封瞬间与首轮之间
  （至多一个 Resync 间隔）的新 AC 会被保守地跳过。
- 派发语义是 at-least-once + 远端幂等去重；接收方必须按 `idempotencyKey` 去重。
