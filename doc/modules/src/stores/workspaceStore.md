# workspaceStore（工作区状态与代码同步）

> 源文件：`src/stores/workspaceStore.ts`

## 职责

持有当前工作区（代码、语言、脏标记、活动文件），编排编辑器与 Rust 后端之间的代码同步（2s 防抖推送**内存**）、语言切换（乐观更新 + 立即持久化）与**落盘时机**（切题 / 失焦 / 关窗由本 store 与调用点协作）。

落盘语义（debounce-to-memory）：防抖只把代码推给后端内存，磁盘写入由后端 auto-save 与显式 `save_workspace` 负责。状态因此分两级：`syncPending`（未推送到后端内存）、`isDirty`（未落盘，驱动「编辑中…/已自动备份」）。

## 核心类型/函数

常量：`CODE_FILE_EXTENSIONS` = `utils/language` 的 `SOURCE_FILE_EXTENSIONS`（探测历史工作区代码文件的后缀清单，**从识别面唯一来源派生**防漂移；新文件名一律经 `sourceFileNameOf` 从语言派生）；`SYNC_DEBOUNCE_MS` = 2000。模块级副作用句柄（**不进响应式 state**，见 `utils/polling` 的句柄约定）：`let syncTimer`（防抖句柄）、`let lastPersistedRevision = 0`（**最近一次被接受的落盘修订号** —— 落盘水位）、`let lastPushedRevision = 0`（**最近一次推送给后端内存的修订号**）。语言权威值为 **HOJ 显示名**（"C++" 等，见 `utils/language`）。

| 名称 | 签名 | 用途 |
|------|------|------|
| state | `workspace` / `activeFile` / `code` / `language`（默认 'C++'）/ `isDirty` / `syncPending` | `activeFile` = **当前代码文件名（权威源）**，读写都锚定它；`isDirty` = 有改动尚未落盘；`syncPending` = 有改动尚未推送到后端内存（防抖窗口内） |
| `currentCode` / `currentLanguage` | getters | 编辑器当前代码与语言 |
| `loadWorkspace` | `(contestId, problemId) => Promise<void>` | **先 `flushPendingSync()`**（本方法整体替换 code/language/workspace，在途改动不先推送就会被加载结果覆盖、再被旧内容回推）→ 加载工作区：**代码文件以 `workspace.activeFile` 为权威源**（后端已持久化；回退链只服务历史工作区：语言派生名 → `CODE_FILE_EXTENSIONS` 后缀探测 —— `files` 来自 HashMap 序列化、键序不稳定，只靠探测可能加载出「旧语言代码 + 新语言元数据」）；language 与文件扩展名矛盾时**以文件为准**并 `warn`（判题端按后缀判语言）；恢复 isDirty；随后 `void purgeStaleCodeFiles()`（历史多文件收敛，P62） |
| `saveWorkspace` | `() => Promise<void>` | 落盘入口（切题 / 失焦 / 关窗 / 手动）：**先 `flushPendingSync()` 再 `save_workspace`** —— 顺序反了会把旧内容写进磁盘；仅当「推送成功 **且** 期间无新改动（`!syncPending`）」才清 `isDirty`（推送失败 → 内容未进后端；期间又落键 → 最新改动连后端内存都还没到，清脏会显示假「已自动备份」） |
| `updateCode` | `(code: string) => void` | 编辑器输入：更新 code + `isDirty = true` + `syncPending = true` + `scheduleSync()` |
| `scheduleSync` | `() => void` | 重置 2s 定时器，到点调用 `flushPendingSync()` |
| `flushPendingSync` | `() => Promise<boolean>` | 取消防抖窗口并立即把代码按 **`activeFile`**（权威源；回退 `sourceFileNameOf(language)`）推送到后端内存；返回是否成功，失败只 `log.error` 并保留 `syncPending`（调用方不应被一次 IPC 失败阻断）。**推送期间又有新改动时不清 `syncPending`**（本次推送带的是调用时刻的内容；清了会让新内容既不被本次携带、又被下次防抖短路跳过）。推送成功后把返回值记为 `lastPushedRevision`，并**补一次清脏判定**：`!syncPending && lastPersistedRevision >= revision` 时清 `isDirty` —— 落盘事件可能在推送在途时已到达（此时 `markPersisted` 因 `syncPending` 推迟清脏），而后端已 clean、后续 auto-save tick 不再发事件，不补判指示器会卡在「编辑中…」。判据方向必须是**水位 ≥ 推送**（磁盘追上编辑器）；反向会在事件未到时提前清脏，即假「已自动备份」 |
| `markPersisted` | `(workspaceId?: string, revision?: number) => void` | 后端落盘事件（`workspace-saved`）到达时清 `isDirty`。四重过滤：① 按 `workspaceId` 过滤过期事件（旧工作区的落盘可能在新工作区已编辑后才送达）；② 按 `revision` **幂等** —— 后端 `revision` 是全局单调计数器，小于等于 `lastPersistedRevision` 的事件一律视为重复或过期而跳过（落盘事实只能被更新的修订号推进，不能被回退），并推进该标量；③ **`revision < lastPushedRevision` 时不清脏**（该落盘快照早于编辑器已推送的最新内容，磁盘还没追上编辑器）；④ 若期间又有新改动（`syncPending`）则不清 —— 后端在写失败或快照后又有新改动时不发该事件，故清除等价于「最新内容确已在磁盘上」 |
| `cancelPendingSync` | `() => void` | 取消未触发的防抖同步、清 `syncPending`，并把 `lastPersistedRevision` 与 `lastPushedRevision` **双双归零**（登出/切换账号时调用，避免向已失效会话写入代码；新会话与旧会话的修订号不可比） |
| `purgeStaleCodeFiles` | `() => Promise<void>` | 清理 ≠ `activeFile` 的代码文件（**单代码文件约束**，P62 已闭合）：语言切换后编辑器内容已复制到新文件名，旧扩展名文件成为过期残留，不清理会一直被后端 `save()` 全量落盘。代码文件判定沿用 `CODE_FILE_EXTENSIONS`，非代码文件不在清理范围；逐个删除、失败仅 `log.error`（下次加载/切换重试）；**每次删除前重查 `activeFile` 与工作区引用**（切换在途时避免误删新 active 文件，后端 `active_file` 守卫为第二道防线） |
| `changeLanguage` | `(lang: string) => void` | 切换语言，见逻辑流程 |
| `installWorkspacePersistenceListener` | `() => void` | 组合根（`main.ts`）安装一次：订阅 `workspace-saved` → `markPersisted(payload.workspaceId, payload.revision)`（`revision` 原样透传，幂等过滤在 store 内完成）；订阅失败只降级指示器 |

