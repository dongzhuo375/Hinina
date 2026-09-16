<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import ContestStatsBar from '@/components/contest/ContestStatsBar.vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import ProblemCard from '@/components/problem/ProblemCard.vue'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { useRankStore } from '@/stores/rankStore'
import type { ContestProblem } from '@/types/contest'
import { getContestPhase } from '@/utils/contest'
import { createPoller } from '@/utils/polling'
import type { Poller } from '@/utils/polling'

/// 题目总览刷新节奏：30s ± 5s 抖动，打散全场客户端的请求相位（见 utils/polling 头注释）
const POLL_INTERVAL_MS = 30_000
const POLL_JITTER_MS = 5_000

const router = useRouter()
const contestStore = useContestStore()
const problemStore = useProblemStore()
const rankStore = useRankStore()
const authStore = useAuthStore()

/// 轮询器句柄是副作用句柄而非渲染状态：放普通变量，不进 ref/reactive
let poller: Poller | null = null
/// 补充数据（limits/状态/榜单）是否已触发过 —— 开赛后题目首次出现时需补拉一次
let supplementaryLoaded = false

const contest = computed(() => contestStore.contest)
const problems = computed(() => contestStore.problems)

const phase = computed(() => getContestPhase(contest.value, Math.floor(Date.now() / 1000)))

/// OI 赛制（contestType===1）罚时口径不同，元信息行文案需区分
const formatLabel = computed(() => (contest.value?.contestType === 1 ? 'OI 赛制' : 'ACM 赛制'))

const startTimeText = computed(() =>
  contest.value
    ? new Date(contest.value.startTime * 1000).toLocaleString('zh-CN', { hour12: false })
    : '',
)

/// 全屏兜底仅在没有任何数据时出现：已渲染出卡片后，轮询的瞬时失败不应把整页打回错误态
const showLoading = computed(() => contestStore.isLoading && problems.value.length === 0)
const showError = computed(
  () => contestStore.error !== null && problems.value.length === 0 && !showLoading.value,
)
/// 未开始且拿不到题目才提示；若 OJ 提前放题则照常展示卡片
const showUpcoming = computed(() => phase.value === 'upcoming' && problems.value.length === 0)

onMounted(async () => {
  await ensureContest()
  loadSupplementary()
  startPolling()
})

onUnmounted(() => {
  stopPolling()
})

/// 外壳（ContestLayout）通常已发起 loadContest：`whenLoaded` 复用在途请求、
/// 无人拉取时兜底发起（P59 统一入口）；失败原因已由 store 写入 error，
/// 模板展示 ErrorMessage 并提供重试
async function ensureContest() {
  if (contest.value) return
  try {
    await contestStore.whenLoaded()
  } catch {
    // 失败原因已写入 contestStore.error
  }
}

/// 补充数据并发拉取且不 await：卡片先用 ac/total 渲染首屏，
/// limits / 我的状态 / 榜单到达后响应式补齐；三者各自吞错，缺失即缺失
function loadSupplementary() {
  const c = contest.value
  const list = problems.value
  if (!c || list.length === 0) return
  supplementaryLoaded = true
  problemStore.loadLimits(c.id, list.map((p) => p.displayId))
  problemStore.loadMyStatus(c.id, list.map((p) => p.problemId))
  if (!rankStore.hasData) {
    // 统计卡与「AC 用时」pill 依赖榜单我的行；失败只影响这两处，不打断页面
    rankStore.loadRank(c.id, authStore.user?.id ?? null, 1).catch(() => {})
  }
}

function startPolling() {
  stopPolling()
  poller = createPoller({
    task: refresh,
    intervalMs: POLL_INTERVAL_MS,
    jitterMs: POLL_JITTER_MS,
    // 切后台/最小化时暂停，避免无谓请求
    isPaused: () => typeof document !== 'undefined' && document.hidden,
    onError: () => {
      // 瞬时失败静默：保留上一次数据，等下一周期（错误已记录在 store.error）
    },
  })
  poller.start()
}

function stopPolling() {
  poller?.stop()
  poller = null
}

