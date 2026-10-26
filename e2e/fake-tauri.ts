import type { Page } from '@playwright/test'
import { expect } from '@playwright/test'

/**
 * E2E 的 Tauri IPC fake（M3，施工方案 §6.3「受控 Tauri internals fake」）。
 *
 * 原理：Tauri v2 的命令与事件全部经 `window.__TAURI_INTERNALS__.invoke` 单点
 * （listen 也是 `invoke('plugin:event|listen',…)`），因此用 addInitScript 注入一个
 * 自包含的 fake internals 即可承接全部 IPC——零生产代码改动、零构建开关，
 * fake 只存在于 Playwright 会话。
 *
 * 边界（与施工方案一致）：
 * - fake 必须模拟真实命令参数、错误与事件，不返回无条件的固定成功；
 * - 未实现的命令 fail-loud（console.error + Unknown 错误），禁止静默兜底；
 * - 数据全部虚构，不含任何真实凭证；IPC 调用日志只留内存，不落 artifact。
 *
 * 类型仅编译期使用；注入脚本（`fakeTauriBootstrap`）经 addInitScript 序列化执行，
 * **不得闭包任何模块级运行时值**。
 */
import type { AnnouncementPage } from '../src/types/announcement'
import type { AppConfig } from '../src/types/config'
import type { Contest, ContestProblem } from '../src/types/contest'
import type { Problem } from '../src/types/problem'
import type { ContestRankPage, RankCell } from '../src/types/rank'
import type { JudgementStatus, SubmissionPage, SubmissionRecord } from '../src/types/submission'
import type { Workspace } from '../src/types/workspace'

/** E2E fake 的初始化选项（按用例覆盖） */
export interface FakeOptions {
  /** get_session 预置会话（会话恢复场景）；缺省 null = 未登录 */
  user?: { id: string; username: string; token: string }
  /** 预置 login 拒绝（登录失败场景），形状与 AppError 序列化一致 */
  failLogin?: { variant: string; message: string }
  /** 每次提交的评测状态序列：依次返回、停在最后一个；缺省 Pending→Running→Accepted */
  judgementSequence?: JudgementStatus[]
}

/** 页面内 fake 控制面（经 window.__e2e 暴露） */
export interface FakeControl {
  calls: { cmd: string; args: Record<string, unknown> }[]
  emitEvent(name: string, payload: unknown): void
  failNext(cmd: string, variant: string, message: string): void
}

declare global {
  interface Window {
    __e2e: FakeControl
    /** @tauri-apps/api 2.11 的事件插件内部全局（_unlisten 前置调用） */
    __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener(event: string, eventId: number): void }
  }
}

/**
 * 注入页面的自包含引导脚本。fixtures 在脚本内部以动态时间构建（保证比赛处于
 * running 赛段，否则路由守卫会把已登录用户弹回登录页）。
 */
