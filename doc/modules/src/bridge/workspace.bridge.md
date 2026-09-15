# workspace.bridge（工作区 IPC 桥接）

> 源文件：`src/bridge/workspace.bridge.ts`

## 职责

工作区相关 Tauri IPC 的薄封装：对 `load_workspace` / `save_workspace` / `current_workspace` / `update_workspace_file` / `set_workspace_language` 五个 Command 做参数透传，不含业务逻辑。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `loadWorkspace` | `(contestId, problemId) => Promise<Workspace>` | invoke `load_workspace`（后端 find_or_create：命中恢复代码，未命中新建） |
| `saveWorkspace` | `() => Promise<void>` | invoke `save_workspace`（持久化脏文件） |
| `currentWorkspace` | `() => Promise<Workspace \| null>` | invoke `current_workspace`（null = 无活动工作区） |
| `updateWorkspaceFile` | `(fileName, content) => Promise<void>` | invoke `update_workspace_file`（编辑器防抖同步的落点） |
| `setWorkspaceLanguage` | `(language) => Promise<Workspace>` | invoke `set_workspace_language`，返回更新后的 Workspace。**语言不属于任何代码文件，`updateWorkspaceFile` 带不上它**；不单独持久化会导致切题/重启后退回默认语言，从而用错语言提交 |

## 直接依赖

- `@/bridge`（`ipcInvoke` 统一出口：错误归一为 `IpcError` + 观察者通知）
- `@/types/workspace`（仅类型）

## 被依赖

- `services/workspace.service.ts` — 唯一调用方

## 逻辑流程

```
workspace.service → 各桥接函数 → ipcInvoke(cmd, args)
  → Rust commands::workspace_cmd 对应 Command → WorkspaceManager
```

设计要点：

- `setWorkspaceLanguage` 是本轮新增，与 Rust 端 `commands::workspace_cmd::set_workspace_language`
  一一对应（参数 `{ language }`，Tauri 自动映射 snake_case 形参）。
- 桥接层不做重试/防抖——节奏控制在 store（2s 防抖）与后端（auto-save）两层完成。
