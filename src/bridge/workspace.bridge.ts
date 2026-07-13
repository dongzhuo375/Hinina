import type { Workspace } from '@/types/workspace'
import { ipcInvoke } from '@/bridge'

/** 加载工作区 */
export async function loadWorkspace(contestId: string, problemId: string): Promise<Workspace> {
  return ipcInvoke<Workspace>('workspace:load', { contestId, problemId })
}

/** 保存工作区 */
export async function saveWorkspace(): Promise<void> {
  return ipcInvoke<void>('workspace:save')
}

/** 获取当前工作区 */
export async function currentWorkspace(): Promise<Workspace | null> {
  return ipcInvoke<Workspace | null>('workspace:current')
}
