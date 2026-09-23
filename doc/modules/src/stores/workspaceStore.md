# workspaceStore（工作区状态与代码同步）

> 源文件：`src/stores/workspaceStore.ts`

## 职责

持有当前工作区（代码、语言、脏标记、活动文件），编排编辑器与 Rust 后端之间的代码同步（2s 防抖推送**内存**）、语言切换（乐观更新 + 立即持久化）与**落盘时机**（切题 / 失焦 / 关窗由本 store 与调用点协作）。

落盘语义（debounce-to-memory）：防抖只把代码推给后端内存，磁盘写入由后端 auto-save 与显式 `save_workspace` 负责。状态因此分两级：`syncPending`（未推送到后端内存）、`isDirty`（未落盘，驱动「编辑中…/已自动备份」）。

## 核心类型/函数

常量：`CODE_FILE_EXTENSIONS` = `utils/language` 的 `SOURCE_FILE_EXTENSIONS`（探测历史工作区代码文件的后缀清单，**从识别面唯一来源派生**防漂移；新文件名一律经 `sourceFileNameOf` 从语言派生）；`SYNC_DEBOUNCE_MS` = 2000。模块级副作用句柄（**不进响应式 state**，见 `utils/polling` 的句柄约定）：`let syncTimer`（防抖句柄）、`let lastPersistedRevision = 0`（**最近一次被接受的落盘修订号** —— 落盘水位）、`let lastPushedRevision = 0`（**最近一次推送给后端内存的修订号**）、`let loadGeneration = 0`（**加载代际计数器** —— 并发 `loadWorkspace` 只认最新一次）、`let loadQueue`（**加载 IPC 单飞队列** —— 保证后端 `find_or_create` 完成顺序与发起顺序一致）。语言权威值为 **HOJ 显示名**（"C++" 等，见 `utils/language`）。

