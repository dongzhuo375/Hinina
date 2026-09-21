/// 工作区实体，对应 Rust `core::entity::workspace::Workspace`。
export interface Workspace {
  id: string
  contestId: string
  problemId: string
  rootPath: string
  files: Record<string, string>
  /// **当前代码文件名（权威源）**：加载与写入都锚定它，`language` 只描述它的语言。
  ///
  /// `null` = 新工作区尚无代码文件，或历史工作区未记录（后端 `serde(default)`）。
  activeFile: string | null
  language: string
  isDirty: boolean
  createdAt: number
  updatedAt: number
}
