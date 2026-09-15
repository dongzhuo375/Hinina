<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import ContestStatsBar from '@/components/contest/ContestStatsBar.vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import RankToolbar from '@/components/rank/RankToolbar.vue'
import ScoreboardTable from '@/components/rank/ScoreboardTable.vue'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { useRankStore } from '@/stores/rankStore'
import { getContestPhase } from '@/utils/contest'

const contestStore = useContestStore()
const rankStore = useRankStore()
const authStore = useAuthStore()

/// 每秒推进一次的时钟：同时驱动「更新于 x 秒前」、比赛阶段跃迁与封榜判定
const nowMs = ref(Date.now())
const nowSecs = computed(() => Math.floor(nowMs.value / 1000))
let clock: ReturnType<typeof setInterval> | null = null
/// 组件已卸载标记：异步引导链路可能在卸载后才落地，此时不得再触碰 store
let alive = true

const contest = computed(() => contestStore.contest)
const phase = computed(() => getContestPhase(contest.value, nowSecs.value))
const isEnded = computed(() => phase.value === 'ended')
const isUpcoming = computed(() => phase.value === 'upcoming')

/**
 * 封榜判定只看 `contest` 字段，**不能**依赖 `forceRefresh` 的返回结果（文档 §9.3）：
 * 非比赛创建者/超管传 `forceRefresh: true` 会被服务端静默忽略，客户端拿到的仍是封榜榜单，
 * 用它反推「是否封榜」永远得不到真值。
 *
 * 文档 §4 把封榜窗口写作 `[sealRankTime, endTime]`，但比赛结束后服务端并不会自动解封
 * （需管理员手动解除），因此这里只判下界，避免结束后提示消失、选手误以为榜单已解封。
 */
const isSealed = computed(() => {
  const current = contest.value
  if (!current?.sealRank || current.sealRankTime === null) return false
  return nowSecs.value >= current.sealRankTime
})

/// 首次加载（还没有任何可展示的数据）才铺满屏 spinner；轮询刷新时保留旧表格不打断阅读
const isBootstrapping = computed(
  () =>
    (!contest.value && contestStore.isLoading) || (rankStore.isLoading && !rankStore.hasData),
)

/// 致命错误：一条数据都没有，只能整块换成 ErrorMessage（轮询失败不属于此列，旧数据要留着）
const isFatalError = computed(() => rankStore.error !== null && !rankStore.hasData)

/// 表格是否真正渲染 —— 底栏（刷新状态 + 分页）只在有表格可解释时才出现
const showTable = computed(
  () => !isBootstrapping.value && !isUpcoming.value && !isFatalError.value,
)

const updatedText = computed(() => {
  if (!rankStore.lastUpdated) return ''
  const secs = Math.max(0, Math.floor((nowMs.value - rankStore.lastUpdated) / 1000))
  if (secs < 60) return `更新于 ${secs} 秒前`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `更新于 ${mins} 分钟前`
  return `更新于 ${Math.floor(mins / 60)} 小时前`
})

/// 刷新状态文案：比赛已结束时 HOJ 网页端会禁用刷新，这里同样停掉轮询并说明原因；
/// 全量快照模式（打星/女生筛选）下自动轮询暂停，只保留手动刷新
const liveText = computed(() => {
  if (isEnded.value) return '比赛已结束，停止刷新'
  if (rankStore.isFullMode) return '全量快照 · 手动刷新'
  return rankStore.isLive ? '实时刷新中' : '实时刷新未开启'
})

/// 是否处于打星/女生的客户端筛选（含快照尚未就位的拉取中/失败态）
const isFullFilterActive = computed(
  () => rankStore.groupFilter === 'star' || rankStore.groupFilter === 'female',
)

/// 底栏「共 N 队」：全量模式显示筛选后的行数，常规模式显示修正后的真实参与人数
const totalLabel = computed(() =>
  rankStore.isFullMode
    ? `共 ${rankStore.fullFilteredRows.length} 队（筛选后）`
    : `共 ${rankStore.participants} 队`,
)

/**
 * 页码窗口：首尾页 + 当前页 ±1，其余折叠成省略号。
 * 榜单按 50 条分页，几百人的比赛会有十几页，全量页码会挤爆底栏。
 */
