import { defineStore } from 'pinia'
import type { JudgementStatus, SubmissionRecord } from '@/types/submission'
import { submissionService } from '@/services/submission.service'
import { configService } from '@/services/config.service'
import { isTerminalStatus } from '@/utils/submission'
import { createPoller } from '@/utils/polling'
import type { Poller } from '@/utils/polling'
import { errorMessage } from '@/utils/error'
import { createLogger } from '@/utils/logger'
import { useProblemStore } from '@/stores/problemStore'
import { getSessionGeneration } from '@/stores/sessionGeneration'

const log = createLogger('submissionStore')

interface SubmissionEntry {
  id: string
  problemId: string
  status: JudgementStatus
  time?: number
  memory?: number
  submittedAt: string
  /// 失败原因（CE 编译错误 / SE / SF 时非空）。轮询拿到后写入，
  /// 供控制台条直接展示 —— 否则选手只看到「Compile Error」四个字
  errorMessage?: string | null
}

/** 仅在评测期间保留提交内容，跨提交入口拦截同一份代码的重复发送。 */
interface PendingSubmission {
  id: string
  contestId: string
  problemId: string
  displayId: string
  language: string
  sourceCode: string
}

/** 单条提交的轮询上下文：Poller 句柄 + 总超时截止时刻 + 连续失败计数 */
interface PollContext {
  poller: Poller
  deadline: number
  /// 连续失败次数。瞬时抖动应静默重试，但**一直失败必须让选手知道** ——
  /// 此前完全静默，网络断开时界面永远停在「评测中」，选手无从判断是评测慢还是断了
  failures: number
}

/**
 * submissionId → 轮询上下文。
 *
 * 定时器属于副作用句柄而非渲染状态，故放在模块作用域（不进入响应式系统），
 * 由 `stopPolling` / `stopAllPolling` 统一回收；登出时经 `stores/session` 调用后者。
 */
const pollContexts = new Map<string, PollContext>()

/// 评测轮询抖动上限：±20% 间隔（封顶 500ms）。
/// 提交后的轮询相位天然分散（选手不会同一毫秒提交），小抖动即可避免
/// 全场同时提交（如开题瞬间）后的同相位查询尖峰
const POLL_JITTER_RATIO = 0.2
const POLL_JITTER_CAP_MS = 500

/// 连续失败多少次后把错误提示到界面。
///
/// 取 3 是为了同时满足两件事：偶发丢包/服务端瞬时 5xx 仍被静默容忍（不打扰选手），
/// 而真正断网时一个轮询周期内就能给出反馈（默认节拍 1~2s，约 3~6s 可见）。
const POLL_ERROR_REPORT_THRESHOLD = 3

/// 提交历史默认页大小（与后端命令默认值一致）
const HISTORY_PAGE_SIZE = 20

/**
 * 提交 store — 本会话提交条目 + 评测收敛轮询 + 服务端提交历史。
 *
 * 轮询语义（P54 迁移到 createPoller 后的不变量）：
 * - 每条提交一个独立 Poller（递归 setTimeout + 抖动 + 重入保护），
 *   命中终态或超过总超时（deadline）自动停止；
 * - **节拍唯一归属前端**：后端 `get_judgement` 是单次查询（无内层阻塞循环），
 *   服务端请求节奏 = 本 store 的 Poller 周期，±20% 抖动真实生效；
 *   `stopPolling`（登出）即刻停发请求，不存在停不掉的在途后端循环；
 * - **刻意不配置 isPaused（页面隐藏不暂停）**：提交结果轮询是有限生命周期的
 *   收敛轮询，选手切窗口查资料回来就该看到结果；暂停只会推迟收敛、
 *   拉长「评测中」焦虑期（与榜单类无限轮询的取舍相反）；
 * - 瞬时失败不中断循环，由总超时兜底；认证类失败经 bridge 观察者触发全局
 *   会话守卫（登出清理会调用 stopAllPolling，循环随之终止）。
 */
