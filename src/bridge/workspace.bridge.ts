import { listen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Workspace } from '@/types/workspace'
import { ipcInvoke } from '@/bridge'

/** 加载工作区 */
export async function loadWorkspace(contestId: string, problemId: string): Promise<Workspace> {
  return ipcInvoke<Workspace>('load_workspace', { contestId, problemId })
}

/** 保存工作区（落盘；成功时后端会下发 `workspace-saved`） */
export async function saveWorkspace(): Promise<void> {
  return ipcInvoke<void>('save_workspace')
}

/** 获取当前工作区 */
export async function currentWorkspace(): Promise<Workspace | null> {
  return ipcInvoke<Workspace | null>('current_workspace')
}

/**
 * 更新工作区文件内容（前端编辑器同步到后端）。
 *
 * 返回本次内容被赋予的修订号：用于与落盘事件的修订号比较 ——
 * 落盘落后于推送时磁盘还没追上编辑器，不能清脏标记。
 */
export async function updateWorkspaceFile(fileName: string, content: string): Promise<number> {
  return ipcInvoke<number>('update_workspace_file', { fileName, content })
}

/**
 * 设置当前工作区的编程语言（后端立即落盘）。
 *
 * 语言不属于任何代码文件，`updateWorkspaceFile` 带不上它；
 * 不单独持久化会导致切题/重启后退回默认语言，从而用错语言提交。
 */
export async function setWorkspaceLanguage(language: string): Promise<Workspace> {
  return ipcInvoke<Workspace>('set_workspace_language', { language })
}

/**
 * 删除当前工作区中的文件（P62：旧代码文件清理）。
 *
 * 供语言切换后的旧扩展名文件清理使用：编辑器内容已复制到新文件名，
 * 旧文件删除后 save() 不再全量写出它们。后端守卫拒绝删除当前代码文件
 * （activeFile）与 workspace.json，调用方无需重复设防。
 */
export async function deleteWorkspaceFile(fileName: string): Promise<void> {
  return ipcInvoke<void>('delete_workspace_file', { fileName })
}

/** 落盘事件载荷（Rust `main.rs` 的事件桥下发） */
export interface WorkspaceSavedPayload {
  workspaceId: string
  /** 内容修订号（Rust 侧写盘时的修订号）；用于幂等处理重复/过期事件 */
  revision?: number
  /** true = 后台 auto-save，false = 显式 `save_workspace` */
  auto: boolean
}

/**
 * 订阅工作区落盘事件（前端「已自动备份」指示的唯一真相来源）。
 *
 * 后台 auto-save 由 Rust 触发，前端无从感知；只有收到本事件才表示**最新内容
 * 确已落盘**（后端在写失败或快照之后又有新改动时不发）。返回取消订阅函数。
 *
 * **本事件只驱动 UI 指示，不是真实状态来源**：真实落盘状态由 `saveWorkspace`
 * 的 IPC 返回值与磁盘真值保证；事件丢失时指示器退化为「编辑中…」直到下次显式保存。
 */
export async function onWorkspaceSaved(
  handler: (payload: WorkspaceSavedPayload) => void,
): Promise<UnlistenFn> {
  return listen<WorkspaceSavedPayload>('workspace-saved', (event) => handler(event.payload))
}
