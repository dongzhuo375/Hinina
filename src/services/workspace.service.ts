import type { Workspace } from '@/types/workspace'
import * as workspaceBridge from '@/bridge/workspace.bridge'

/**
 * 工作区服务 — 管理工作区的加载、保存与当前状态。
 */
export class WorkspaceService {
  /**
   * 加载指定比赛与题目的工作区。
   */
  async loadWorkspace(contestId: string, problemId: string): Promise<Workspace> {
    return workspaceBridge.loadWorkspace(contestId, problemId)
  }

  /**
   * 保存当前工作区。
   */
  async saveWorkspace(): Promise<void> {
    return workspaceBridge.saveWorkspace()
  }

  /**
   * 获取当前活跃的工作区（可能为 null）。
   */
  async currentWorkspace(): Promise<Workspace | null> {
    return workspaceBridge.currentWorkspace()
  }
}

export const workspaceService = new WorkspaceService()
