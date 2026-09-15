<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import { useContestStore } from '@/stores/contestStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import type { ContestProblem } from '@/types/contest'
import type { SubmissionRecord } from '@/types/submission'
import {
  STATUS_OPTIONS,
  formatClock,
  formatCodeLength,
  formatDurationHms,
  formatMemoryKb,
  isJudging,
  statusLabel,
  statusTone,
} from '@/utils/submission'
import type { StatusTone } from '@/utils/submission'
import { createPoller } from '@/utils/polling'
import type { Poller } from '@/utils/polling'

/// 评测页轮询节奏：5s ± 1s，仅在当前页存在评测中的提交时运行（收敛即停）
const POLL_INTERVAL_MS = 5_000
const POLL_JITTER_MS = 1_000

const route = useRoute()
const router = useRouter()
const contestStore = useContestStore()
const submissionStore = useSubmissionStore()

/// 组件已卸载标记：异步引导链路可能在卸载后才落地，此时不得再触碰 store
let alive = true
/// 轮询器是副作用句柄而非渲染状态：放普通变量，不进 ref/reactive
let poller: Poller | null = null

const contest = computed(() => contestStore.contest)
const history = computed(() => submissionStore.history)

/// 状态筛选下拉的字符串桥接值（'' = 全部；其余为 HOJ 状态码字符串）
const statusSelectValue = computed(() =>
  history.value.statusFilter === null ? '' : String(history.value.statusFilter),
)

/// 当前页记录的客户端搜索过滤：运行编号 / 题目标题 / 展示题号 包含匹配
const searchQuery = ref('')
const visibleRecords = computed<SubmissionRecord[]>(() => {
  const q = searchQuery.value.trim().toLowerCase()
  if (!q) return history.value.records
  return history.value.records.filter((r) => {
    return (
      r.submitId.toLowerCase().includes(q) ||
      r.title.toLowerCase().includes(q) ||
      r.displayId.toLowerCase().includes(q) ||
      r.displayPid.toLowerCase().includes(q)
    )
  })
})

/// displayId → 比赛题目（气球色徽章 / 解题页跳转用）
const problemByDisplayId = computed(() => {
  const map = new Map<string, ContestProblem>()
  for (const p of contestStore.problems) map.set(p.displayId, p)
  return map
})

function badgeColorOf(record: SubmissionRecord): string {
  return problemByDisplayId.value.get(record.displayId)?.color || ''
}

/// 赛时相对时间：submitTime − contest.startTime（同为 epoch 秒）；赛前提交钳制为 --:--:--
function contestElapsed(record: SubmissionRecord): string {
  const start = contest.value?.startTime
  if (start == null) return '--:--:--'
  return formatDurationHms(record.submitTime - start)
}

/// tone → pill 样式类（与 utils/submission 的语义色调一一对应）
const TONE_CLASSES: Record<StatusTone, string> = {
  ac: 'border-emerald-200 bg-emerald-50 text-emerald-600',
  wa: 'border-rose-200 bg-rose-50 text-rose-600',
  tle: 'border-amber-200 bg-amber-50 text-amber-600',
  pending: 'border-cyan-200 bg-cyan-50 text-cyan-600',
  system: 'border-slate-200 bg-slate-100 text-slate-600',
  neutral: 'border-slate-200 bg-slate-100 text-slate-500',
}

function toneClasses(tone: StatusTone): string {
  return TONE_CLASSES[tone] ?? TONE_CLASSES.neutral
}

/// 首次加载（还没有任何可展示数据）才铺满 spinner；刷新时保留旧表格不打断阅读
const isBootstrapping = computed(
  () => history.value.isLoading && history.value.records.length === 0,
)
const isFatalError = computed(
  () =>
    (history.value.error !== null && history.value.records.length === 0) ||
    (contestStore.error !== null && !contest.value),
)
const fatalMessage = computed(
  () => history.value.error ?? contestStore.error ?? '加载提交记录失败',
)

/**
 * 页码窗口：首尾页 + 当前页 ±1，其余折叠成省略号（与榜单页同一算法）。
 */