/// 每周期：重拉比赛（刷新 ac/total）+ 我的提交状态（刷新卡片 pill）
async function refresh() {
  try {
    await contestStore.loadContest()
  } catch {
    return // 比赛拉取失败：本周期跳过，等下一周期
  }
  if (supplementaryLoaded) {
    const c = contest.value
    if (c && problems.value.length > 0) {
      await problemStore.loadMyStatus(c.id, problems.value.map((p) => p.problemId))
    }
  } else {
    // 开赛后题目首次出现（挂载时还是空列表）：补拉 limits / 状态 / 榜单
    loadSupplementary()
  }
}

async function retry() {
  try {
    await contestStore.loadContest()
  } catch {
    // 仍失败则停留在错误页
  }
  loadSupplementary()
}

function openProblem(problem: ContestProblem) {
  router.push({ name: 'ProblemSolve', params: { displayId: problem.displayId } })
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-body)]">
    <LoadingSpinner v-if="showLoading" class="flex-1" message="正在加载比赛数据…" />
    <ErrorMessage
      v-else-if="showError"
      class="flex-1"
      :message="contestStore.error ?? '加载比赛失败'"
      :retry="retry"
    />
    <template v-else>
      <!-- 子头横幅：标题 + 元信息 + 实时统计卡 -->
      <section
        v-if="contest"
        class="shrink-0 border-b border-[var(--border-color)] bg-[var(--bg-card)] px-6 py-4 shadow-xs"
      >
        <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
          <div>
            <h2 class="text-2xl font-bold tracking-tight text-[var(--text-primary)]">
              {{ contest.title }} 题目总览
              <span class="text-lg font-normal text-[var(--text-muted)]">/ Problemset</span>
            </h2>
            <p class="mt-1 flex items-center gap-2 text-xs text-[var(--text-secondary)]">
              <span>共 {{ problems.length }} 道题目</span>
              <span>·</span>
              <span>{{ formatLabel }}</span>
              <span>·</span>
              <span>点击题目卡片查看详情与提交</span>
            </p>
          </div>
          <ContestStatsBar />
        </div>
      </section>

      <div class="flex-1 overflow-y-auto p-6">
        <!-- 比赛未开始：HOJ 此时通常不放题，给明确提示而非错误页 -->
        <div v-if="showUpcoming" class="flex flex-col items-center justify-center gap-3 py-24 text-center">
          <div
            class="flex h-14 w-14 items-center justify-center rounded-2xl border border-amber-200 bg-amber-50 text-[var(--color-warning)]"
          >
            <svg class="h-7 w-7" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
              <path
                d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
                stroke-linecap="round"
                stroke-linejoin="round"
              ></path>
            </svg>
          </div>
          <h2 class="text-lg font-bold tracking-tight text-[var(--text-primary)]">比赛尚未开始</h2>
          <p class="max-w-sm text-sm leading-relaxed text-[var(--text-secondary)]">
            计划开始时间：{{ startTimeText }}。题目列表将在开赛后自动出现，页面每 30 秒左右自动刷新。
          </p>
        </div>

        <!-- 空态：比赛已开始但题目列表为空 -->
        <div
          v-else-if="problems.length === 0"
          class="flex flex-col items-center justify-center gap-3 py-24 text-center"
        >
          <div
            class="flex h-14 w-14 items-center justify-center rounded-2xl border border-[var(--border-color)] bg-[var(--bg-card)] text-[var(--text-muted)]"
          >
            <svg class="h-7 w-7" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
              <path
                d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
                stroke-linecap="round"
                stroke-linejoin="round"
              ></path>
            </svg>
          </div>
          <h2 class="text-lg font-bold tracking-tight text-[var(--text-primary)]">暂无题目</h2>
          <p class="max-w-sm text-sm leading-relaxed text-[var(--text-secondary)]">
            本场比赛尚未发布题目，或题目列表获取失败。
          </p>
          <button
            class="rounded-md bg-[var(--color-primary)] px-4 py-1.5 text-sm text-white transition-colors hover:bg-[var(--color-primary-hover)]"
            @click="retry"
          >
            刷新
          </button>
        </div>

        <!-- 题目卡片网格：桌面端一行三卡（lg 起），窄窗口逐级降为两列/单列；三列时收窄间距保持卡片呼吸感 -->
        <div v-else class="mx-auto grid max-w-7xl grid-cols-1 gap-5 md:grid-cols-2 lg:grid-cols-3 xl:gap-6">
          <ProblemCard
            v-for="problem in problems"
            :key="problem.displayId"
            :problem="problem"
            @open="openProblem(problem)"
          />
        </div>
      </div>
    </template>
  </div>
</template>