## 直接依赖

- `pinia`
- `@/types/workspace`（仅类型）
- `@/services/workspace.service`（`workspaceService`，含 `onWorkspaceSaved` 订阅）
- `@/services/config.service`（`getDefaultLanguage` 默认语言）
- `@/utils/language`（`sourceFileNameOf` / `normalizeHojLanguage` / `SOURCE_FILE_EXTENSIONS`）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `views/ProblemSolveView.vue` — 加载工作区、切题/失焦/离开页面时落盘、提交时取 `language` / `code`
- `components/editor/CodeEditor.vue`（经 ProblemSolveView 的 v-model / update:language 绑定；`isDirty` 驱动「编辑中…/已自动备份」）
- `components/problem/ProblemStatement.vue` — 读 `language` 计算生效 limits
- `stores/session.ts` — 登出清理：`cancelPendingSync()` + `$reset()`
- `main.ts` — 组合根：`installWorkspacePersistenceListener()`（落盘事件订阅）+ 关窗握手调用 `saveWorkspace()`

## 逻辑流程

```
updateCode(code)
  → code 更新 + isDirty = true + syncPending = true
  → scheduleSync：重置 2s 定时器 → 到点 flushPendingSync()
     workspaceService.updateWorkspaceFile(sourceFileNameOf(language), code)
     → 返回修订号 → lastPushedRevision = revision
     （后端 update_file 只写内存 + 标脏 + 递增修订号；落盘由 auto-save / save_workspace 负责）
     → 补判：!syncPending && lastPersistedRevision >= revision → isDirty = false

saveWorkspace()            // 切题（ProblemSolveView.load） / 失焦 / 关窗（main.ts） / 离开解题页
  → flushPendingSync()     // 先推内存：反了会把旧内容写进磁盘
  → workspaceService.saveWorkspace()   // 后端全量落盘 + 发布 Saved
  → 推送成功才清 isDirty

workspace-saved 事件（后端 CoreEvent::WorkspaceSaved 经 main.rs 前端事件桥下发）
  → markPersisted(workspaceId, revision)
      ├─ workspaceId 与当前工作区不符 → 丢弃（过期事件）
      ├─ revision <= lastPersistedRevision → 丢弃（重复/回退）
      ├─ revision < lastPushedRevision → 不清脏（磁盘还没追上编辑器）
      └─ 否则推进 lastPersistedRevision → isDirty = false（「已自动备份」= 磁盘真值）

changeLanguage(lang)
  → 相同语言直接 return
  → this.language = lang                      // 乐观更新：UI 立即生效，不等待 IPC
  → workspaceService.setLanguage(lang)        // 立即持久化到后端 workspace.json
     失败 → 只 log.error 记录（utils/logger 作用域日志），不回滚本地选择
  → 派生文件名变化时（如 C++ → Java）：activeFile = 新派生名 + syncPending = true
  → flushPendingSync()                        // 把当前代码同步到新文件名
     失败（pushed=false）→ 中止整条链：旧文件保留作崩溃恢复副本，下次重试
  → saveWorkspace()                           // 新文件内容 + 元数据（active_file=新名）落盘
     失败 → 中止清理（同上）
  → purgeStaleCodeFiles()                     // 磁盘确有新副本后，静默清理旧扩展名文件
                                             //（同族切换派生名不变也执行，顺带收敛历史遗留）

loadWorkspace(contestId, problemId)
  → flushPendingSync() → 加载 → activeFile 解析（权威源 + 历史回退链）
  → void purgeStaleCodeFiles()                // 历史多文件工作区收敛（P62）
```

