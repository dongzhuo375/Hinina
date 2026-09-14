import type { Workspace } from '@/types/workspace'
import { ipcInvoke } from '@/bridge'

/** 加载工作区 */
export async function loadWorkspace(contestId: string, problemId: string): Promise<Workspace> {
  return ipcInvoke<Workspace>('load_workspace', { contestId, problemId })
}

/** 保存工作区 */
export async function saveWorkspace(): Promise<void> {
  return ipcInvoke<void>('save_workspace')
}

/** 获取当前工作区 */
export async function currentWorkspace(): Promise<Workspace | null> {
  return ipcInvoke<Workspace | null>('current_workspace')
}

/** 更新工作区文件内容（前端编辑器同步到后端） */
export async function updateWorkspaceFile(fileName: string, content: string): Promise<void> {
  return ipcInvoke<void>('update_workspace_file', { fileName, content })
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
