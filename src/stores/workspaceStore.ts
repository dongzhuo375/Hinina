import { defineStore } from 'pinia'
import type { Workspace } from '@/types/workspace'
import { workspaceService } from '@/services/workspace.service'
import { configService } from '@/services/config.service'
import { normalizeHojLanguage, sourceFileNameOf } from '@/utils/language'

/// 工作区代码文件的已知后缀（探测历史文件用；新文件名一律经 sourceFileNameOf 派生）
const CODE_FILE_EXTENSIONS = [
  '.cpp', '.cc', '.cxx', '.c', '.java', '.kt', '.py', '.go', '.rs',
  '.js', '.ts', '.cs', '.php', '.rb', '.pl', '.hs', '.sql',
] as const

export const useWorkspaceStore = defineStore('workspace', {
  state: () => ({
    workspace: null as Workspace | null,
    activeFile: null as string | null,
    code: '',
    /// 当前语言 —— 权威值为 HOJ 显示名（"C++" 等，与提交契约同源，见 utils/language）
    language: 'C++',
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
      // 工作区元数据可能残留历史 Monaco id（'cpp'），统一归一为 HOJ 显示名；
      // 未记录语言时用配置的默认语言，兜底 "C++"
      this.language = this.workspace.language
        ? normalizeHojLanguage(this.workspace.language)
        : await configService.getDefaultLanguage()
      this.isDirty = this.workspace.isDirty
      // 查找代码文件：优先取当前语言派生的文件名，其次按已知代码后缀探测
      //（兼容历史工作区中已存在的任意命名；后缀清单覆盖 HOJ 常见语言）
      const codeKeys = Object.keys(this.workspace.files)
      const derivedName = sourceFileNameOf(this.language)
      const codeFile =
        (codeKeys.includes(derivedName) ? derivedName : undefined) ??
        codeKeys.find((k) => CODE_FILE_EXTENSIONS.some((ext) => k.endsWith(ext)))
      this.code = codeFile ? this.workspace.files[codeFile] : ''
      this.activeFile = codeFile ?? derivedName
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
          // 文件名后缀必须与语言严格一致：判题端按后缀判定语言与 limits 倍率
          const fileName = sourceFileNameOf(this.language)
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
