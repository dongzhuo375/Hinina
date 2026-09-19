# workspace.bridge（工作区 IPC 桥接）

> 源文件：`src/bridge/workspace.bridge.ts`

## 职责

工作区相关 Tauri IPC 的薄封装：对 `load_workspace` / `save_workspace` / `current_workspace` / `update_workspace_file` / `set_workspace_language` 五个 Command 做参数透传，并把后端的 `workspace-saved` 事件（落盘通知）封装为订阅函数，不含业务逻辑。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `loadWorkspace` | `(contestId, problemId) => Promise<Workspace>` | invoke `load_workspace`（后端 find_or_create：命中恢复代码，未命中新建） |
| `saveWorkspace` | `() => Promise<void>` | invoke `save_workspace`（全量落盘；成功时后端另发 `workspace-saved`） |
| `currentWorkspace` | `() => Promise<Workspace \| null>` | invoke `current_workspace`（null = 无活动工作区） |
| `updateWorkspaceFile` | `(fileName, content) => Promise<void>` | invoke `update_workspace_file`（编辑器防抖同步的落点，**只写后端内存不落盘**） |
| `setWorkspaceLanguage` | `(language) => Promise<Workspace>` | invoke `set_workspace_language`，返回更新后的 Workspace。**语言不属于任何代码文件，`updateWorkspaceFile` 带不上它**；不单独持久化会导致切题/重启后退回默认语言，从而用错语言提交 |
| `WorkspaceSavedPayload` | `{ workspaceId: string; auto: boolean }` | 落盘事件载荷（`auto = true` 为后台 auto-save，`false` 为显式保存） |
| `onWorkspaceSaved` | `(handler) => Promise<UnlistenFn>` | 订阅 `workspace-saved`（`@tauri-apps/api/event` 的 `listen`）：后端在**内容确已落盘**时才发（写失败、或快照后又有新改动时不发），前端据此清「编辑中…」指示 |

## 直接依赖

- `@/bridge`（`ipcInvoke` 统一出口：错误归一为 `IpcError` + 观察者通知）
- `@tauri-apps/api/event`（`listen` —— 唯一使用事件通道的桥接，见设计要点）
- `@/types/workspace`（仅类型）

## 被依赖

- `services/workspace.service.ts` — 唯一调用方

## 逻辑流程

```
workspace.service → 各桥接函数 → ipcInvoke(cmd, args)
  → Rust commands::workspace_cmd 对应 Command → WorkspaceManager

后端 WorkspaceEvent::Saved / AutoSaveTriggered
  → main.rs 的事件桥 emit("workspace-saved", { workspaceId, auto })
  → onWorkspaceSaved 回调 → workspaceStore.markPersisted()
```

设计要点：

- `setWorkspaceLanguage` 与 Rust 端 `commands::workspace_cmd::set_workspace_language`
  一一对应（参数 `{ language }`，Tauri 自动映射 snake_case 形参）。
- 桥接层不做重试/防抖——节奏控制在 store（2s 防抖）与后端（auto-save）两层完成。
- **`onWorkspaceSaved` 为何打破「桥接只有 ipcInvoke」的惯例**：后台 auto-save 由 Rust
  触发，是唯一非前端发起的落盘路径，前端无法用 invoke 感知；「已自动备份」若没有
  这个信号就只能靠猜（旧实现即为假指示器）。事件名与载荷定义在 `main.rs` 的
  `WORKSPACE_SAVED_EVENT` 常量，两端需同步修改。
