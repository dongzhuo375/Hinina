# workspaceStore（工作区状态与代码同步）

> 源文件：`src/stores/workspaceStore.ts`

## 职责

持有当前工作区（代码、语言、脏标记、活动文件），编排编辑器与 Rust 后端之间的代码同步（2s 防抖推送）与语言切换（乐观更新 + 立即持久化）。

## 核心类型/函数

常量：`CODE_FILE_EXTENSIONS`（探测历史工作区代码文件的后缀白名单；新文件名一律经 `sourceFileNameOf` 从语言派生）。语言权威值为 **HOJ 显示名**（"C++" 等，见 `utils/language`）。

| 名称 | 签名 | 用途 |
|------|------|------|
| state | `workspace` / `activeFile` / `code` / `language`（默认 'cpp'）/ `isDirty` / `_syncTimer` | `_syncTimer` 是 2s 防抖定时器句柄（下划线前缀标记非持久化状态） |
| `currentCode` / `currentLanguage` | getters | 编辑器当前代码与语言 |
| `loadWorkspace` | `(contestId, problemId) => Promise<void>` | 加载工作区：恢复 language（元数据残留的历史 Monaco id 经 `normalizeHojLanguage` 归一为 HOJ 显示名；未记录语言时用 `configService.getDefaultLanguage()`，兜底 "C++"）、isDirty；代码文件优先取当前语言派生名，其次按 `CODE_FILE_EXTENSIONS` 后缀探测（兼容历史任意命名） |
| `saveWorkspace` | `() => Promise<void>` | 显式保存并清脏标记（切题前落盘由 ProblemSolveView 调用） |
| `updateCode` | `(code: string) => void` | 编辑器输入：更新 code + 标脏 + 触发防抖同步 |
| `debouncedSync` | `() => void` | 2s 无操作后把代码按 `sourceFileNameOf(language)` 派生的文件名推送到后端（`updateWorkspaceFile`）；失败只 console.error |
| `cancelPendingSync` | `() => void` | 取消未触发的防抖同步（登出/切换账号时调用，避免向已失效会话写入代码） |
| `changeLanguage` | `(lang: string) => void` | 切换语言，见逻辑流程 |

## 直接依赖

- `pinia`
- `@/types/workspace`（仅类型）
- `@/services/workspace.service`（`workspaceService`）
- `@/services/config.service`（`getDefaultLanguage` 默认语言）

## 被依赖

- `views/ProblemSolveView.vue` — 加载工作区、切题前落盘、提交时取 `language` / `code`
- `components/editor/CodeEditor.vue`（经 ProblemSolveView 的 v-model / update:language 绑定）
- `components/problem/ProblemStatement.vue` — 读 `language` 计算生效 limits
- `stores/session.ts` — 登出清理：`cancelPendingSync()` + `$reset()`

## 逻辑流程

```
updateCode(code)
  → code 更新 + isDirty = true
  → debouncedSync：重置 2s 定时器 → 到点仍 dirty 时
     workspaceService.updateWorkspaceFile(sourceFileNameOf(language), code)
     （后端 update_file 写内存 + 落盘；auto-save 与 save_workspace 再兜底）

changeLanguage(lang)
  → 相同语言直接 return
  → this.language = lang + isDirty = true      // 乐观更新：UI 立即生效，不等待 IPC
  → workspaceService.setLanguage(lang)          // 立即持久化到后端 workspace.json
     失败 → 只 console.error，不回滚本地选择
```

设计要点：

- **changeLanguage 为什么乐观更新且不回滚**：语言不属于任何代码文件，防抖同步
  （updateWorkspaceFile）带不上它，必须走独立的 `set_workspace_language` 立即落盘——
  否则切题/重启后退回默认语言，Java 代码被当 C++ 提交。持久化失败时**阻断切换比
  丢失持久化更影响比赛**（选手当下就要用新语言写代码），故只记录日志不回滚；
  日志中明示后果（「切题或重启后可能退回默认语言」）便于排查。
- 语言切换标脏（isDirty = true）：驱动 CodeEditor 备份指示回到「编辑中…」，
  并让后续保存路径把元数据一并落盘（后端 `save()` 会 `persist_meta`）。
- `_syncTimer` 是副作用句柄，登出时必须先 `cancelPendingSync()` 再 `$reset()`
  （`$reset` 清不掉已排定的 setTimeout 回调，见 `stores/session.md`）。
- 代码文件按后缀识别而非固定文件名：兼容后端工作区中已存在的任意命名。
