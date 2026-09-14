# workspace.service（工作区服务）

> 源文件：`src/services/workspace.service.ts`

## 职责

工作区领域的服务层：把 store 的意图（加载/保存/查询/同步代码/切换语言）转发给 Bridge，是 Store 访问工作区后端的唯一入口。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `WorkspaceService.loadWorkspace` | `(contestId, problemId) => Promise<Workspace>` | 加载或创建工作区 |
| `WorkspaceService.saveWorkspace` | `() => Promise<void>` | 保存当前工作区 |
| `WorkspaceService.currentWorkspace` | `() => Promise<Workspace \| null>` | 获取当前活跃工作区（可能为 null） |
| `WorkspaceService.updateWorkspaceFile` | `(fileName, content) => Promise<void>` | 同步编辑器代码到后端 |
| `WorkspaceService.setLanguage` | `(language) => Promise<Workspace>` | 设置语言（后端**立即持久化元数据**并返回更新后的 Workspace） |
| `workspaceService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/workspace.bridge`（五个桥接函数）
- `@/types/workspace`（仅类型）

## 被依赖

- `stores/workspaceStore.ts` — 唯一调用方（分层约定：View/组件不直接调 bridge）

## 逻辑流程

```
workspaceStore 各 action → workspaceService 对应方法 → workspace.bridge → IPC
（当前为一比一转发，无额外编排；领域参数整形与失败策略都在 store 层）
```

设计要点：

- `setLanguage` 与 `updateWorkspaceFile` 分开：语言属于工作区**元数据**（workspace.json），
  代码属于**文件内容**，后端持久化路径不同（前者立即落盘、后者随保存/自动保存落盘），
  服务层保持两个独立方法而不是合并参数。
- 无状态、无缓存：工作区的权威状态在后端（内存 + 磁盘），前端 store 只是投影。
