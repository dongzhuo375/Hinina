import { defineStore } from 'pinia'
import type { Workspace } from '@/types/workspace'
import { workspaceService } from '@/services/workspace.service'
import { configService } from '@/services/config.service'

/// 语言 → 默认文件名映射
const langFileMap: Record<string, string> = {
  c: 'main.c',
  cpp: 'main.cpp',
  java: 'Main.java',
  python: 'main.py',
}

export const useWorkspaceStore = defineStore('workspace', {
  state: () => ({
    workspace: null as Workspace | null,
    activeFile: null as string | null,
    code: '',
    language: 'cpp',
    isDirty: false,
    /// 2 秒防抖定时器句柄（非持久化状态）
    _syncTimer: null as ReturnType<typeof setTimeout> | null,
  }),

  getters: {
    /** 当前编辑器中的代码 */
    currentCode: (state) => state.code,

    /** 当前编辑器语言 */
    currentLanguage: (state) => state.language,
  },

  actions: {
    /** 加载指定比赛与题目的工作区 */
    async loadWorkspace(contestId: string, problemId: string) {
      this.workspace = await workspaceService.loadWorkspace(contestId, problemId)
      // 工作区未记录语言时用配置的默认语言（Monaco id，P55 消费落地），兜底 cpp
      this.language = this.workspace.language || (await configService.getDefaultLanguage())
      this.isDirty = this.workspace.isDirty
      // 查找代码文件（main.cpp / main.c / Main.java / main.py）
      const codeKeys = Object.keys(this.workspace.files)
      const codeFile = codeKeys.find(k => k.endsWith('.cpp') || k.endsWith('.c') || k.endsWith('.java') || k.endsWith('.py'))
      this.code = codeFile ? this.workspace.files[codeFile] : ''
      this.activeFile = codeFile ?? 'main.cpp'
    },

    /** 保存当前工作区 */
    async saveWorkspace() {
      await workspaceService.saveWorkspace()
      this.isDirty = false
    },

    /** 更新编辑器内代码（标记脏状态 + 防抖同步到后端） */
    updateCode(code: string) {
      this.code = code
      this.isDirty = true
      this.debouncedSync()
    },

    /** 防抖同步：2 秒无操作后将代码推送到 Rust 后端。*/
    debouncedSync() {
      if (this._syncTimer) clearTimeout(this._syncTimer)
      this._syncTimer = setTimeout(() => {
        if (this.isDirty) {
          const fileName = langFileMap[this.language] || 'main.cpp'
          workspaceService.updateWorkspaceFile(fileName, this.code).catch((e) => {
            console.error('[workspaceStore] 代码同步失败:', e)
          })
        }
      }, 2000)
    },

    /** 取消尚未触发的防抖同步（登出/切换账号时调用，避免向已失效会话写入代码） */
    cancelPendingSync() {
      if (this._syncTimer) {
        clearTimeout(this._syncTimer)
        this._syncTimer = null
      }
    },

    /**
     * 切换编辑器语言。
     *
     * 本地立即生效（乐观更新，UI 不等待 IPC），同时把语言持久化到后端元数据 ——
     * 语言不属于任何代码文件，防抖同步（updateWorkspaceFile）带不上它，
     * 不落盘就会在切题/重启后退回默认语言，导致用错语言提交。
     * 持久化失败只记录日志、不回滚本地选择：阻断切换比丢失持久化更影响比赛。
     */
    changeLanguage(lang: string) {
      if (this.language === lang) return
      this.language = lang
      this.isDirty = true
      workspaceService.setLanguage(lang).catch((e) => {
        console.error('[workspaceStore] 语言持久化失败（切题或重启后可能退回默认语言）:', e)
      })
    },
  },
})
