# workspace（工作区跨端契约类型）

> 源文件：`src/types/workspace.ts`

## 职责

工作区实体的跨端契约类型，与 Rust `core::entity::workspace::Workspace` 对齐（camelCase 序列化）。

## 核心类型/函数

| 名称 | 形状 | 关键语义 |
|------|------|----------|
| `Workspace` | `{ id, contestId, problemId, rootPath, files: Record<string, string>, language, isDirty, createdAt, updatedAt }` | `id` 含随机后缀防碰撞；`files` 为文件名 → 内容的内存快照（代码文件按后缀识别，见 workspaceStore.loadWorkspace）；`language` 为 **HOJ 显示名**（"C++" 等，权威值域见 `utils/language`；新建时后端为空串，前端经 `normalizeHojLanguage` 归一历史 Monaco id 并回退配置默认语言）；`isDirty` 后端脏标记；`createdAt`/`updatedAt` 为 **UTC 毫秒级时间戳**（updated_at 用于崩溃恢复时判断最近活跃工作区） |

## 直接依赖

无（纯类型声明文件）

## 被依赖

- `bridge/workspace.bridge.ts`、`services/workspace.service.ts`、`stores/workspaceStore.ts`（均仅类型引用）

## 逻辑流程

无（纯类型定义）。

设计要点：

- **时间戳单位是毫秒**——与比赛实体（`types/contest.ts`，秒）不同：Workspace 时间戳
  由 Rust 侧 `SystemTime` 直接生成（内部元数据），比赛时间来自 HOJ API（秒级约定）。
  跨类型比较时间时须先统一单位。
- `language` 属于工作区**元数据**（持久化在 workspace.json），不属于任何代码文件——
  这是 `set_workspace_language` 独立 Command 存在的根因（见 `bridge/workspace.bridge.md`
  与 Rust `service/workspace/manager.md`）。