function fakeTauriBootstrap(options: FakeOptions): void {
  const nowSecs = Math.floor(Date.now() / 1000)

  // ── fixtures（全部虚构） ────────────────────────────────────────────

  const contest: Contest = {
    id: '1001',
    title: 'E2E 验证赛',
    startTime: nowSecs - 3600,
    endTime: nowSecs + 8 * 3600,
    description: '用于 Playwright E2E 的虚构比赛数据，不含真实比赛信息。',
    contestType: 0,
    status: 0,
    auth: 1,
    rankShowName: 'username',
    sealRank: false,
    sealRankTime: null,
    allowEndSubmit: false,
    oiRankScoreType: null,
  }

  const problems: ContestProblem[] = [
    { id: 1, displayId: 'A', cid: '1001', problemId: '101', displayTitle: '两数之和', ac: 3, total: 5, color: '#FF0000' },
    { id: 2, displayId: 'B', cid: '1001', problemId: '102', displayTitle: '字符串反转', ac: 1, total: 2, color: '#1E90FF' },
  ]

  // get_problem 按**比赛内展示题号**查询（视图层语义），返回的 Problem.id 是真实 pid
  const problemDetails: Record<string, Problem> = {
    A: {
      id: '101',
      title: '两数之和',
      description: '<p>给定整数数组与目标值，找出和为目标值的两个整数下标。</p>',
      inputDescription: '<p>第一行两个整数 n 与 target。</p>',
      outputDescription: '<p>一行两个下标。</p>',
      samples: [{ input: '4 9\n2 7 11 15\n', output: '0 1\n' }],
      timeLimit: 1000,
      memoryLimit: 128,
      languages: ['C++', 'Java', 'Python 3'],
    },
    B: {
      id: '102',
      title: '字符串反转',
      description: '<p>将给定字符串反转后输出。</p>',
      inputDescription: '<p>一行字符串。</p>',
      outputDescription: '<p>反转后的字符串。</p>',
      samples: [{ input: 'abc\n', output: 'cba\n' }],
      timeLimit: 2000,
      memoryLimit: 256,
      languages: ['C++', 'Java', 'Python 3'],
    },
  }

  const config: AppConfig = {
    user: { lastUsername: 'e2e_player' },
    oj: {
      active: 'HOJ',
      instances: [{ id: 'HOJ', baseUrl: 'https://e2e.invalid', enabled: true, options: {} }],
      // 必须与 contest.id 一致：登录页简报按 contestRef 从 list_contests 中筛选
      contestRef: '1001',
      contestPassword: null,
      timeoutSecs: 10,
      // E2E 快轮询（真实默认 2s/300s 上限）：缩短评测轮询等待
      pollIntervalSecs: 1,
      pollTimeoutSecs: 60,
      cacheTtlSecs: 600,
      cacheProblemStatement: true,
    },
    editor: {
      fontSize: 14,
      tabSize: 4,
      autoSave: true,
      autoSaveIntervalSecs: 10,
      defaultLanguage: 'C++',
    },
    theme: { themeName: 'light', editorTheme: 'vs' },
    layout: { sidebarWidth: 240, splitRatio: 0.48 },
  }

  // 工作区按题目真实 ID（pid）隔离，内存态跨切题保留（对应 Rust 磁盘 + 内存双层）
  const initialCode: Record<string, string> = {
    '101': '#include <bits/stdc++.h>\nusing namespace std;\n\nint main() {\n    return 0;\n}\n',
    '102': '// B 题工作区初始代码\n',
  }
  const workspaces = new Map<string, Workspace>()
  let lastWorkspaceId: string | null = null
  let revisionCounter = 100

  function getWorkspace(problemId: string): Workspace {
    let ws = workspaces.get(problemId)
    if (!ws) {
      ws = {
        id: `ws-${problemId}`,
        contestId: contest.id,
        problemId,
        rootPath: `/e2e/workspace/${problemId}`,
        files: { 'main.cpp': initialCode[problemId] ?? '' },
        activeFile: 'main.cpp',
        language: '',
        isDirty: false,
        createdAt: nowSecs,
        updatedAt: nowSecs,
      }
      workspaces.set(problemId, ws)
    }
    return ws
  }

  /**
   * 工作区变更类命令的目标 pid：必须已有工作区经 load_workspace 加载。
   * 真实后端的这些命令作用于「当前工作区」，没有工作区会被 Rust 侧拒绝——
   * fake 若以空 id 兜底建出幻影工作区，前端在真实后端会失败的调用就假绿了
   * （评审二轮：本文件自身的 fail-loud 纪律同样适用于这条边界）。
   */
  function requireLoadedProblemId(): string {
    if (!lastWorkspaceId) throw { Workspace: 'fake: 尚无已加载的工作区（先 load_workspace）' }
    return lastWorkspaceId.replace('ws-', '')
  }

  const announcements = [
    { id: '701', title: '比赛须知', content: '请勿交流题目解法。', author: 'admin', createdAt: nowSecs - 1800, updatedAt: nowSecs - 1800 },
  ]

  const emptyCell: RankCell = {
    errorNum: 0, tryNum: null, isAc: false, isFirstAc: false, acTime: null, isAfterContest: false, score: null,
  }
  const rankPage: ContestRankPage = {
    records: [{
      rank: 1, uid: '1', username: 'e2e_player', realname: '', nickname: '', school: '', gender: '', avatar: '',
      ac: 0, total: 0, totalTime: 0, totalScore: null,
      submissionInfo: { A: { ...emptyCell } },
      timeInfo: {},
    }],
    total: 1, size: 20, current: 1, pages: 1,
  }

  // ── 会话内提交与评测序列 ────────────────────────────────────────────

  interface SessionSubmission {
    record: SubmissionRecord
    sequence: JudgementStatus[]
    seqIndex: number
  }
  const submissions: SessionSubmission[] = []
  let submitSeq = 9000

  // ── 调用记录 / 一次性错误注入 / 事件注册表 ──────────────────────────

  const calls: { cmd: string; args: Record<string, unknown> }[] = []
  const failQueue = new Map<string, { variant: string; message: string }[]>()
  if (options.failLogin) {
    failQueue.set('login', [options.failLogin])
  }

  // 会话共享状态：get_session / login / logout / validate_session 读写同一份，
  // 使「登录后刷新恢复会话」「登出后 get_session 为空」可被真实表达
  //（评审 P3：此前三命令无共享状态，会话语义只能靠预置 user 绕开）
  let currentUser: { id: string; username: string; token: string } | null = options.user ?? null

  // 必填参数表（键名为前端 invoke 的 camelCase 键）。取证自 Rust 命令签名的
  // 非 Option 参数：auth_cmd login(username,password)、workspace_cmd
  // load_workspace(contest_id,problem_id)/update_workspace_file(file_name,content)/
  // set_workspace_language(language)/delete_workspace_file(file_name)、
  // submission_cmd submit_code(contest_id,problem_id,display_id,language,source_code)/
  // get_judgement(submission_id)/list_contest_submissions(contest_id)、
  // problem_cmd get_problem(contest_id,problem_id)/get_user_problem_status(contest_id,problem_ids)/
  // get_contest_problem_limits(contest_id,display_ids)、
  // contest_cmd get_contest_rank(contest_id)/list_contest_announcements(contest_id)/
  // get_read_announcement_ids(contest_id)/mark_announcements_read(contest_id,ids)、
  // config_cmd update_config(config)。缺参时真实 Tauri 运行时直接拒绝——
  // fake 同样拒绝（裸字符串，与 Tauri 的参数错误形态一致），避免参数回归假绿。
  const REQUIRED_ARGS: Record<string, string[]> = {
    login: ['username', 'password'],
    get_problem: ['contestId', 'problemId'],
    get_user_problem_status: ['contestId', 'problemIds'],
    get_contest_problem_limits: ['contestId', 'displayIds'],
    get_contest_rank: ['contestId'],
    list_contest_announcements: ['contestId'],
    get_read_announcement_ids: ['contestId'],
    mark_announcements_read: ['contestId', 'ids'],
    submit_code: ['contestId', 'problemId', 'displayId', 'language', 'sourceCode'],
    get_judgement: ['submissionId'],
    list_contest_submissions: ['contestId'],
    load_workspace: ['contestId', 'problemId'],
    update_workspace_file: ['fileName', 'content'],
    set_workspace_language: ['language'],
    delete_workspace_file: ['fileName'],
    update_config: ['config'],
  }
  const VEC_ARGS = new Set(['problemIds', 'displayIds', 'ids'])

  function rejectMissingArg(cmd: string, key: string): never {
    // 形态对齐 Tauri 参数错误（裸字符串，bridge 的 parseAppError 按纯消息处理）
    throw `invalid args \`${key}\` for command \`${cmd}\`: missing required key`
  }

  function assertRequiredArgs(cmd: string, args: Record<string, unknown> | undefined): void {
    const required = REQUIRED_ARGS[cmd]
    if (!required) return
    for (const key of required) {
      const value = args?.[key]
      if (value === undefined || value === null) rejectMissingArg(cmd, key)
      if (VEC_ARGS.has(key) && !Array.isArray(value)) rejectMissingArg(cmd, key)
    }
  }

  let callbackSeq = 0
  const callbacks = new Map<number, { cb: (event: unknown) => void; once: boolean }>()
  const eventListeners = new Map<string, Set<number>>()

  function emitEvent(name: string, payload: unknown): void {
    const ids = eventListeners.get(name)
    if (!ids) return
    for (const id of [...ids]) {
      const entry = callbacks.get(id)
      if (!entry) continue
      if (entry.once) {
        ids.delete(id)
        callbacks.delete(id)
      }
      entry.cb({ event: name, id, payload })
    }
  }

  // ── 命令表（缺命令 fail-loud，禁止静默兜底） ────────────────────────

  async function invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
    calls.push({ cmd, args: args ?? {} })

    const queue = failQueue.get(cmd)
    if (queue && queue.length > 0) {
      const { variant, message } = queue.shift()!
      // 形状与 AppError 序列化一致：{ Variant: message }（bridge 的 parseAppError 单键对象路径）
      throw { [variant]: message }
    }

    switch (cmd) {
      case 'get_session':
        return currentUser ? structuredClone(currentUser) : null
      case 'login': {
        currentUser = {
          id: '1',
          username: String(args?.username),
          token: 'e2e-session-token',
        }
        return structuredClone(currentUser)
      }
      case 'logout':
        currentUser = null
        return null
      case 'validate_session':
        return currentUser ? 'valid' : 'invalid'
      case 'get_config':
        return structuredClone(config)
      case 'update_config':
        Object.assign(config, structuredClone(args?.config))
        return null
      case 'reload_config':
        return null
      case 'switch_oj':
        return null
      case 'list_contests':
        return structuredClone([contest])
      case 'load_configured_contest':
        return structuredClone({ contest, problems })
      case 'get_problem': {
        const detail = problemDetails[String(args?.problemId ?? '')]
        if (!detail) throw { Problem: `fake 未找到题目 ${String(args?.problemId)}` }
        return structuredClone(detail)
      }
      case 'get_user_problem_status': {
        // 0=未提交 / 1=已AC / 2=尝试过，由会话内提交状态推导
        const map: Record<string, number> = {}
        for (const s of submissions) {
          const pid = s.record.pid
          if (s.record.status === 'Accepted') map[pid] = 1
          else if (map[pid] !== 1) map[pid] = 2
        }
        return map
      }
      case 'get_contest_problem_limits':
        return problems.map((p) => ({
          displayId: p.displayId,
          timeLimit: problemDetails[p.displayId]?.timeLimit ?? 1000,
          memoryLimit: problemDetails[p.displayId]?.memoryLimit ?? 128,
        }))
      case 'get_contest_rank':
        return structuredClone(rankPage)
      case 'load_workspace': {
        const ws = getWorkspace(String(args?.problemId ?? ''))
        lastWorkspaceId = ws.id
        return structuredClone(ws)
      }
      case 'save_workspace': {
        emitEvent('workspace-saved', { workspaceId: lastWorkspaceId, revision: revisionCounter, auto: false })
        return null
      }
      case 'current_workspace':
        return lastWorkspaceId ? structuredClone(getWorkspace(lastWorkspaceId.replace('ws-', ''))) : null
      case 'update_workspace_file': {
        const ws = getWorkspace(requireLoadedProblemId())
        ws.files[String(args?.fileName ?? 'main.cpp')] = String(args?.content ?? '')
        ws.updatedAt = Math.floor(Date.now() / 1000)
        revisionCounter += 1
        return revisionCounter
      }
      case 'set_workspace_language': {
        const ws = getWorkspace(requireLoadedProblemId())
        ws.language = String(args?.language ?? '')
        return structuredClone(ws)
      }
      case 'delete_workspace_file': {
        const ws = getWorkspace(requireLoadedProblemId())
        delete ws.files[String(args?.fileName ?? '')]
        return null
      }
      case 'submit_code': {
        submitSeq += 1
        const id = String(submitSeq)
        const displayId = String(args?.displayId ?? '')
        submissions.push({
          record: {
            submitId: id,
            pid: String(args?.problemId ?? ''),
            displayPid: `HOJ-${contest.id}-${String(args?.problemId ?? '')}`,
            title: problemDetails[displayId]?.title ?? '',
            displayId,
            username: currentUser?.username ?? 'anonymous',
            submitTime: Math.floor(Date.now() / 1000),
            status: 'Pending',
            timeMs: 0,
            memoryKb: 0,
            score: null,
            length: String(args?.sourceCode ?? '').length,
            language: String(args?.language ?? ''),
          },
          sequence: options.judgementSequence ?? ['Pending', 'Running', 'Accepted'],
          seqIndex: 0,
        })
        return id
      }
      case 'get_judgement': {
        const entry = submissions.find((s) => s.record.submitId === String(args?.submissionId ?? ''))
        if (!entry) throw { Submission: `fake 无此提交 ${String(args?.submissionId)}` }
        if (entry.seqIndex < entry.sequence.length) {
          entry.record.status = entry.sequence[entry.seqIndex]
          entry.seqIndex += 1
          if (entry.record.status === 'Accepted') {
            entry.record.timeMs = 12
            entry.record.memoryKb = 2048
          }
        }
        return {
          status: entry.record.status,
          score: entry.record.status === 'Accepted' ? 100 : 0,
          timeMs: entry.record.timeMs,
          memoryKb: entry.record.memoryKb,
          errorMessage: null,
        }
      }
      case 'list_contest_submissions': {
        // HOJ 状态码（utils/submission STATUS_OPTIONS）→ 变体名
        const statusMap: Record<number, JudgementStatus> = {
          0: 'Accepted', [-1]: 'WrongAnswer', 1: 'TimeLimitExceeded', 2: 'MemoryLimitExceeded',
          3: 'RuntimeError', [-2]: 'CompilationError', [-3]: 'PresentationError', 8: 'PartiallyAccepted',
          5: 'Pending', 6: 'Compiling', 7: 'Running',
        }
        let filtered = submissions.map((s) => s.record)
        const displayFilter = args?.problemDisplayId
        if (typeof displayFilter === 'string' && displayFilter !== '') {
          filtered = filtered.filter((r) => r.displayId === displayFilter)
        }
        if (typeof args?.status === 'number' && args.status in statusMap) {
          filtered = filtered.filter((r) => r.status === statusMap[args.status as number])
        }
        const page = Math.max(1, Number(args?.currentPage ?? 1))
        const size = Math.max(1, Number(args?.limit ?? 20))
        return {
          records: filtered.slice((page - 1) * size, page * size),
          total: filtered.length,
          size,
          current: page,
          pages: Math.ceil(filtered.length / size),
        } satisfies SubmissionPage
      }
      case 'list_contest_announcements':
        return {
          records: announcements,
          total: announcements.length,
          size: Math.max(1, Number(args?.limit ?? 20)),
          current: Math.max(1, Number(args?.currentPage ?? 1)),
          pages: 1,
        } satisfies AnnouncementPage
      case 'get_read_announcement_ids':
        return []
      case 'mark_announcements_read':
        return null
      default:
        console.error(`[fake-tauri] 未实现的命令: ${cmd}`, args)
        throw { Unknown: `fake-tauri: 未实现的命令 ${cmd}` }
    }
  }

  // 事件桥：listen/unlisten 与窗口命令在命令表外单独分派（其余命令走上表）。
  // listen 的 args.handler 已是 transformCallback 返回的回调 id
  const dispatch = async (cmd: string, args?: Record<string, unknown>): Promise<unknown> => {
    if (cmd === 'plugin:event|listen') {
      const eventId = Number(args?.handler)
      const event = String(args?.event ?? '')
      if (!eventListeners.has(event)) eventListeners.set(event, new Set())
      eventListeners.get(event)!.add(eventId)
      calls.push({ cmd, args: args ?? {} })
      return eventId
    }
    if (cmd === 'plugin:event|unlisten') {
      eventListeners.get(String(args?.event ?? ''))?.delete(Number(args?.eventId))
      calls.push({ cmd, args: args ?? {} })
      return null
    }
    if (cmd.startsWith('plugin:window|')) {
      calls.push({ cmd, args: args ?? {} })
      return null
    }
    // 参数校验先于错误注入与命令体（对齐 Tauri：反序列化先于命令执行）
    assertRequiredArgs(cmd, args)
    return invoke(cmd, args)
  }

  ;(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    metadata: {
      currentWindow: { label: 'main' },
      currentWebview: { label: 'main' },
    },
    transformCallback(cb: (event: unknown) => void, once = false): number {
      callbackSeq += 1
      callbacks.set(callbackSeq, { cb, once })
      return callbackSeq
    },
    unregisterCallback(id: number): void {
      callbacks.delete(id)
    },
    invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
      return dispatch(cmd, args)
    },
  }

  // ── 控制面（用例断言与事件/错误注入入口） ───────────────────────────

  ;(window as unknown as { __e2e: FakeControl }).__e2e = {
    calls,
    emitEvent,
    failNext(cmd: string, variant: string, message: string): void {
      if (!failQueue.has(cmd)) failQueue.set(cmd, [])
      failQueue.get(cmd)!.push({ variant, message })
    },
  }

  // @tauri-apps/api 2.11.x 的 _unlisten（event.js:43）在 invoke 之前先调
  // window.__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener —— 缺了这个全局，
  // 任何 unlisten 都会先抛 TypeError（评审 P2：当前用例未触发，第一个卸载/注销类
  // spec 必撞）。接到事件注册表的清理上（后续 invoke 的 unlisten 分支幂等）。
  ;(window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener(event: string, eventId: number): void {
      eventListeners.get(String(event))?.delete(Number(eventId))
    },
  }
}

