<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import CodeEditor from '@/components/editor/CodeEditor.vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import { configService } from '@/services/config.service'
import { submissionService } from '@/services/submission.service'
import { useContestStore } from '@/stores/contestStore'
import type { JudgeCase, SubmissionCases, SubmissionDetail } from '@/types/submission'
import { monacoIdOf } from '@/utils/language'
import {
  formatCodeLength,
  formatDurationHms,
  formatMemoryKb,
  isJudging,
  isTerminalStatus,
  statusAbbr,
  statusLabel,
  statusTone,
} from '@/utils/submission'
import type { StatusTone } from '@/utils/submission'
import { createPoller } from '@/utils/polling'
import type { Poller } from '@/utils/polling'
import { errorMessage } from '@/utils/error'
import { createLogger } from '@/utils/logger'

const log = createLogger('SubmissionDetailView')

/// 详情页收敛轮询抖动：±20% 间隔（封顶 500ms），与 submissionStore 同口径
const JITTER_RATIO = 0.2
const JITTER_CAP_MS = 500

const route = useRoute()
const router = useRouter()
const contestStore = useContestStore()

const submitId = computed(() => String(route.params.submitId ?? ''))

const detail = ref<SubmissionDetail | null>(null)
const cases = ref<SubmissionCases | null>(null)
/// 测试点明细加载失败不致命：降级为提示条，详情主体照常展示
const casesError = ref<string | null>(null)
const loadError = ref<string | null>(null)
const isLoading = ref(true)

/// 组件已卸载标记：异步链路落地后不得再写状态
let alive = true
/// 路由切换代际：同一组件从提交 A 复用到 B 时，A 的详情/测试点/轮询一律失效
let loadToken = 0
/// 轮询器与截止时刻是副作用句柄，放普通变量（不进响应式系统）
let poller: Poller | null = null
let deadline = 0
let pollIntervalMs = 5_000
let pollJitterMs = 500
let pollTimeoutMs = 0
/// 初次配置读取共享给首载与路由 watch；切换发生在配置在途时也只等待同一份配置
let scheduleReady: Promise<void> | null = null

const judging = computed(() => (detail.value ? isJudging(detail.value.status) : false))

/// tone → pill 样式类（与评测页同一套语义色）
const TONE_CLASSES: Record<StatusTone, string> = {
  ac: 'border-[var(--color-ac-border)] bg-[var(--color-ac-bg)] text-[var(--color-success)]',
  wa: 'border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] text-[var(--color-error)]',
  tle: 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-warning)]',
  pending: 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-info)]',
  system: 'border-[var(--border-color)] bg-[var(--bg-sidebar)] text-[var(--text-secondary)]',
  neutral: 'border-[var(--border-color)] bg-[var(--bg-sidebar)] text-[var(--text-secondary)]',
}

function toneClasses(tone: StatusTone): string {
  return TONE_CLASSES[tone] ?? TONE_CLASSES.neutral
}

/// 详情 pid 匹配到当前比赛题目时可跳解题页；匹配不到（如赛后换配置）只显示 displayPid
const matchedProblem = computed(() => {
  const pid = detail.value?.pid
  if (!pid) return null
  return contestStore.problems.find((p) => p.problemId === pid) ?? null
})

const submitTimeText = computed(() => {
  const d = detail.value
  if (!d) return ''
  return new Date(d.submitTime * 1000).toLocaleString('zh-CN', { hour12: false })
})

const contestElapsedText = computed(() => {
  const d = detail.value
  const start = contestStore.contest?.startTime
  if (!d || start == null) return ''
  return formatDurationHms(d.submitTime - start)
})

const codeLanguage = computed(() => monacoIdOf(detail.value?.language ?? ''))

// ── 测试点明细 ──

const caseList = computed<JudgeCase[]>(() => cases.value?.cases ?? [])
const acCaseCount = computed(
  () => caseList.value.filter((c) => c.status === 'Accepted').length,
)
const hasSubTasks = computed(() => (cases.value?.subTasks?.length ?? 0) > 0)
const showScoreColumn = computed(() => caseList.value.some((c) => c.score !== null))
const modeBadge = computed(() => {
  const mode = cases.value?.mode
  return mode && mode !== 'default' ? mode : ''
})

function groupPassed(group: JudgeCase[]): number {
  return group.filter((c) => c.status === 'Accepted').length
}

// ── 代码折叠卡 ──

const codeExpanded = ref(false)