设计要点：

- **changeLanguage 为什么乐观更新且不回滚**：语言不属于任何代码文件，防抖同步
  （updateWorkspaceFile）带不上它，必须走独立的 `set_workspace_language` 立即落盘——
  否则切题/重启后退回默认语言，Java 代码被当 C++ 提交。持久化失败时**阻断切换比
  丢失持久化更影响比赛**（选手当下就要用新语言写代码），故只记录日志不回滚；
  日志中明示后果（「切题或重启后可能退回默认语言」）便于排查。
- **changeLanguage 不再标脏**：语言由后端立即落盘，不属于「未落盘的代码改动」；
  旧实现置 `isDirty = true` 会让「编辑中…」在语言切换后一直挂着。
- **单代码文件约束（P62 已闭合）**：切换语言时编辑器内容复制到新派生文件名，旧扩展名
  文件由 `purgeStaleCodeFiles` 静默清理（不弹窗、不阻断 —— 内容已复制，常规场景删除
  无损失；历史分叉旧文件一并清理是项目决策）。**清理链为 flush → save → purge**：
  purge 删除的旧文件是磁盘上唯一的崩溃恢复副本，必须等新文件内容与元数据
  （active_file=新名，顺带修正 `set_language` 落盘的旧名 meta）确已落盘后才删 ——
  否则 auto-save 间隔内（默认 30s，最长 300s 或可关闭）崩溃会丢代码；推送失败
  （flush 返回 false，`saveWorkspace` 同款检查）或落盘失败时中止清理，旧文件保留，
  下次加载/切换时重试。`loadWorkspace` 的清理无需先落盘 —— 加载时 active 文件的
  磁盘副本本来就在，删的是其他文件，不影响崩溃恢复。极端时序下清理先于推送到达、
  误删新文件名也无损 —— 随后的推送会以编辑器内容重建它。
- **落盘时机由调用点编排，而不是靠后端周期**：auto-save 周期最长 300 秒，
  而「内存 → 磁盘」之间只有后端内存副本；切题（loadWorkspace 前 save）、失焦、
  页面隐藏、离开解题页、关窗各由对应调用点显式落盘。store 只提供 `flushPendingSync`
  与 `saveWorkspace` 两个原子动作，不自行注册全局监听。
- **防抖句柄不进 state**：`syncTimer`、`lastPersistedRevision`、`lastPushedRevision` 都是模块级普通变量
  （与 `utils/polling` 的约定一致），登出时必须先 `cancelPendingSync()` 再 `$reset()`
  （`$reset` 清不掉已排定的 setTimeout 回调，也清不掉模块级标量，见 `stores/session.md`）。
- **两个修订号水位缺一不可**：`lastPersistedRevision` 是**磁盘水位**（后端确已落盘到的修订号），
  `lastPushedRevision` 是**编辑器水位**（已推送进后端内存的修订号）。清脏的充分条件是
  「磁盘水位 ≥ 编辑器水位」，即磁盘已包含编辑器已推送的全部内容；只看磁盘水位会在事件早到时
  误判（磁盘还落后却清脏 = 假「已自动备份」），只看编辑器水位则会在事件永不到达时卡住指示器。
- **`revision` 幂等为什么只需一个标量**：后端 `revision` 是**全局单调计数器**（工作区每次
  内容改动 +1，见 `service/workspace/manager.md`），因此「同一事件重复送达」与「旧修订号
  的事件晚于新修订号到达」都能用 `revision <= lastPersistedRevision` 一次判掉；落盘事实
  只能被更新的修订号推进。切账号后新旧会话的修订号不可比，故在 `cancelPendingSync` 归零。
- **修订号比较的语义边界**：`revision` 是 `WorkspaceManager` 实例级全局计数器（不是每工作区一份），
  且只在 `new()` 初始化为 0、无其他重置点 —— 所以跨工作区切换不会回退，比较仍然成立；
  `markPersisted` 的 `workspaceId` 过滤先于水位推进，其他工作区的快照不会污染本工作区的水位。
- 代码文件按后缀识别而非固定文件名：兼容后端工作区中已存在的任意命名。
