import { defineStore } from 'pinia'
import type { Workspace } from '@/types/workspace'
import { workspaceService } from '@/services/workspace.service'
import { configService } from '@/services/config.service'
import { normalizeHojLanguage, SOURCE_FILE_EXTENSIONS, hojLanguageOfFileName, sourceFileNameOf } from '@/utils/language'
import { createLogger } from '@/utils/logger'

const log = createLogger('workspaceStore')

/// 工作区代码文件探测后缀 = utils/language 识别面唯一来源（新文件名一律经 sourceFileNameOf 派生）
const CODE_FILE_EXTENSIONS = SOURCE_FILE_EXTENSIONS

/// 代码同步防抖间隔：把编辑器内容推送到后端**内存**（不落盘）
const SYNC_DEBOUNCE_MS = 2_000

/// 防抖句柄：模块级普通变量，不放进响应式 state（见 `utils/polling` 的句柄约定）
let syncTimer: ReturnType<typeof setTimeout> | null = null

/// 最近一次被接受的落盘修订号（模块级副作用句柄，不进响应式系统）。
///
/// 后端 `revision` 是**全局单调计数器**（工作区每次内容改动 +1），因此只需记一个
/// 标量：小于等于它的落盘事件一律视为重复或过期，幂等跳过 —— 落盘事实只能被
/// 更新的修订号推进，不能被回退。
///
/// 登出/切换账号时由 `cancelPendingSync` 归零（新会话与旧会话的修订号不可比）。
let lastPersistedRevision = 0

/// 最近一次成功推送到后端内存的修订号（模块级副作用句柄，不进响应式系统）。
///
/// 与 `lastPersistedRevision` 配对使用：落盘事件的修订号**落后于**它时，
/// 说明磁盘还没追上编辑器已推送的最新内容（后端内存里有比磁盘新的改动），
/// 不能清除脏标记 —— 否则指示器会短暂显示「已自动备份」而磁盘落后于编辑器。
///
/// 登出/切换账号时同样由 `cancelPendingSync` 归零。
let lastPushedRevision = 0

/**
 * 工作区 store —— 代码的「内存 → 磁盘」两级状态机。
 *
 * 落盘语义（debounce-to-memory）：编辑器改动先经 2 秒防抖推送到后端**内存**
 * （`update_workspace_file` 不写盘），磁盘写入由后端 auto-save 周期与显式
 * `save_workspace` 负责；后者由前端在**切题 / 失焦 / 关窗**时编排 —— 这三处
 * 是内存副本可能被替换或进程可能退出的时刻。因此：
 * - `syncPending`：有改动尚未推送到后端内存（防抖窗口内）
 * - `isDirty`：有改动尚未落盘（磁盘真值落后于编辑器）—— 驱动「编辑中…/已自动备份」
 *
 * `isDirty` 的清除有两个来源：显式落盘成功（`saveWorkspace`）与后端落盘事件
 * （`markPersisted`，含后台 auto-save）。二者都是「内容确已在磁盘上」的确证，
 * 因此指示器不会再出现「已备份」却其实没写盘的情况。
 */