// ── CE / 错误信息复制 ──

const copied = ref(false)
let copiedTimer: ReturnType<typeof setTimeout> | null = null

async function copyErrorMessage() {
  const text = detail.value?.errorMessage
  if (!text) return
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    // WebView 剪贴板 API 不可用时的兜底：隐藏 textarea + execCommand
    const ta = document.createElement('textarea')
    ta.value = text
    ta.style.position = 'fixed'
    ta.style.opacity = '0'
    document.body.appendChild(ta)
    ta.select()
    try {
      document.execCommand('copy')
    } catch (e) {
      log.warn('复制错误信息失败:', e)
    }
    document.body.removeChild(ta)
  }
  copied.value = true
  if (copiedTimer) clearTimeout(copiedTimer)
  copiedTimer = setTimeout(() => (copied.value = false), 1500)
}

// ── 数据加载与收敛轮询 ──

/// 只有当前路由、当前代际且组件仍存活时才允许异步链路写状态
function isCurrentLoad(id: string, token: number): boolean {
  return alive && token === loadToken && id === submitId.value
}

/// 首载、重试与路由参数切换的统一入口：旧提交的数据和轮询不得带入新提交
async function reload(id: string): Promise<void> {
  const token = ++loadToken
  stopPoller()
  detail.value = null
  cases.value = null
  casesError.value = null
  loadError.value = null
  isLoading.value = true
  codeExpanded.value = false
  copied.value = false
  if (copiedTimer) {
    clearTimeout(copiedTimer)
    copiedTimer = null
  }
  if (!id) {
    isLoading.value = false
    return
  }
  if (scheduleReady) await scheduleReady
  if (!isCurrentLoad(id, token)) return
  deadline = Date.now() + pollTimeoutMs
  await loadDetail(id, token)
}

/**
 * 拉取提交详情；到达终态后补拉一次测试点明细。
 *
 * 轮询周期只走本函数：评测中 `get-all-case-result` 尚无稳定结果，
 * 每周期重复拉取纯属浪费（终态判据命中后 cases 只请求一次）。
 * id/token 在发起时固定，路由切换后旧响应不得覆盖新提交。
 */
async function loadDetail(id: string, token: number): Promise<void> {
  try {
    const d = await submissionService.getSubmissionDetail(id)
    if (!isCurrentLoad(id, token)) return
    detail.value = d
    loadError.value = null
  } catch (e) {
    if (!isCurrentLoad(id, token)) return
    // 已有旧数据时瞬时失败不清空页面，只等下一周期
    if (!detail.value) {
      loadError.value = errorMessage(e, '加载提交详情失败')
    }
  }
  isLoading.value = false
  syncPoller(id, token)
  if (detail.value && isTerminalStatus(detail.value.status)) await loadCases(id, token)
}

/// 拉取测试点明细；失败不致命，降级为提示条（详情主体照常展示）
async function loadCases(id: string, token: number): Promise<void> {
  try {
    const c = await submissionService.getSubmissionCases(id)
    if (!isCurrentLoad(id, token)) return
    cases.value = c
    casesError.value = null
  } catch {
    if (!isCurrentLoad(id, token)) return
    casesError.value = '测试点明细加载失败'
  }
}

async function retry(): Promise<void> {
  await reload(submitId.value)
}

function stopPoller() {
  poller?.stop()
  poller = null
}

/// 非终态且未超总超时才轮询；到达终态或超过 deadline 立即停
function syncPoller(id: string, token: number) {
  if (!isCurrentLoad(id, token)) return
  const d = detail.value
  const shouldPoll = !!d && !isTerminalStatus(d.status) && Date.now() < deadline
  if (shouldPoll && !poller) {
    poller = createPoller({
      task: async () => {
        if (!isCurrentLoad(id, token)) return
        if (Date.now() >= deadline) {
          stopPoller()
          return
        }
        await loadDetail(id, token)
      },
      intervalMs: pollIntervalMs,
      jitterMs: pollJitterMs,
      // 不配置 isPaused：与 submissionStore 的提交收敛轮询同一取舍 ——
      // 有限生命周期，切窗口回来就该看到结果
      onError: () => {
        // 瞬时失败静默，总超时兜底
      },
    })
    poller.start()
  } else if (!shouldPoll) {
    stopPoller()
  }
}