export const useSubmissionStore = defineStore('submission', {
  state: () => ({
    /// 本会话内发起的提交（编辑器控制台条「最新记录」的本地数据源之一）
    submissions: [] as SubmissionEntry[],
    pendingSubmissions: [] as PendingSubmission[],
    isSubmitting: false,
    error: null as string | null,

    /**
     * 服务端提交历史（评测页数据源；onlyMine 由后端强制）。
     * contestId 在首次 fetchHistory 时写入，翻页/筛选复用。
     */
    history: {
      contestId: '',
      records: [] as SubmissionRecord[],
      total: 0,
      current: 1,
      pageSize: HISTORY_PAGE_SIZE,
      pages: 0,
      /// 题目筛选（比赛内展示题号 "A"/"B"…；null=全部）
      problemFilter: null as string | null,
      /// 状态筛选（HOJ 状态码；null=全部）
      statusFilter: null as number | null,
      isLoading: false,
      error: null as string | null,
    },
  }),

  getters: {
    pendingFor: (state) => {
      return (contestId: string, problemId: string, displayId: string, language: string, sourceCode: string): SubmissionEntry | null => {
        const pending = state.pendingSubmissions.find((s) => s.contestId === contestId
          && s.problemId === problemId && s.displayId === displayId
          && s.language === language && s.sourceCode === sourceCode)
        if (!pending) return null
        return state.submissions.find((s) => s.id === pending.id && !isTerminalStatus(s.status)) ?? null
      }
    },

    /** 指定题目在本会话内的最新一条提交（无则 null） */
    latestLocalFor: (state) => {
      return (problemId: string): SubmissionEntry | null => {
        for (let i = state.submissions.length - 1; i >= 0; i--) {
          if (state.submissions[i].problemId === problemId) return state.submissions[i]
        }
        return null
      }
    },
  },

  actions: {
    /**
     * 视图层提交守卫的阻断上报（评审 #14：视图经 action 操作 store 状态）。
     *
     * 三道守卫（加载中 / displayId 未配对 / 题目 ID 不一致）拒绝提交时调用，
     * 文案经控制台条现有错误面展示 —— 消除「点提交没反应」的静默失败。
     */
    reportSubmitBlocked(message: string) {
      this.error = message
    },

    /** 提交代码、记录提交条目并启动评测轮询 */
    async submitCode(
      contestId: string,
      problemId: string,
      displayId: string,
      language: string,
      sourceCode: string,
    ): Promise<string> {
      const pending = this.pendingFor(contestId, problemId, displayId, language, sourceCode)
      if (pending) return pending.id
      // Store 层同步上锁：鼠标、快捷键和快捷提交即使同一帧触发，也只能发出一个请求。
      if (this.isSubmitting) throw new Error('代码正在提交，请稍候')
      this.isSubmitting = true
      this.error = null
      // 会话代际：submit_code 在途时若发生登出/会话失效清理，迟到响应不得落库
      const sessionGen = getSessionGeneration()
      let submissionId: string
      try {
        submissionId = await submissionService.submitCode(
          contestId,
          problemId,
          displayId,
          language,
          sourceCode,
        )
        if (sessionGen !== getSessionGeneration()) {
          // 清理后才返回的旧提交：不得重建 submissions/pendingSubmissions，
          // 也不得进入下方 startPolling（评审 P1）
          return submissionId
        }
        this.submissions.push({
          id: submissionId,
          problemId,
          status: 'Pending',
          submittedAt: new Date().toISOString(),
        })
        this.pendingSubmissions.push({ id: submissionId, contestId, problemId, displayId, language, sourceCode })
      } catch (e) {
        // 会话已切换时不写错误，避免把旧会话的失败污染进新会话状态
        if (sessionGen === getSessionGeneration()) this.error = errorMessage(e, '提交失败')
        throw e
      } finally {
        // 会话切换后 $reset 已把锁复位；此刻新会话可能已有自己的提交在途，
        // 旧请求的 finally 不得把新会话的锁清掉（评审四轮 #2）
        if (sessionGen === getSessionGeneration()) this.isSubmitting = false
      }
      // 提交已成功，轮询启动与提交结果解耦（轮询参数读取失败也有兜底值）。
      // pendingSubmissions 已同步写入，初始化轮询期间仍会拦截重复发送。
      await this.startPolling(submissionId)
      return submissionId
    },

    /** 轮询评测结果并更新对应提交的状态 */
    async pollResult(submissionId: string) {
      const result = await submissionService.pollJudgement(submissionId)
      const entry = this.submissions.find((s) => s.id === submissionId)
      if (entry) {
        entry.status = result.status
        entry.time = result.timeMs
        // 内存占用同样回填，否则控制台条/提交列表只能显示耗时
        entry.memory = result.memoryKb
        // 失败原因随轮询回填：CE 的编译错误是选手改代码的唯一依据
        entry.errorMessage = result.errorMessage
      }
      // 成功即清除残留错误：轮询期间的一次瞬时失败不应永久挂在 UI 上
      this.error = null
      return result
    },

    /**
     * 启动指定提交的评测轮询：命中终态或超过总超时后自动停止。
     *
     * 幂等 —— 重复调用会先停掉该提交已有的轮询器，避免并发轮询。
     */
    async startPolling(submissionId: string) {
      // 重启轮询只替换定时器，保留该提交的重复发送保护。
      pollContexts.get(submissionId)?.poller.stop()
      pollContexts.delete(submissionId)
      const sessionGen = getSessionGeneration()
      const { intervalMs, timeoutMs } = await configService.getPollSchedule()
      // 读取配置期间会话可能已被清理（登出/切换账号），此时不再注册轮询器
      if (sessionGen !== getSessionGeneration()) return
      if (!this.submissions.some((s) => s.id === submissionId)) return

      const deadline = Date.now() + timeoutMs
      const poller = createPoller({
        task: () => this.pollOnce(submissionId),
        intervalMs,
        jitterMs: Math.min(POLL_JITTER_CAP_MS, Math.round(intervalMs * POLL_JITTER_RATIO)),
        // 不配置 isPaused：收敛轮询需在后台继续，切回窗口立即可见结果（见文件头注释）
        onError: () => {
          // 兜底：pollOnce 自己吞异常，正常不会走到这里；万一它抛了（例如
          // 依赖的 store 初始化失败）也要记一次失败，不能让提示静默失效
          this.notePollFailure(submissionId)
        },
      })
      pollContexts.set(submissionId, { poller, deadline, failures: 0 })
      poller.start()
    },

    /**
     * 记一次轮询失败：前几次静默容忍（瞬时抖动），超过阈值后提示到界面。
     *
     * 阈值之前完全静默是刻意设计（不打扰选手），但**永远静默**会让「服务端挂了」
     * 与「评测很慢」在界面上完全同形 —— 选手只能干等。
     */
    notePollFailure(submissionId: string) {
      const ctx = pollContexts.get(submissionId)
      if (!ctx) return
      ctx.failures += 1
      if (ctx.failures >= POLL_ERROR_REPORT_THRESHOLD) {
        this.error = `评测结果查询连续失败 ${ctx.failures} 次（提交 #${submissionId}），仍在重试`
      }
    },

    /** 单次轮询：终态或超时即停止；瞬时失败不中断循环，由总超时兜底 */
    async pollOnce(submissionId: string) {
      const ctx = pollContexts.get(submissionId)
      if (!ctx) return
      if (Date.now() >= ctx.deadline) {
        log.warn(`提交 ${submissionId} 评测轮询超时，已停止`)
        // 超时停止时**终态未知**（服务端可能稍后才出结果）：安全动作是把「我的题目
        // 状态」标记为过期，让总览页重拉一次 —— 否则 AC/尝试过 pill 与解题进度会一直
        // 停留在错误值，直到用户进入某题或下次提交。开场判题积压时超过 5 分钟是现实场景。
        useProblemStore().invalidateMyStatus()
        // 本地条目同步收敛为终态型 Unknown：若任其停留在 Pending（非终态），
        // EditorConsoleBar 的 serverPill 遮蔽逻辑会无限期隐藏服务端摘要里
        // 已到的终态结果，左 pill 永远卡在「Pending」转圈；置为 Unknown 后
        // 组件的终态侦听还会重拉一次摘要，让服务端结果接管展示。
        const entry = this.submissions.find((s) => s.id === submissionId)
        if (entry && !isTerminalStatus(entry.status)) entry.status = 'Unknown'
        this.stopPolling(submissionId)
        return
      }
      try {
        const result = await this.pollResult(submissionId)
        ctx.failures = 0
        if (isTerminalStatus(result.status)) {
          // 评测终结意味着「我的题目状态」可能已变（AC / 尝试过）：标记过期，
          // 由总览页在下次可见刷新时重拉一次，而不是让它每 30s 整表重拉
          useProblemStore().invalidateMyStatus()
          this.stopPolling(submissionId)
        }
      } catch {
        // 网络抖动等瞬时错误：等待下一次轮询。**计数上报在这里而不是 poller 的
        // onError** —— pollOnce 自己吞掉异常，poller 永远收不到 rejection，
        // 挂在 onError 上等于计数永不发生（错误提示静默失效）
        this.notePollFailure(submissionId)
      }
    },

    /** 停止指定提交的轮询 */
    stopPolling(submissionId: string) {
      this.pendingSubmissions = this.pendingSubmissions.filter((s) => s.id !== submissionId)
      const ctx = pollContexts.get(submissionId)
      if (ctx) {
        ctx.poller.stop()
        pollContexts.delete(submissionId)
      }
    },

    /** 停止全部轮询（登出/切换账号时调用，防止定时器脱离会话继续请求） */
    stopAllPolling() {
      this.pendingSubmissions = []
      pollContexts.forEach((ctx) => ctx.poller.stop())
      pollContexts.clear()
    },

    // ── 服务端提交历史（评测页 / 最新记录 pill 数据源） ──

    /**
     * 拉取一页提交历史（本人；筛选条件取自 history state）。
     *
     * 失败记录 history.error 并上抛，由调用方决定提示；轮询路径自行吞错。
     */
    async fetchHistory(contestId: string, page?: number) {
      // 会话代际（评审四轮 #1）：history 是 state 内的嵌套对象，`$reset()` 会整体
      // 替换它，因此迟到的旧请求写的是被替换掉的旧对象引用 —— 实际不会泄漏。
      // 守卫保留为纵深防御（若将来 history 改为模块级句柄或浅重置，这层仍成立）
      const sessionGen = getSessionGeneration()
      const h = this.history
      h.contestId = contestId
      h.isLoading = true
      h.error = null
      try {
        const result = await submissionService.listContestSubmissions({
          contestId,
          currentPage: page ?? h.current,
          limit: h.pageSize,
          problemDisplayId: h.problemFilter,
          status: h.statusFilter,
        })
        if (sessionGen !== getSessionGeneration()) return
        h.records = result.records
        h.total = result.total
        h.current = result.current
        h.pages = result.pages
      } catch (e) {
        if (sessionGen !== getSessionGeneration()) return
        h.error = errorMessage(e, '加载提交记录失败')
        throw e
      } finally {
        // 旧会话的 finally 不得改动新会话的加载态
        if (sessionGen === getSessionGeneration()) h.isLoading = false
      }
    },

    /** 设置题目筛选并回到第 1 页（displayId；null=全部题目） */
    async setHistoryProblemFilter(contestId: string, displayId: string | null) {
      this.history.problemFilter = displayId
      await this.fetchHistory(contestId, 1)
    },

    /**
     * 清空题目/状态筛选（不发起请求）。
     *
     * 与 setHistoryProblemFilter 分开：后者是「用户改筛选」语义，清空即重拉第 1 页；
     * 本方法供评测页无 `?problem=` 深链进入时清除上次访问的残留筛选，
     * 请求由调用方随后的 fetchHistory 统一发出，避免同一引导链路重复拉取。
     */
    resetHistoryFilters() {
      this.history.problemFilter = null
      this.history.statusFilter = null
    },

    /** 设置状态筛选并回到第 1 页（HOJ 状态码；null=全部状态） */
    async setHistoryStatusFilter(contestId: string, status: number | null) {
      this.history.statusFilter = status
      await this.fetchHistory(contestId, 1)
    },

    /** 翻页 */
    async setHistoryPage(contestId: string, page: number) {
      const h = this.history
      if (page < 1 || (h.pages > 0 && page > h.pages)) return
      await this.fetchHistory(contestId, page)
    },

    /**
     * 查询指定题目的最新一条提交与总数（解题页「最新记录」pill 与「提交记录 (n)」数据源）。
     *
     * 独立于 history state（不干扰评测页的列表/筛选）；无提交时 latest 为 null。
     */
    async fetchProblemSummary(
      contestId: string,
      displayId: string,
    ): Promise<{ latest: SubmissionRecord | null; total: number }> {
      const page = await submissionService.listContestSubmissions({
        contestId,
        currentPage: 1,
        limit: 1,
        problemDisplayId: displayId,
        status: null,
      })
      return { latest: page.records[0] ?? null, total: page.total }
    },
  },
})