/** 在页面导航前注入 fake internals（addInitScript 时序：先于应用脚本执行） */
export async function installFakeTauri(page: Page, options: FakeOptions = {}): Promise<void> {
  await page.addInitScript(fakeTauriBootstrap, options)
}

/** 统计某命令的调用次数（断言去重/轮询次数用） */
export async function callCount(page: Page, cmd: string): Promise<number> {
  return page.evaluate((c) => window.__e2e.calls.filter((call) => call.cmd === c).length, cmd)
}

/** 读取页面内 fake 控制面 */
export async function fakeControl(page: Page): Promise<FakeControl> {
  return page.evaluate(() => window.__e2e)
}

/**
 * 登录并进入题目总览（四个核心场景共用的进场路径）。
 * 账号密码为虚构值——fake 不校验凭证内容，只认 failLogin 注入。
 */
export async function loginAndEnterContest(page: Page, options: FakeOptions = {}): Promise<void> {
  await installFakeTauri(page, options)
  await page.goto('/')
  await expect(page.getByText('登录参赛系统')).toBeVisible()
  await page.locator('#contest-username').fill('e2e_player')
  await page.locator('#contest-password').fill('e2e-not-a-real-password')
  await page.getByRole('button', { name: '登录进入赛场' }).click()
  await page.waitForURL('**/contest/problems')
  await expect(page.getByText('题目总览')).toBeVisible()
}