onMounted(async () => {
  // 比赛数据用于「题目」跳转与赛时相对时间；深链直接进入时可能尚未加载，失败静默
  contestStore.whenLoaded().catch(() => {})
  scheduleReady = (async () => {
    const schedule = await configService.getPollSchedule()
    if (!alive) return
    pollIntervalMs = schedule.intervalMs
    pollJitterMs = Math.min(JITTER_CAP_MS, Math.round(schedule.intervalMs * JITTER_RATIO))
    pollTimeoutMs = schedule.timeoutMs
  })()
  await reload(submitId.value)
})

// 同一路由记录复用（/submissions/A → /submissions/B）不会重新触发 onMounted
watch(submitId, (id) => { void reload(id) })

onUnmounted(() => {
  alive = false
  stopPoller()
  if (copiedTimer) clearTimeout(copiedTimer)
})

function goBack() {
  router.push({ name: 'Submissions' })
}

function openProblem() {
  const p = matchedProblem.value
  if (!p) return
  router.push({ name: 'ProblemSolve', params: { displayId: p.displayId } })
}
</script>

<template>
  <div class="submission-detail flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-body)]">
    <!-- 顶栏：返回 + 标题 + 评测中指示 -->
    <div
      class="flex shrink-0 items-center gap-3 border-b border-[var(--border-color)] bg-[var(--bg-body)] px-8 py-5"
    >
      <button
        type="button"
        class="flex items-center gap-1.5 rounded-sm border border-[var(--border-color)] bg-[var(--bg-sidebar)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] shadow-none transition-colors hover:bg-[var(--bg-sidebar)] hover:text-[var(--text-primary)]"
        @click="goBack"
      >
        <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true">
          <path d="M10.5 19.5L3 12m0 0l7.5-7.5M3 12h18" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <span>返回评测列表</span>
      </button>
      <h1 class="min-w-0 truncate text-base font-bold tracking-tight text-[var(--text-primary)]">
        提交 <span class="font-mono">#{{ submitId }}</span>
      </h1>
      <span
        v-if="judging"
        class="flex shrink-0 items-center gap-1.5 rounded-sm border border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] px-2.5 py-1 text-xs font-medium text-[var(--color-info)]"
      >
        <svg class="h-3 w-3 animate-spin" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" aria-hidden="true">
          <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
          <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
        </svg>
        <span>评测中，自动刷新…</span>
      </span>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-8 pb-8">
      <LoadingSpinner v-if="isLoading && !detail" message="正在加载提交详情…" />

      <ErrorMessage
        v-else-if="loadError && !detail"
        :message="loadError"
        :retry="retry"
      />

      <div v-else-if="detail" class="mx-auto flex max-w-[1400px] flex-col gap-6">
        <!-- 判定横幅卡：大状态 pill + 指标条 + 元信息 -->
        <section
          class="result-summary border-y border-[var(--border-color)] bg-[var(--bg-card)] p-7"
        >
          <div class="flex flex-wrap items-center gap-4">
            <span
              class="inline-flex items-center gap-2 rounded-sm border px-4 py-2 text-base font-bold shadow-none"
              :class="[toneClasses(statusTone(detail.status)), judging ? '' : '']"
            >
              <svg
                v-if="judging"
                class="h-4 w-4 animate-spin"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                viewBox="0 0 24 24"
                aria-hidden="true"
              >
                <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
                <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
              </svg>
              <span>{{ statusLabel(detail.status) }}</span>
            </span>

            <!-- 指标条：等宽数字，逐项分隔 -->
            <div
              class="flex flex-wrap items-center gap-x-5 gap-y-2 font-mono text-xs tabular-nums"
            >
              <div class="flex items-baseline gap-1.5">
                <span class="font-sans text-xs text-[var(--text-muted)]">耗时</span>
                <span class="text-sm font-semibold text-[var(--text-primary)]">
                  {{ judging ? '-' : `${detail.timeMs} ms` }}
                </span>
              </div>
              <div class="flex items-baseline gap-1.5">
                <span class="font-sans text-xs text-[var(--text-muted)]">内存</span>
                <span class="text-sm font-semibold text-[var(--text-primary)]">
                  {{ judging ? '-' : formatMemoryKb(detail.memoryKb) }}
                </span>
              </div>
              <div v-if="detail.score !== null" class="flex items-baseline gap-1.5">
                <span class="font-sans text-xs text-[var(--text-muted)]">得分</span>
                <span class="text-sm font-semibold text-[var(--text-primary)]">{{ detail.score }}</span>
              </div>
              <div v-if="detail.oiRankScore !== null" class="flex items-baseline gap-1.5">
                <span class="font-sans text-xs text-[var(--text-muted)]">榜单计分</span>
                <span class="text-sm font-semibold text-[var(--text-primary)]">
                  {{ detail.oiRankScore }}
                </span>
              </div>
              <div class="flex items-baseline gap-1.5">
                <span class="font-sans text-xs text-[var(--text-muted)]">代码长度</span>
                <span class="text-sm font-semibold text-[var(--text-primary)]">
                  {{ formatCodeLength(detail.length) }}
                </span>
              </div>
              <div class="flex items-baseline gap-1.5">
                <span class="font-sans text-xs text-[var(--text-muted)]">语言</span>
                <span class="text-sm font-semibold text-[var(--text-primary)]">{{ detail.language }}</span>
              </div>
            </div>
          </div>

          <!-- 元信息行 -->
          <div
            class="mt-4 flex flex-wrap items-center gap-x-5 gap-y-2 border-t border-[var(--border-color)] pt-3.5 text-xs text-[var(--text-secondary)]"
          >
            <div class="flex items-center gap-1.5">
              <span class="text-[var(--text-muted)]">题目</span>
              <button
                v-if="matchedProblem"
                type="button"
                class="font-medium text-[var(--color-primary)] transition-colors hover:text-[var(--color-primary-hover)] hover:underline"
                @click="openProblem"
              >
                {{ matchedProblem.displayId }} · {{ matchedProblem.displayTitle }}
              </button>
              <span v-else class="font-mono font-medium text-[var(--text-primary)]">
                {{ detail.displayPid }}
              </span>
            </div>
            <div class="flex items-center gap-1.5">
              <span class="text-[var(--text-muted)]">提交者</span>
              <span class="font-mono font-medium text-[var(--text-primary)]">{{ detail.username }}</span>
            </div>
            <div class="flex items-center gap-1.5">
              <span class="text-[var(--text-muted)]">提交时间</span>
              <span class="font-mono tabular-nums">{{ submitTimeText }}</span>
              <span v-if="contestElapsedText" class="font-mono text-[var(--text-muted)] tabular-nums">
                (赛时 {{ contestElapsedText }})
              </span>
            </div>
            <div v-if="detail.judger" class="flex items-center gap-1.5">
              <span class="text-[var(--text-muted)]">评测机</span>
              <span class="font-mono">{{ detail.judger }}</span>
            </div>
          </div>
        </section>

        <!-- CE / 错误信息面板 -->
        <section
          v-if="detail.errorMessage"
          class="rounded-sm border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)]/60 shadow-none"
        >
          <div class="flex items-center justify-between gap-3 border-b border-[var(--color-wa-border)]/70 px-4 py-2.5">
            <div class="flex items-center gap-2 text-xs font-semibold text-[var(--color-error)]">
              <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true">
                <path d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <span>错误信息</span>
            </div>
            <button
              type="button"
              class="flex items-center gap-1.5 rounded-md border border-[var(--color-wa-border)] bg-[var(--bg-card)] px-2.5 py-1 text-xs font-medium text-[var(--color-error)] transition-colors hover:bg-[var(--color-wa-bg)]"
              @click="copyErrorMessage"
            >
              <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true">
                <path d="M15.666 3.888A2.25 2.25 0 0013.5 2.25h-3c-1.03 0-1.9.693-2.166 1.638m7.332 0c.055.194.084.4.084.612v0a.75.75 0 01-.75.75H9a.75.75 0 01-.75-.75v0c0-.212.03-.418.084-.612m7.332 0c.646.049 1.288.11 1.927.184 1.1.128 1.907 1.077 1.907 2.185V19.5a2.25 2.25 0 01-2.25 2.25H6.75A2.25 2.25 0 014.5 19.5V6.257c0-1.108.806-2.057 1.907-2.185a48.208 48.208 0 011.927-.184" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <span>{{ copied ? '已复制' : '复制' }}</span>
            </button>
          </div>
          <pre
            class="max-h-72 overflow-auto px-4 py-3 font-mono text-xs leading-relaxed whitespace-pre-wrap text-[var(--color-error)]"
          >{{ detail.errorMessage }}</pre>
        </section>

        <!-- 测试点明细 -->
        <section class="overflow-hidden rounded-sm border border-[var(--border-color)] bg-[var(--bg-card)] shadow-none">
          <div
            class="flex flex-wrap items-center justify-between gap-2 border-b border-[var(--border-color)] px-4 py-2.5"
          >
            <div class="flex items-center gap-2.5">
              <h2 class="text-sm font-bold text-[var(--text-primary)]">测试点明细</h2>
              <span
                v-if="caseList.length > 0"
                class="rounded-sm border border-[var(--border-color)] bg-[var(--bg-sidebar)] px-2.5 py-0.5 font-mono text-xs font-medium tabular-nums text-[var(--text-secondary)]"
              >
                {{ acCaseCount }}/{{ caseList.length }} 通过
              </span>
              <span
                v-if="modeBadge"
                class="rounded-sm border border-purple-200 bg-purple-50 px-2.5 py-0.5 font-mono text-xs font-medium text-purple-600"
              >
                {{ modeBadge }}
              </span>
            </div>
          </div>

          <!-- 评测中且无明细：骨架占位 -->
          <div v-if="caseList.length === 0 && !hasSubTasks && judging" class="flex flex-col gap-2 p-4">
            <div
              v-for="n in 4"
              :key="n"
              class="h-8 rounded-sm bg-[var(--bg-sidebar)]"
              :style="{ opacity: 1 - n * 0.18 }"
            ></div>
            <p class="pt-1 text-center text-xs text-[var(--text-muted)]">评测进行中，测试点结果将自动刷新…</p>
          </div>

          <!-- 终态但服务端未给明细（部分 OJ 配置隐藏测试点） -->
          <div
            v-else-if="caseList.length === 0 && !hasSubTasks"
            class="flex flex-col items-center gap-1.5 p-8 text-center"
          >
            <p class="text-sm text-[var(--text-secondary)]">
              {{ casesError ? casesError : '服务端未返回测试点明细' }}
            </p>
            <p class="text-xs text-[var(--text-muted)]">部分 OJ 配置会隐藏测试点结果，以最终判定为准</p>
          </div>

          <!-- 子任务制：按 groupNum 分组渲染 -->
          <div v-else-if="hasSubTasks" class="flex flex-col gap-4 p-4">
            <div
              v-for="group in cases?.subTasks ?? []"
              :key="group.groupNum"
              class="overflow-hidden rounded-sm border border-[var(--border-color)]"
            >
              <div
                class="flex items-center justify-between gap-2 border-b border-[var(--border-color)] bg-[var(--bg-sidebar)]/80 px-3.5 py-2"
              >
                <span class="font-mono text-xs font-semibold text-[var(--text-primary)]">
                  子任务 #{{ group.groupNum }}
                </span>
                <span class="font-mono text-xs tabular-nums text-[var(--text-secondary)]">
                  {{ groupPassed(group.cases) }}/{{ group.cases.length }} 通过
                </span>
              </div>
              <table class="w-full border-collapse text-left">
                <thead>
                  <tr class="border-b border-[var(--border-color)] font-mono text-xs tracking-wider text-[var(--text-muted)] uppercase">
                    <th class="px-3.5 py-1.5 font-semibold">#</th>
                    <th class="px-3.5 py-1.5 font-semibold">结果</th>
                    <th class="px-3.5 py-1.5 text-right font-semibold">耗时</th>
                    <th class="px-3.5 py-1.5 text-right font-semibold">内存</th>
                    <th v-if="showScoreColumn" class="px-3.5 py-1.5 text-right font-semibold">得分</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-[var(--border-color)] text-xs">
                  <tr v-for="c in group.cases" :key="c.caseId">
                    <td class="px-3.5 py-2 font-mono tabular-nums text-[var(--text-secondary)]">{{ c.seq }}</td>
                    <td class="px-3.5 py-2">
                      <span
                        class="inline-flex rounded-sm border px-2 py-0.5 font-mono text-xs font-semibold"
                        :class="toneClasses(statusTone(c.status))"
                      >
                        {{ statusAbbr(c.status) }}
                      </span>
                    </td>
                    <td class="px-3.5 py-2 text-right font-mono tabular-nums text-[var(--text-secondary)]">{{ c.timeMs }} ms</td>
                    <td class="px-3.5 py-2 text-right font-mono tabular-nums text-[var(--text-secondary)]">{{ formatMemoryKb(c.memoryKb) }}</td>
                    <td v-if="showScoreColumn" class="px-3.5 py-2 text-right font-mono tabular-nums text-[var(--text-secondary)]">
                      {{ c.score ?? '-' }}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <!-- 平铺测试点表 -->
          <table v-else class="w-full border-collapse text-left">
            <thead>
              <tr class="border-b border-[var(--border-color)] bg-[var(--bg-sidebar)]/80 font-mono text-xs tracking-wider text-[var(--text-muted)] uppercase">
                <th class="px-4 py-2 font-semibold">#</th>
                <th class="px-4 py-2 font-semibold">结果</th>
                <th class="px-4 py-2 text-right font-semibold">耗时</th>
                <th class="px-4 py-2 text-right font-semibold">内存</th>
                <th v-if="showScoreColumn" class="px-4 py-2 text-right font-semibold">得分</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-[var(--border-color)] text-xs">
              <tr v-for="c in caseList" :key="c.caseId" class="transition-colors hover:bg-[var(--bg-sidebar)]/70">
                <td class="px-4 py-2 font-mono tabular-nums text-[var(--text-secondary)]">{{ c.seq }}</td>
                <td class="px-4 py-2">
                  <span
                    class="inline-flex rounded-sm border px-2 py-0.5 font-mono text-xs font-semibold"
                    :class="toneClasses(statusTone(c.status))"
                  >
                    {{ statusAbbr(c.status) }}
                  </span>
                </td>
                <td class="px-4 py-2 text-right font-mono tabular-nums text-[var(--text-secondary)]">{{ c.timeMs }} ms</td>
                <td class="px-4 py-2 text-right font-mono tabular-nums text-[var(--text-secondary)]">{{ formatMemoryKb(c.memoryKb) }}</td>
                <td v-if="showScoreColumn" class="px-4 py-2 text-right font-mono tabular-nums text-[var(--text-secondary)]">
                  {{ c.score ?? '-' }}
                </td>
              </tr>
            </tbody>
          </table>
        </section>

        <!-- 源代码（默认折叠） -->
        <section class="overflow-hidden rounded-sm border border-[var(--border-color)] bg-[var(--bg-card)] shadow-none">
          <button
            type="button"
            class="flex w-full items-center justify-between gap-3 px-4 py-3 text-left transition-colors hover:bg-[var(--bg-sidebar)]/70"
            @click="codeExpanded = !codeExpanded"
          >
            <div class="flex items-center gap-2.5">
              <h2 class="text-sm font-bold text-[var(--text-primary)]">源代码</h2>
              <span
                class="rounded-sm border border-[var(--border-color)] bg-[var(--bg-sidebar)] px-2.5 py-0.5 font-mono text-xs font-medium text-[var(--text-secondary)]"
              >
                {{ detail.language }}
              </span>
              <span class="font-mono text-xs tabular-nums text-[var(--text-muted)]">
                {{ formatCodeLength(detail.length) }}
              </span>
            </div>
            <span class="flex items-center gap-1 text-xs font-medium text-[var(--color-primary)]">
              <span>{{ codeExpanded ? '收起' : '展开' }}</span>
              <svg
                class="h-3.5 w-3.5 transition-transform"
                :class="{ 'rotate-180': codeExpanded }"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                viewBox="0 0 24 24"
                aria-hidden="true"
              >
                <path d="M6 9l6 6 6-6" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </span>
          </button>
          <!--
            外层必须**自身就是 flex 容器**：`CodeEditor` 根节点是 `flex-1` 的 flex 项，
            在普通块级父容器里 `flex-1` 不生效、高度退回内容高度 —— 实测编辑器塌缩成
            5px，展开「源代码」只看到一片空白（代码并非未返回，是没被渲染出来）。
            `overflow-hidden` 兜住 Monaco 首次布局的瞬时溢出（容器 `clientHeight`
            为 0 时它会回退到 body 高度）。与 `QuickSubmitDialog` 同一写法。
          -->
          <div
            v-if="codeExpanded"
            class="flex h-[520px] flex-col overflow-hidden border-t border-[var(--border-color)]"
          >
            <p
              v-if="!detail.code"
              class="px-4 py-6 text-sm text-[var(--text-secondary)]"
            >
              未取到本次提交的源代码
            </p>
            <CodeEditor
              v-else
              :model-value="detail.code"
              :language="codeLanguage"
              :is-dirty="false"
              readonly
            />
          </div>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.result-summary > div:first-child { justify-content: space-between; padding-bottom: 20px; }
.result-summary > div:first-child > span { padding: 0; border: 0; border-radius: 0; font-size: 30px; background: transparent; }
.result-summary > div:first-child > div { font-size: 14px; gap: 24px; }
.submission-detail section { border-radius: 0; }
</style>