| 名称 | 签名 | 用途 |
|------|------|------|
| state | `workspace` / `activeFile` / `code` / `language`（默认 'C++'）/ `isDirty` / `syncPending` / `isLoadingWorkspace` | `activeFile` = **当前代码文件名（权威源）**，读写都锚定它；`isDirty` = 有改动尚未落盘；`syncPending` = 有改动尚未推送到后端内存（防抖窗口内）；`isLoadingWorkspace` = 工作区加载在途（PR21-8：期间编辑器只读、输入拒收，ProblemSolveView 绑定为 CodeEditor 的 `locked`） |
| `currentCode` / `currentLanguage` | getters | 编辑器当前代码与语言 |
| `loadWorkspace` | `(contestId, problemId) => Promise<void>` | **`const gen = ++loadGeneration` + 置位 `isLoadingWorkspace`** → **先 `flushPendingSync()`**（本方法整体替换 code/language/workspace，在途改动不先推送就会被加载结果覆盖、再被旧内容回推）→ **flush 后代际检查**（已有更新切题则本次作废，不发 IPC）→ **加载 IPC 经 `loadQueue` 单飞**（等前一次落定再发起；轮到时已非最新代际则跳过 IPC 返回 null —— `load_workspace` 是 async 命令，并发乱序完成会让后端 current 停在旧工作区而 store 已切新工作区，解锁后的防抖推送就会把新题代码写进旧工作区目录）→ **IPC 后代际检查 + 原子应用**（迟到结果作废；结果经 turn 返回后一次性写入 store，轮次内直接写 `this.workspace` 会留下「workspace 已是新题、code 还是旧题」的失配对；**元数据语言解析在 language 为空 —— 新建工作区恒如此，Rust 端 `String::new()` —— 时走 `getDefaultLanguage`，配置缓存冷时是真实 IPC，必须先于任何 store 写入完成并在其后复查代际，否则 await 期间被取代会应用过期字段并提前解锁，apply 块因此真正原子，评审三轮**）→ 加载工作区：**代码文件以 `workspace.activeFile` 为权威源**（后端已持久化；回退链只服务历史工作区：语言派生名 → `CODE_FILE_EXTENSIONS` 后缀探测 —— `files` 来自 HashMap 序列化、键序不稳定，只靠探测可能加载出「旧语言代码 + 新语言元数据」）；language 与文件扩展名矛盾时**以文件为准**并 `warn`（判题端按后缀判语言）；恢复 isDirty；随后 `void purgeStaleCodeFiles()`（历史多文件收敛，P62）。**仅成功路径解锁**：失败保持锁定（闩锁，见设计要点）。**加载在途拒收一切输入（PR21-8）**：慢加载窗口内编辑器仍显示旧题代码，中途敲键属旧题上下文，随加载结果被覆盖是预期；「保留用户输入」更坏（旧题文本经防抖推到新工作区文件名下，跨工作区错配），正解是编辑器只读（CodeEditor `locked`）+ store 拒收（`updateCode` / `changeLanguage` 守卫）双保险 |
| `saveWorkspace` | `() => Promise<void>` | 落盘入口（切题 / 失焦 / 关窗 / 手动）：**先 `flushPendingSync()` 再 `save_workspace`** —— 顺序反了会把旧内容写进磁盘；仅当「推送成功 **且** 期间无新改动（`!syncPending`）」才清 `isDirty`（推送失败 → 内容未进后端；期间又落键 → 最新改动连后端内存都还没到，清脏会显示假「已自动备份」） |
| `updateCode` | `(code: string) => void` | 编辑器输入：更新 code + `isDirty = true` + `syncPending = true` + `scheduleSync()`；**`isLoadingWorkspace` 时拒收**（PR21-8：Monaco 只读是第一道防线，此处兜住工具条清空/上传等绕过键盘的程序化路径） |
| `scheduleSync` | `() => void` | 重置 2s 定时器，到点调用 `flushPendingSync()` |
| `flushPendingSync` | `() => Promise<boolean>` | 取消防抖窗口并立即把代码按 **`activeFile`**（权威源；回退 `sourceFileNameOf(language)`）推送到后端内存；返回是否成功，失败只 `log.error` 并保留 `syncPending`（调用方不应被一次 IPC 失败阻断）。**推送期间又有新改动时不清 `syncPending`**（本次推送带的是调用时刻的内容；清了会让新内容既不被本次携带、又被下次防抖短路跳过）。推送成功后把返回值记为 `lastPushedRevision`，并**补一次清脏判定**：`!syncPending && lastPersistedRevision >= revision` 时清 `isDirty` —— 落盘事件可能在推送在途时已到达（此时 `markPersisted` 因 `syncPending` 推迟清脏），而后端已 clean、后续 auto-save tick 不再发事件，不补判指示器会卡在「编辑中…」。判据方向必须是**水位 ≥ 推送**（磁盘追上编辑器）；反向会在事件未到时提前清脏，即假「已自动备份」 |
| `markPersisted` | `(workspaceId?: string, revision?: number) => void` | 后端落盘事件（`workspace-saved`）到达时清 `isDirty`。四重过滤：① 按 `workspaceId` 过滤过期事件（旧工作区的落盘可能在新工作区已编辑后才送达）；② 按 `revision` **幂等** —— 后端 `revision` 是全局单调计数器，小于等于 `lastPersistedRevision` 的事件一律视为重复或过期而跳过（落盘事实只能被更新的修订号推进，不能被回退），并推进该标量；③ **`revision < lastPushedRevision` 时不清脏**（该落盘快照早于编辑器已推送的最新内容，磁盘还没追上编辑器）；④ 若期间又有新改动（`syncPending`）则不清 —— 后端在写失败或快照后又有新改动时不发该事件，故清除等价于「最新内容确已在磁盘上」 |
| `cancelPendingSync` | `() => void` | 取消未触发的防抖同步、清 `syncPending`，并把 `lastPersistedRevision` 与 `lastPushedRevision` **双双归零**（登出/切换账号时调用，避免向已失效会话写入代码；新会话与旧会话的修订号不可比） |
| `purgeStaleCodeFiles` | `() => Promise<void>` | 清理 ≠ `activeFile` 的代码文件（**单代码文件约束**，P62 已闭合）：语言切换后编辑器内容已复制到新文件名，旧扩展名文件成为过期残留，不清理会一直被后端 `save()` 全量落盘。代码文件判定沿用 `CODE_FILE_EXTENSIONS`，非代码文件不在清理范围；逐个删除、失败仅 `log.error`（下次加载/切换重试）；**每次删除前重查 `activeFile` 与工作区引用**（切换在途时避免误删新 active 文件，后端 `active_file` 守卫为第二道防线） |
| `changeLanguage` | `(lang: string) => void` | 切换语言，见逻辑流程；**`isLoadingWorkspace` 时拒收**（PR21-8 同源：此时切换会把旧题代码经 flush 推到新派生文件名下 —— 后端若已切到新工作区，就是跨工作区的内容错配） |
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
  → gen = ++loadGeneration + isLoadingWorkspace = true
  → flushPendingSync() → 代际检查（过期则作废，不发 IPC）
  → loadQueue 单飞发起加载 IPC（过期 turn 跳过 IPC 返回 null）
  → 代际检查 + 原子应用（迟到结果作废，一次性写入 store；
    language 为空时 getDefaultLanguage 属 apply 前置解析，其后复查代际）
  → activeFile 解析（权威源 + 历史回退链）
  → void purgeStaleCodeFiles()                // 历史多文件工作区收敛（P62）
  → 仅成功路径复位 isLoadingWorkspace（带代际守卫；失败 catch 显式重置为 true）