const pageItems = computed<(number | '…')[]>(() => {
  const pages = rankStore.pages
  const current = rankStore.current
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

/**
 * 确保比赛数据就绪。
 *
 * 外壳 `ContestLayout` 也会拉取比赛，但子视图的 `onMounted` 先于父视图触发，
 * 直接进入榜单页时这里才是实际发起方；`whenLoaded` 复用在途请求（P59 统一入口），
 * 失败原因已写入 contestStore.error，模板据此展示重试入口。
 */
async function ensureContest(): Promise<void> {
  if (contest.value) return
  try {
    await contestStore.whenLoaded()
  } catch {
    // 失败原因已写入 contestStore.error
  }
}

async function bootstrap(): Promise<void> {
  await ensureContest()
  if (!alive) return
  const contestId = contest.value?.id
  if (!contestId) return
  try {
    await rankStore.loadRank(contestId, authStore.user?.id ?? null, 1)
  } catch {
    // 失败原因已写入 rankStore.error
  }
  if (!alive) return
  // 比赛已结束时不启动轮询（HOJ 网页端此时禁用刷新，继续打请求只是浪费服务端算力）
  if (!isEnded.value) rankStore.startLive(() => isEnded.value)
}

async function retry(): Promise<void> {
  // 全量快照模式下错误来自快照拉取，重试即重拉快照
  if (isFullFilterActive.value) {
    await rankStore.refresh()
    return
  }
  const contestId = rankStore.contestId || contest.value?.id
  if (!contestId) {
    await bootstrap()
    return
  }
  try {
    await rankStore.loadRank(contestId, authStore.user?.id ?? null, rankStore.current)
  } catch {
    // 同上
  }
}

/// 手动刷新：常规模式重拉当前页，全量模式重拉整个快照（refresh 内部吞异常）
function manualRefresh(): void {
  void rankStore.refresh()
}

function goToPage(page: number) {
  rankStore.setPage(page).catch(() => {
    // 失败原因已写入 rankStore.error，旧数据保留
  })
}

onMounted(() => {
  clock = setInterval(() => {
    nowMs.value = Date.now()
  }, 1000)
  void bootstrap()
})

onUnmounted(() => {
  alive = false
  if (clock) clearInterval(clock)
  clock = null
  // 轮询器是模块级副作用句柄，离开榜单页必须回收，否则会在后台持续打 OJ
  rankStore.stopLive()
})

/// 页面停留期间比赛结束（本地时钟越过 endTime）：立即停掉轮询
watch(isEnded, (ended) => {
  if (ended) rankStore.stopLive()
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-body)]">
    <!-- 头部：标题 + 统计卡 + 工具条 -->
    <div
      class="shrink-0 border-b border-[var(--border-color)] bg-[var(--bg-card)] px-5 pb-2.5 pt-3.5"
    >
      <div class="mb-2.5 flex items-center justify-between gap-4">
        <div class="flex min-w-0 items-center space-x-3">
          <h1 class="truncate text-lg font-bold tracking-tight text-[var(--text-primary)]">
            {{ contest?.title ?? '比赛' }} 实时榜单
          </h1>
          <span class="shrink-0 font-mono text-xs text-[var(--text-muted)]">/ Live Scoreboard</span>
        </div>
        <ContestStatsBar />
      </div>
      <RankToolbar />
    </div>

    <div class="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden px-4 py-3">
      <!-- 封榜提示：只信 contest 字段（文档 §9.3），封榜期间通过状态对普通用户不可见 -->
      <div
        v-if="isSealed"
        class="flex shrink-0 items-center gap-2 rounded-lg border border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] px-3 py-2 text-xs text-amber-800"
      >
        <svg
          class="h-3.5 w-3.5 shrink-0"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          <rect x="4" y="10" width="16" height="10" rx="2" stroke-linejoin="round" />
          <path d="M8 10V7a4 4 0 0 1 8 0v3" stroke-linecap="round" />
        </svg>
        <span class="font-medium">榜单已封榜，仅显示尝试次数</span>
        <span class="text-amber-700/80">封榜时段内的提交结果将在赛后统一揭晓</span>
      </div>

      <!-- 轮询失败但仍有旧数据：顶部一条不打断阅读的错误条，绝不清空榜单 -->
      <div
        v-if="rankStore.error && rankStore.hasData"
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
        <span class="min-w-0 flex-1 truncate">
          榜单刷新失败：{{ rankStore.error }}（已保留上一次数据，将自动重试）
        </span>
        <button
          type="button"
          class="shrink-0 rounded-md border border-[var(--color-wa-border)] bg-white px-2 py-0.5 font-medium transition-colors hover:bg-rose-50"
          @click="retry"
        >
          立即重试
        </button>
      </div>

      <!-- 全量快照模式状态行：打星/女生为客户端跨页筛选，展示快照规模与手动刷新入口 -->
      <div
        v-if="isFullFilterActive"
        class="flex shrink-0 flex-wrap items-center gap-2 rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-xs text-slate-500"
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
            d="M3 4a1 1 0 0 1 1-1h16a1 1 0 0 1 1 1v2a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V4zm3 7a1 1 0 1 0 0 2h12a1 1 0 1 0 0-2H6zm3 7a1 1 0 1 0 0 2h6a1 1 0 1 0 0-2H9z"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <span v-if="rankStore.isFullMode" class="min-w-0">
          客户端筛选 · 已加载 {{ rankStore.fullLoadedRows }} 行
          <template v-if="rankStore.fullFetchState === 'truncated'">
            （已达 2000 行上限，结果可能不完整）
          </template>
        </span>
        <span v-else-if="rankStore.fullFetchState === 'loading'">正在拉取全量榜单…</span>
        <span v-else-if="rankStore.fullFetchState === 'error'">
          全量榜单拉取失败，当前仅筛选本页数据
        </span>
        <span v-else>客户端筛选（等待快照）</span>
        <button
          type="button"
          class="ml-auto shrink-0 rounded-md border border-slate-200 bg-white px-2 py-0.5 font-medium text-slate-600 transition-colors enabled:hover:bg-slate-100 disabled:cursor-not-allowed disabled:opacity-40"
          :disabled="rankStore.isLoading"
          @click="manualRefresh"
        >
          手动刷新
        </button>
      </div>

      <div
        class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-[var(--border-color)] bg-white shadow-sm"
      >
        <LoadingSpinner v-if="isBootstrapping" message="正在加载榜单…" class="m-auto" />

        <div
          v-else-if="isUpcoming"
          class="flex flex-1 flex-col items-center justify-center gap-2 p-8 text-center"
        >
          <span class="text-3xl">⏳</span>
          <p class="text-sm font-medium text-[var(--text-primary)]">
            比赛尚未开始，榜单将在开赛后显示
          </p>
          <p class="text-xs text-[var(--text-muted)]">页面会自动刷新，无需手动操作</p>
        </div>

        <ErrorMessage
          v-else-if="isFatalError && rankStore.error"
          :message="rankStore.error"
          :retry="retry"
          class="m-auto"
        />

        <ScoreboardTable v-else />

        <!-- 底栏：刷新状态 + 分页 -->
        <div
          v-if="showTable"
          class="flex shrink-0 flex-wrap items-center justify-between gap-3 border-t border-[var(--border-color)] px-4 py-2"
        >
          <div class="flex items-center gap-2 text-xs text-[var(--text-secondary)]">
            <span
              class="inline-block h-2 w-2 rounded-full"
              :class="
                isEnded
                  ? 'bg-[var(--text-muted)]'
                  : rankStore.isLive && !rankStore.isFullMode
                    ? 'animate-pulse bg-emerald-500'
                    : 'bg-amber-400'
              "
            ></span>
            <span class="font-medium">{{ liveText }}</span>
            <span v-if="updatedText" class="font-mono text-[var(--text-muted)]">
              · {{ updatedText }}
            </span>
          </div>

          <div v-if="rankStore.pages > 1" class="flex items-center gap-2 text-xs">
            <span class="font-mono text-[var(--text-muted)]">{{ totalLabel }}</span>
            <div class="flex items-center gap-1">
              <button
                type="button"
                class="rounded-md border border-[var(--border-color)] px-2 py-1 transition-colors enabled:hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-40"
                :disabled="rankStore.current <= 1 || rankStore.isLoading"
                @click="goToPage(rankStore.current - 1)"
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
                    item === rankStore.current
                      ? 'border-[var(--color-primary)] bg-[var(--color-primary)] font-semibold text-white'
                      : 'border-[var(--border-color)] enabled:hover:bg-slate-50'
                  "
                  :disabled="rankStore.isLoading"
                  @click="goToPage(item)"
                >
                  {{ item }}
                </button>
              </template>
              <button
                type="button"
                class="rounded-md border border-[var(--border-color)] px-2 py-1 transition-colors enabled:hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-40"
                :disabled="rankStore.current >= rankStore.pages || rankStore.isLoading"
                @click="goToPage(rankStore.current + 1)"
              >
                下一页
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
