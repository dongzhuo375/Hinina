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
| `WorkspaceService.updateWorkspaceFile` | `(fileName, content) => Promise<number>` | 同步编辑器代码到后端；返回本次内容被赋予的**修订号**（与落盘事件的修订号同源，store 据此判断磁盘是否追上编辑器） |
| `WorkspaceService.setLanguage` | `(language) => Promise<Workspace>` | 设置语言（后端**立即持久化元数据**并返回更新后的 Workspace） |
| `WorkspaceService.deleteWorkspaceFile` | `(fileName) => Promise<void>` | 删除当前工作区中的文件（P62：旧代码文件清理；后端守卫拒绝删除 activeFile 与 workspace.json） |
| `WorkspaceService.onWorkspaceSaved` | `(handler) => Promise<() => void>` | 订阅后端落盘事件（显式保存 / 后台 auto-save 成功），返回取消订阅函数 |
| `workspaceService` | 单例 | 全局唯一实例 |

## 直接依赖

- `@/bridge/workspace.bridge`（七个桥接函数/订阅）
- `@/types/workspace`（仅类型）
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
- `onWorkspaceSaved` 是服务层唯一的订阅方法（其余方法都是一比一转发）：store 需要感知
  后台 auto-save 的落盘结果，而事件来源在 Rust 侧，故由服务层暴露订阅、由组合根装配。
- 无状态、无缓存：工作区的权威状态在后端（内存 + 磁盘），前端 store 只是投影。