```

设计要点：

- **加载在途拒收输入（PR21-8 已闭合）**：`loadWorkspace` 的 IPC 往返窗口内编辑器仍显示
  旧题代码，此时敲键属旧题上下文 —— 随加载结果被覆盖是预期行为；「保留用户输入」反而
  更坏（会把旧题文本经防抖推到新工作区文件名下，跨工作区错配）。防线三层：CodeEditor
  `locked`（Monaco 只读，用户可见不可编辑）→ `updateCode` 守卫（兜住工具条清空/上传等
  绕过键盘的程序化路径）→ `changeLanguage` 守卫（防止旧题代码被 flush 推到新派生文件名
  下）。**失败闩锁（评审二轮）**：仅成功路径解锁 —— 最新一次加载失败时后端 current
  与 store 是否一致不可知（单飞队列下更早的过期加载可能已在后端完成切换、其结果被
  代际检查丢弃），解锁后的防抖推送会把 store 代码写进后端实际工作区（跨工作区写入）；
  保持锁定 + 视图层错误提示（含重试），下次加载成功即解锁。**原子应用（评审二轮）**：
  加载结果经 turn 返回后代际检查通过才一次性写入 store，轮次内直接写 `this.workspace`
  会留下「workspace 已是新题、code 还是旧题」的失配对，后续加载失败时无法自愈。
  **apply 块的原子性以「无 await」为准（评审三轮）**：元数据语言解析（language 为空时
  走 `getDefaultLanguage`，新建工作区恒如此、配置缓存冷时是真实 IPC）是 apply 路径上
  唯一的 await，必须先于任何 store 写入完成并在其后复查代际 —— 否则该 await 期间被
  新加载取代的过期加载会应用过期字段并无条件解锁，击穿失败闩锁。解锁同样带代际守卫、
  失败 catch 显式重置标志为 true（而非仅「不清除」），均为防御性加固。
  加载期间的失焦/页面隐藏落盘（`flushToDisk`）无需守卫：两个守卫保证 `syncPending`
  在加载期间不可能置位，flush 空转、后端 `save_workspace` 对 clean 工作区是无操作。
- **并发切题的代际 + 单飞（PR21-8 评审加固）**：快速切题 A→B→C 时 B、C 两次加载并发
  在途。**代际计数器** `loadGeneration`：B 先完成不得复位加载锁（否则 C 的 IPC 仍在飞，
  三层防线提前全部失效）、迟到结果不得覆盖 store（两处代际检查：flush 后、IPC 后）；
  **单飞队列** `loadQueue`：`load_workspace` 是 async 命令，并发 `find_or_create` 乱序
  完成会让后端 current 停在旧工作区而 store 已切新工作区 —— 解锁后的防抖推送就会把
  新题代码写进旧工作区目录；串行化保证后端切换顺序与发起顺序一致，轮到时已过期的
  turn 直接跳过 IPC（不产生多余的后端切换），队列吞错（单次失败不阻塞后续切题，错误
  经 turn 向调用方传播）。加载锁横跨整个并发窗口还带来一个推论：锁期间 `syncPending`
  不可能置位，因此所有 flush 都空转，不存在「flush 携带旧题内容写到已切换的后端」的
  窗口 —— 携带内容的只有第一次 flush，它必然落在任何加载 IPC 之前（后端还在旧工作区）。
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
