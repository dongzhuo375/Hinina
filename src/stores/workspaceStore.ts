import { defineStore } from 'pinia'
import type { Workspace } from '@/types/workspace'
import { workspaceService } from '@/services/workspace.service'

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
      this.language = this.workspace.language || 'cpp'
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

    /** 防抖同步：2 秒无操作后将代码推送到 Rust 后端 */
    debouncedSync() {
      if (this._syncTimer) clearTimeout(this._syncTimer)
      this._syncTimer = setTimeout(() => {
        if (this.isDirty) {
          workspaceService.updateWorkspaceFile('main.cpp', this.code).catch(() => {})
        }
      }, 2000)
    },

    /** 切换编辑器语言 */
    changeLanguage(lang: string) {
      this.language = lang
      this.isDirty = true
    },
  },
})
