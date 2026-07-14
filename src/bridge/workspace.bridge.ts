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
