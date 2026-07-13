/// 工作区实体，对应 Rust `core::entity::workspace::Workspace`。
export interface Workspace {
  id: string
  contestId: string
  problemId: string
  rootPath: string
  files: Record<string, string>
  language: string
  isDirty: boolean
  createdAt: number
  updatedAt: number
}