const pageItems = computed<(number | '…')[]>(() => {
  const pages = history.value.pages
  const current = history.value.current
  if (pages <= 1) return []
  if (pages <= 7) return Array.from({ length: pages }, (_, i) => i + 1)
  const nums = new Set<number>([1, pages, current])
  for (const delta of [-1, 1]) {
    const candidate = current + delta
    if (candidate > 1 && candidate < pages) nums.add(candidate)
  }
  if (current <= 3) for (const n of [2, 3, 4]) nums.add(n)
  if (current >= pages - 2) for (const n of [pages - 3, pages - 2, pages - 1]) nums.add(n)
  const sorted = [...nums].filter((n) => n >= 1 && n <= pages).sort((a, b) => a - b)
  const items: (number | '…')[] = []
  let previous = 0
  for (const n of sorted) {
    if (previous !== 0 && n - previous > 1) items.push('…')
    items.push(n)
    previous = n
  }
  return items
})

async function bootstrap(): Promise<void> {
  try {
    await contestStore.whenLoaded()
  } catch {
    // 失败原因已写入 contestStore.error
  }
  if (!alive) return
  const contestId = contest.value?.id
  if (!contestId) return
  const problemQuery = route.query.problem
  try {
    // 深链携带 ?problem=A 时以题目筛选作为首查（setHistoryProblemFilter 内部回到第 1 页）
    if (typeof problemQuery === 'string' && problemQuery) {
      await submissionStore.setHistoryProblemFilter(contestId, problemQuery)
    } else {
      await submissionStore.fetchHistory(contestId, 1)
    }
  } catch {
    // 失败原因已写入 history.error
  }
}

async function retry(): Promise<void> {
  if (!contest.value) {
    await bootstrap()
    return
  }
  try {
    await submissionStore.fetchHistory(contest.value.id, history.value.current)
  } catch {
    // 同上
  }
}

function onProblemFilterChange(e: Event) {
  const contestId = contest.value?.id
  if (!contestId) return
  const value = (e.target as HTMLSelectElement).value
  submissionStore.setHistoryProblemFilter(contestId, value || null).catch(() => {
    // 失败原因已写入 history.error
  })
}

function onStatusFilterChange(e: Event) {
  const contestId = contest.value?.id
  if (!contestId) return
  const raw = (e.target as HTMLSelectElement).value
  const status = raw === '' ? null : Number.parseInt(raw, 10)
  submissionStore.setHistoryStatusFilter(contestId, status).catch(() => {
    // 同上
  })
}

function refresh() {
  const contestId = history.value.contestId || contest.value?.id
  if (!contestId) return
  submissionStore.fetchHistory(contestId, history.value.current).catch(() => {
    // 同上：旧数据保留，错误条兜底
  })
}

function goToPage(page: number) {
  const contestId = history.value.contestId || contest.value?.id
  if (!contestId) return
  submissionStore.setHistoryPage(contestId, page).catch(() => {
    // 同上
  })
}

function clearProblemFilter() {
  const contestId = contest.value?.id
  if (!contestId) return
  submissionStore.setHistoryProblemFilter(contestId, null).catch(() => {})
}

function openDetail(record: SubmissionRecord) {
  router.push({ name: 'SubmissionDetail', params: { submitId: record.submitId } })
}

function openProblem(record: SubmissionRecord) {
  if (!problemByDisplayId.value.has(record.displayId)) return
  router.push({ name: 'ProblemSolve', params: { displayId: record.displayId } })
}

// ── 评测中提交的实时刷新（视图自有轮询，收敛即停） ──

function stopPoller() {
  poller?.stop()
  poller = null
}

function syncPoller() {
  const hasJudging = history.value.records.some((r) => isJudging(r.status))
  if (hasJudging && !poller) {
    poller = createPoller({
      task: async () => {
        const contestId = history.value.contestId || contest.value?.id
        if (!contestId || !alive) return
        try {
          await submissionStore.fetchHistory(contestId, history.value.current)
        } catch {
          // 静默：瞬时失败等下一周期，错误已写入 history.error（旧数据保留）
        }
      },
      intervalMs: POLL_INTERVAL_MS,
      jitterMs: POLL_JITTER_MS,
      // 切后台暂停：评测页轮询属于「在场才需要」的列表刷新，与提交收敛轮询取舍相反
      isPaused: () => typeof document !== 'undefined' && document.hidden,
      onError: () => {},
    })
    poller.start()
  } else if (!hasJudging) {
    stopPoller()
  }
}

/// 记录集每次刷新（引导/筛选/翻页/轮询）后重新决策轮询启停
watch(() => history.value.records, syncPoller)

onMounted(() => {
  void bootstrap()
})