export const useWorkspaceStore = defineStore('workspace', {
  state: () => ({
    workspace: null as Workspace | null,
    activeFile: null as string | null,
    code: '',
    /// 当前语言 —— 权威值为 HOJ 显示名（"C++" 等，与提交契约同源，见 utils/language）
    language: 'C++',
    /// 有改动尚未落盘（磁盘真值落后于编辑器）
    isDirty: false,
    /// 有改动尚未推送到后端内存（防抖窗口内）
    syncPending: false,
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
      // 本方法整体替换 code / language / workspace：在途的未推送改动必须先推送，
      // 否则加载结果会覆盖 store.code，随后防抖回调再把**旧内容**推给后端 ——
      // 「敲键 → 切视图 → 立刻切回」会因此静默丢掉最后一次编辑
      await this.flushPendingSync()

      this.workspace = await workspaceService.loadWorkspace(contestId, problemId)
      // 工作区元数据可能残留历史 Monaco id（'cpp'），统一归一为 HOJ 显示名；
      // 未记录语言时用配置的默认语言，兜底 "C++"
      const metaLanguage = this.workspace.language
        ? normalizeHojLanguage(this.workspace.language)
        : await configService.getDefaultLanguage()

      // 当前代码文件以 `activeFile` 为**权威源**（后端已持久化到 workspace.json）。
      // 回退链只服务于历史工作区（meta 无该字段）：当前语言派生名 → 已知代码后缀探测。
      //
      // 为什么不只按语言派生：语言切换后旧文件仍留在 `files` 里，而 `files` 来自
      // Rust HashMap 的序列化、**键序不稳定** —— 靠「探测第一个匹配后缀」可能加载出
      // 「旧语言代码 + 新语言元数据」的组合（提交即 CE，高亮也不符）。
      const codeKeys = Object.keys(this.workspace.files)
      const recorded = this.workspace.activeFile
      const derivedName = sourceFileNameOf(metaLanguage)
      const activeFile =
        (recorded && codeKeys.includes(recorded) ? recorded : undefined) ??
        (codeKeys.includes(derivedName) ? derivedName : undefined) ??
        codeKeys.find((k) => CODE_FILE_EXTENSIONS.some((ext) => k.endsWith(ext))) ??
        derivedName

      this.activeFile = activeFile
      this.code = this.workspace.files[activeFile] ?? ''

      // 语言与代码文件扩展名矛盾时**以文件为准**：判题端按后缀判定语言与 limits
      // 倍率，元数据说 Java 而文件是 main.cpp 时按 Java 提交必然 CE。
      const implied = hojLanguageOfFileName(activeFile)
      this.language = implied ?? metaLanguage
      if (implied && normalizeHojLanguage(metaLanguage) !== implied) {
        log.warn(
          `工作区语言元数据（${metaLanguage}）与代码文件（${activeFile}）不一致，已以文件为准`,
        )
      }

      this.isDirty = this.workspace.isDirty
      this.syncPending = false
    },

    /**
     * 保存当前工作区到磁盘（切题 / 失焦 / 关窗 / 手动保存的入口）。
     *
     * 顺序有意：**先把在途改动推进后端内存，再落盘** —— 反了会把旧内容写进磁盘。
     */
    async saveWorkspace() {
      const pushed = await this.flushPendingSync()
      await workspaceService.saveWorkspace()
      // 只有「推送成功 **且** 推送/落盘期间没有新改动」才能清脏：
      // - 推送失败 → 内容未进后端；
      // - 期间又落键（syncPending）→ 最新改动连后端内存都还没到，
      //   此时清脏会显示「已自动备份」而磁盘落后于编辑器（假 clean）
      if (pushed && !this.syncPending) this.isDirty = false
    },

    /** 更新编辑器内代码（标记脏状态 + 防抖推送到后端内存） */
    updateCode(code: string) {
      this.code = code
      this.isDirty = true
      this.syncPending = true
      this.scheduleSync()
    },

    /** 防抖调度：2 秒无操作后把代码推送到后端内存 */
    scheduleSync() {
      if (syncTimer) clearTimeout(syncTimer)
      syncTimer = setTimeout(() => {
        void this.flushPendingSync()
      }, SYNC_DEBOUNCE_MS)
    },

    /**
     * 立即把在途改动推送到后端内存（取消防抖窗口）。
     *
     * 返回是否成功（无在途改动视为成功）。失败只记录日志、保留 `syncPending`
     * 供下次重试 —— 调用方（切题 / 关窗）不应被一次 IPC 失败阻断。
     *
     * **推送期间又有新改动时不清 `syncPending`**：本次推送的是调用时刻的内容，
     * 若期间用户继续敲键（`updateCode` 已排定新的防抖），清掉标记会让新内容既
     * 不被本次推送携带、又被下次防抖（`syncPending === false` 直接短路）跳过 ——
     * 新内容永远到不了后端。
     */
    async flushPendingSync(): Promise<boolean> {
      if (syncTimer) {
        clearTimeout(syncTimer)
        syncTimer = null
      }
      if (!this.syncPending) return true

      // 文件名以 `activeFile` 为权威源：写入必须落到「代码加载自的那个文件」，
      // 不能每次重新按语言派生 —— 那会让读写锚定到不同文件（P62）。
      // 回退仅用于 activeFile 尚未建立的极早时刻。
      const fileName = this.activeFile ?? sourceFileNameOf(this.language)
      const content = this.code
      try {
        const revision = await workspaceService.updateWorkspaceFile(fileName, content)
        if (typeof revision === 'number') lastPushedRevision = revision
        if (this.code === content) this.syncPending = false
        // 落盘事件可能在推送在途时已到达：markPersisted 因 syncPending 推迟清脏，
        // 而后端已 clean、后续 auto-save tick 不再发事件，指示器会卡在「编辑中…」。
        // 此处补判 —— 落盘水位追上本次推送的修订号，说明磁盘确已包含编辑器内容。
        // 注意方向：必须水位 ≥ 推送（磁盘追上编辑器）；推送 ≥ 水位时磁盘还落后，
        // 提前清脏就是假「已自动备份」。
        if (!this.syncPending && typeof revision === 'number' && lastPersistedRevision >= revision) {
          this.isDirty = false
        }
        return true
      } catch (e) {
        log.error('代码同步失败（内容仍留在编辑器，稍后重试）:', e)
        return false
      }
    },

    /**
     * 后端确认落盘（`workspace-saved` 事件：显式保存或后台 auto-save）。
     *
     * 只在事件到达时清除脏标记：后端在写盘失败、或快照之后又有新改动时**不发布**
     * 该事件，因此这里的清除等价于「最新内容确已在磁盘上」。
     *
     * `workspaceId` 用于过滤过期事件：切题前保存旧工作区会发布 `Saved`，该事件
     * 可能在新工作区已加载（甚至已编辑）之后才送达，按 id 过滤避免误清新工作区的脏标记。
     *
     * `revision` 用于**幂等**处理：同一事件重复送达、或旧修订号的事件晚于新修订号
     * 到达时，直接跳过 —— 落盘事实只能被更新的修订号推进，不能被回退。
     * 此外，事件修订号**落后于最近一次推送**（`lastPushedRevision`）时同样不清脏：
     * 该落盘快照早于编辑器已推送的最新内容，磁盘还没追上编辑器。
     * 事件在推送在途到达（`syncPending` 推迟清脏）的场景由 `flushPendingSync`
     * 在推送返回后按「水位 ≥ 推送」补判，避免指示器卡住。
     */
    markPersisted(workspaceId?: string, revision?: number) {
      if (workspaceId && this.workspace && this.workspace.id !== workspaceId) return

      if (revision !== undefined) {
        if (revision <= lastPersistedRevision) return
        lastPersistedRevision = revision
        // 落盘快照早于最近一次推送：磁盘落后于编辑器，脏标记保持
        if (revision < lastPushedRevision) return
      }

      if (this.syncPending) return // 事件到达后又有新改动：仍需落盘
      this.isDirty = false
    },

    /** 取消尚未触发的防抖同步（登出/切换账号时调用，避免向已失效会话写入代码） */
    cancelPendingSync() {
      if (syncTimer) {
        clearTimeout(syncTimer)
        syncTimer = null
      }
      this.syncPending = false
      // 登出/切换账号后修订号不再可比（新会话与旧会话的落盘事实无关）
      lastPersistedRevision = 0
      lastPushedRevision = 0
    },

    /**
     * 切换编辑器语言。
     *
     * 本地立即生效（乐观更新，UI 不等待 IPC），同时把语言持久化到后端元数据 ——
     * 语言不属于任何代码文件，防抖同步（updateWorkspaceFile）带不上它，
     * 不落盘就会在切题/重启后退回默认语言，导致用错语言提交。
     * 持久化失败只记录日志、不回滚本地选择：阻断切换比丢失持久化更影响比赛。
     *
     * 语言本身由后端**立即落盘**，因此不计入「未落盘的代码改动」（不置 isDirty）。
     * 派生文件名随之变化时（如 C++ → Java），把 `activeFile` 切到新文件名并把当前
     * 代码推送过去（后端 `update_file` 会一并把它记为当前文件并持久化）——
     * 判题端按后缀判语言，代码必须落在与语言一致的扩展名上。
     *
     * **已知遗留（P62 未闭合部分）**：旧扩展名的文件不会被删除，工作区里会短暂
     * 并存两个代码文件；清理旧文件需要「按文件删除」能力与「旧文件非空时提示选手
     * 确认」的交互，属独立改造，不在本轮。
     */
    changeLanguage(lang: string) {
      if (this.language === lang) return
      const previousFile = sourceFileNameOf(this.language)
      this.language = lang
      workspaceService.setLanguage(lang).catch((e) => {
        log.error('语言持久化失败（切题或重启后可能退回默认语言）:', e)
      })

      const nextFile = sourceFileNameOf(lang)
      if (nextFile !== previousFile) {
        // 权威源先跟上，再推送 —— 否则 flush 会把代码写回旧文件名
        this.activeFile = nextFile
        this.syncPending = true
        void this.flushPendingSync()
      }
    },
  },
})

/**
 * 安装后端落盘事件订阅 —— 仅应在组合根（`main.ts`）调用一次。
 *
 * 「已自动备份」必须反映磁盘真值，而后台 auto-save 由 Rust 触发；事件桥是
 * 前端感知它的唯一途径。订阅失败只降级指示器（「编辑中…」会一直显示到下次
 * 显式落盘），不影响保存本身。
 */
export function installWorkspacePersistenceListener(): void {
  void workspaceService
    .onWorkspaceSaved((payload) => {
      useWorkspaceStore().markPersisted(payload.workspaceId, payload.revision)
      log.debug(`工作区已落盘（${payload.auto ? 'auto-save' : '显式保存'}）: ${payload.workspaceId}`)
    })
    .catch((e) => {
      log.error('工作区落盘事件订阅失败，「已自动备份」指示将不更新:', e)
    })
}