onUnmounted(() => {
  alive = false
  stopPoller()
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-body)]">
    <!-- 头部：标题 + 筛选工具条 -->
    <div class="shrink-0 border-b border-[var(--border-color)] bg-[var(--bg-card)] px-5 pb-3 pt-4">
      <div class="mb-3 flex items-center justify-between gap-4">
        <div class="flex min-w-0 items-center space-x-3">
          <h1 class="truncate text-lg font-bold tracking-tight text-[var(--text-primary)]">
            {{ contest?.title ?? '比赛' }} 评测状态
          </h1>
          <span class="shrink-0 font-mono text-xs text-[var(--text-muted)]">/ Submissions</span>
        </div>
      </div>

      <!-- 筛选工具条：题目 / 状态 / 搜索 / 刷新 -->
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div class="flex flex-wrap items-center gap-2">
          <div class="relative">
            <select
              class="cursor-pointer appearance-none rounded-lg border border-[var(--border-color)] bg-white py-1.5 pl-3 pr-8 text-xs font-medium text-[var(--text-primary)] shadow-xs transition-colors hover:bg-slate-50 focus:border-[var(--color-primary)] focus:outline-none"
              :value="history.problemFilter ?? ''"
              @change="onProblemFilterChange"
            >
              <option value="">全部题目</option>
              <option v-for="p in contestStore.problems" :key="p.displayId" :value="p.displayId">
                {{ p.displayId }} {{ p.displayTitle }}
              </option>
            </select>
            <svg
              class="pointer-events-none absolute right-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--text-muted)]"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <path d="M6 9l6 6 6-6" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </div>

          <div class="relative">
            <select
              class="cursor-pointer appearance-none rounded-lg border border-[var(--border-color)] bg-white py-1.5 pl-3 pr-8 text-xs font-medium text-[var(--text-primary)] shadow-xs transition-colors hover:bg-slate-50 focus:border-[var(--color-primary)] focus:outline-none"
              :value="statusSelectValue"
              @change="onStatusFilterChange"
            >
              <option value="">全部状态</option>
              <option v-for="opt in STATUS_OPTIONS" :key="opt.value" :value="String(opt.value)">
                {{ opt.label }}
              </option>
            </select>
            <svg
              class="pointer-events-none absolute right-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--text-muted)]"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <path d="M6 9l6 6 6-6" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </div>

          <div class="relative flex items-center">
            <svg
              class="pointer-events-none absolute left-2.5 h-3.5 w-3.5 text-[var(--text-muted)]"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <path
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
            <input
              v-model="searchQuery"
              type="text"
              placeholder="搜索提交编号 / 题目…"
              title="在当前页内过滤（运行编号 / 题目）"
              class="w-52 rounded-lg border border-[var(--border-color)] bg-white py-1.5 pl-8 pr-3 text-xs text-[var(--text-primary)] shadow-xs transition-colors placeholder:text-[var(--text-muted)] hover:bg-slate-50 focus:border-[var(--color-primary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-primary)]"
            />
          </div>
        </div>

        <button
          type="button"
          title="刷新列表"
          class="flex items-center gap-1.5 rounded-lg border border-[var(--border-color)] bg-slate-50 px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] shadow-xs transition-colors hover:bg-slate-100 active:bg-slate-200"
          @click="refresh"
        >
          <svg
            class="h-4 w-4 text-[var(--text-secondary)]"
            :class="{ 'animate-spin': history.isLoading }"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
            aria-hidden="true"
          >
            <path d="M4 4v5h5M20 20v-5h-5" stroke-linecap="round" stroke-linejoin="round" />
            <path
              d="M20.49 9A9 9 0 005.64 5.64L4 7.5m16 4.5l-1.64 1.86A9 9 0 013.51 15"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          <span>刷新</span>
        </button>
      </div>
    </div>

    <!-- 表格主体 -->
    <div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 py-3">
      <!-- 轮询/刷新失败但仍有旧数据：顶部错误条，不清空表格 -->
      <div
        v-if="history.error && history.records.length > 0"
        class="flex shrink-0 items-center gap-2 rounded-lg border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] px-3 py-2 text-xs text-rose-700"
      >
        <svg
          class="h-3.5 w-3.5 shrink-0"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          <path
            d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <span class="min-w-0 flex-1 truncate">刷新失败：{{ history.error }}（已保留上一次数据）</span>
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-wa-border)] bg-white px-2 py-0.5 font-medium transition-colors hover:bg-rose-50"
          @click="retry"
        >
          立即重试
        </button>
      </div>

      <div
        class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-[var(--border-color)] bg-white shadow-sm"
      >
        <LoadingSpinner v-if="isBootstrapping" message="正在加载提交记录…" class="m-auto" />

        <ErrorMessage
          v-else-if="isFatalError"
          :message="fatalMessage"
          :retry="retry"
          class="m-auto"
        />

        <template v-else>
          <!-- 空态 -->
          <div
            v-if="!history.isLoading && history.records.length === 0"
            class="flex flex-1 flex-col items-center justify-center gap-3 p-12 text-center"
          >
            <div
              class="flex h-14 w-14 items-center justify-center rounded-2xl border border-[var(--border-color)] bg-[var(--bg-body)] text-[var(--text-muted)]"
            >
              <svg
                class="h-7 w-7"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                viewBox="0 0 24 24"
                aria-hidden="true"
              >
                <path
                  d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
            </div>
            <p class="text-sm font-medium text-[var(--text-primary)]">暂无提交记录</p>
            <p v-if="history.problemFilter" class="text-xs text-[var(--text-muted)]">
              当前按题目 {{ history.problemFilter }} 过滤，尚无匹配提交
            </p>
            <p v-else class="text-xs text-[var(--text-muted)]">
              在解题页提交代码后，评测结果会出现在这里
            </p>
            <button
              v-if="history.problemFilter"
              type="button"
              class="rounded-md border border-[var(--border-color)] bg-white px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-slate-50"
              @click="clearProblemFilter"
            >
              清除题目筛选
            </button>
          </div>

          <!-- 搜索无匹配（当前页有数据但被过滤光） -->
          <div
            v-else-if="visibleRecords.length === 0"
            class="flex flex-1 flex-col items-center justify-center gap-2 p-12 text-center"
          >
            <p class="text-sm font-medium text-[var(--text-primary)]">当前页没有匹配的提交</p>
            <p class="text-xs text-[var(--text-muted)]">搜索仅作用于本页记录，试试更换关键词或翻页</p>
          </div>

          <table v-else class="w-full border-collapse text-left">
            <thead>
              <tr
                class="border-b border-[var(--border-color)] bg-slate-50/80 font-mono text-[11px] tracking-wider text-[var(--text-secondary)] uppercase"
              >
                <th class="px-4 py-3 font-semibold">运行编号</th>
                <th class="px-4 py-3 font-semibold">提交时间</th>
                <th class="px-4 py-3 font-semibold">题目</th>
                <th class="px-4 py-3 font-semibold">语言</th>
                <th class="px-4 py-3 font-semibold">评测结果</th>
                <th class="px-4 py-3 text-right font-semibold">耗时</th>
                <th class="px-4 py-3 text-right font-semibold">内存</th>
                <th class="px-4 py-3 text-right font-semibold">代码长度</th>
                <th class="px-4 py-3 text-center font-semibold">操作</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-[var(--border-color)] text-xs text-[var(--text-primary)]">
              <tr
                v-for="record in visibleRecords"
                :key="record.submitId"
                class="transition-colors hover:bg-slate-50/70"
              >
                <td class="px-4 py-3 font-mono text-xs font-semibold tabular-nums">
                  #{{ record.submitId }}
                </td>
                <td
                  class="px-4 py-3 font-mono text-[11px] tabular-nums text-[var(--text-secondary)]"
                >
                  <div>{{ formatClock(record.submitTime) }}</div>
                  <div class="text-[10px] text-[var(--text-muted)]">
                    赛时 {{ contestElapsed(record) }}
                  </div>
                </td>
                <td class="px-4 py-3">
                  <div
                    class="flex items-center gap-2"
                    :class="problemByDisplayId.has(record.displayId) ? 'cursor-pointer' : ''"
                    @click="openProblem(record)"
                  >
                    <!-- 字母徽章：底色用 HOJ 气球色，未配置/未匹配时回退石板灰 -->
                    <span
                      class="flex h-5 w-5 shrink-0 items-center justify-center rounded font-mono text-[11px] font-bold"
                      :class="badgeColorOf(record) ? 'text-white' : 'bg-slate-100 text-slate-600'"
                      :style="
                        badgeColorOf(record) ? { backgroundColor: badgeColorOf(record) } : undefined
                      "
                    >
                      {{ record.displayId || '·' }}
                    </span>
                    <span class="font-medium hover:text-[var(--color-primary)]">
                      {{ record.title }}
                    </span>
                  </div>
                </td>
                <td class="px-4 py-3 text-[11px] text-[var(--text-secondary)]">
                  {{ record.language }}
                </td>
                <td class="px-4 py-3">
                  <span
                    class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-semibold shadow-xs"
                    :class="toneClasses(statusTone(record.status))"
                  >
                    <svg
                      v-if="isJudging(record.status)"
                      class="h-3 w-3 animate-spin"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2.5"
                      viewBox="0 0 24 24"
                      aria-hidden="true"
                    >
                      <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
                      <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
                    </svg>
                    <span>{{ statusLabel(record.status) }}</span>
                  </span>
                </td>
                <td
                  class="px-4 py-3 text-right font-mono text-[11px] tabular-nums text-[var(--text-secondary)]"
                >
                  {{ isJudging(record.status) ? '-' : `${record.timeMs} ms` }}
                </td>
                <td
                  class="px-4 py-3 text-right font-mono text-[11px] tabular-nums text-[var(--text-secondary)]"
                >
                  {{ isJudging(record.status) ? '-' : formatMemoryKb(record.memoryKb) }}
                </td>
                <td
                  class="px-4 py-3 text-right font-mono text-[11px] tabular-nums text-[var(--text-secondary)]"
                >
                  {{ formatCodeLength(record.length) }}
                </td>
                <td class="px-4 py-3 text-center">
                  <button
                    type="button"
                    class="inline-flex items-center gap-1 text-xs font-medium text-[var(--color-primary)] transition-colors hover:text-[var(--color-primary-hover)]"
                    @click="openDetail(record)"
                  >
                    <svg
                      class="h-3.5 w-3.5"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                      viewBox="0 0 24 24"
                      aria-hidden="true"
                    >
                      <path
                        d="M2.036 12.322a1.012 1.012 0 010-.639C3.423 7.51 7.36 4.5 12 4.5c4.638 0 8.573 3.007 9.963 7.178.07.207.07.431 0 .639C20.577 16.49 16.64 19.5 12 19.5c-4.638 0-8.573-3.007-9.963-7.178z"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                      />
                      <path d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                    <span>查看详情</span>
                  </button>
                </td>
              </tr>
            </tbody>
          </table>

          <!-- 底栏：总数 + 分页 -->
          <div
            v-if="history.records.length > 0"
            class="mt-auto flex shrink-0 flex-wrap items-center justify-between gap-3 border-t border-[var(--border-color)] px-4 py-2.5"
          >
            <div class="text-xs font-medium text-[var(--text-secondary)]">
              共
              <span class="font-mono font-semibold tabular-nums text-[var(--text-primary)]">
                {{ history.total }}
              </span>
              条提交记录
              <span v-if="searchQuery.trim()" class="text-[var(--text-muted)]">
                · 本页匹配 {{ visibleRecords.length }} 条
              </span>
            </div>
            <div v-if="history.pages > 1" class="flex items-center gap-1 text-xs">
              <button
                type="button"
                class="rounded-md border border-[var(--border-color)] px-2 py-1 transition-colors enabled:hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-40"
                :disabled="history.current <= 1 || history.isLoading"
                @click="goToPage(history.current - 1)"
              >
                上一页
              </button>
              <template
                v-for="(item, index) in pageItems"
                :key="item === '…' ? `gap-${index}` : item"
              >
                <span v-if="item === '…'" class="px-1 text-[var(--text-muted)]">…</span>
                <button
                  v-else
                  type="button"
                  class="min-w-7 rounded-md border px-2 py-1 font-mono transition-colors disabled:opacity-60"
                  :class="
                    item === history.current
                      ? 'border-[var(--color-primary)] bg-[var(--color-primary)] font-semibold text-white'
                      : 'border-[var(--border-color)] enabled:hover:bg-slate-50'
                  "
                  :disabled="history.isLoading"
                  @click="goToPage(item)"
                >
                  {{ item }}
                </button>
              </template>
              <button
                type="button"
                class="rounded-md border border-[var(--border-color)] px-2 py-1 transition-colors enabled:hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-40"
                :disabled="history.current >= history.pages || history.isLoading"
                @click="goToPage(history.current + 1)"
              >
                下一页
              </button>
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>
