<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useContestStore } from '@/stores/contestStore'
import { useRankStore } from '@/stores/rankStore'
import type { RankGroupFilter } from '@/stores/rankStore'

/// 榜单工具条：搜索 + 分组切换 + 图例。
const rankStore = useRankStore()
const contestStore = useContestStore()

/// 搜索框本地态：与 `rankStore.keyword`（服务端已生效的关键词）解耦，防抖后才提交
const input = ref(rankStore.keyword)
let debounce: ReturnType<typeof setTimeout> | null = null

/**
 * 300ms 防抖。
 *
 * HOJ 内榜是「全量重算后再分页」（文档 §9.9），逐字符触发请求会在赛场上把 OJ 打爆；
 * 且服务端 keyword 会重置到第 1 页，抖动过程中反复跳页也会让表格闪烁。
 */
watch(input, (value) => {
  if (debounce) clearTimeout(debounce)
  debounce = setTimeout(() => {
    const keyword = value.trim()
    if (keyword === rankStore.keyword) return
    rankStore.setKeyword(keyword).catch(() => {
      // 失败原因已写入 rankStore.error，由 RankView 统一展示
    })
  }, 300)
})

onBeforeUnmount(() => {
  if (debounce) clearTimeout(debounce)
})

const GROUPS: ReadonlyArray<{ value: RankGroupFilter; label: string }> = [
  { value: 'all', label: '全场总榜' },
  { value: 'official', label: '正式参赛队' },
  { value: 'star', label: '打星队 (Star)' },
  { value: 'female', label: '女生队' },
]

function selectGroup(filter: RankGroupFilter) {
  rankStore.setGroupFilter(filter).catch(() => {
    // 同上：official 需要重新请求，失败由 RankView 的错误条兜底
  })
}

/// 图例色块直接复用单元格的配色变量，保证「图例 = 表格里真实出现的样子」
type LegendItem = { label: string; swatch: string }

/**
 * ACM 图例。
 *
 * 不含设计稿里的「待评测 (Pending)」：榜单接口的 `submissionInfo` 没有 pending 字段，
 * `resolveRankCell` 也不会产出该档位 —— 列一个表格里永远不会出现的色块只会误导选手。
 */
const ACM_LEGEND: readonly LegendItem[] = [
  { label: '一血 (First Solve)', swatch: 'bg-[var(--color-first-ac)]' },
  {
    label: '通过 (AC)',
    swatch: 'border border-[var(--color-ac-border)] bg-[var(--color-ac-bg)]',
  },
  {
    label: '未通过 (WA)',
    swatch: 'border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)]',
  },
  {
    label: '封榜 (Sealed)',
    swatch: 'border border-[var(--color-frozen-border)] bg-[var(--color-frozen-bg)]',
  },
]

/// OI 图例：档位口径与 `utils/rank.resolveOiRankCell` 一致
const OI_LEGEND: readonly LegendItem[] = [
  {
    label: '满分',
    swatch: 'border border-[var(--color-ac-border)] bg-[var(--color-ac-bg)]',
  },
  {
    label: '部分分',
    swatch: 'border border-[var(--color-pending-border)] bg-[var(--color-pending-bg)]',
  },
  {
    label: '未得分',
    swatch: 'border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)]',
  },
]

/// 赛制决定图例：ACM 与 OI 是两套 VO，单元格档位完全不同
const legend = computed<readonly LegendItem[]>(() =>
  contestStore.contest?.contestType === 1 ? OI_LEGEND : ACM_LEGEND,
)

/**
 * OI 计分规则徽章文案。
 *
 * `oiRankScoreType` 是比赛属性（服务端只读，文档 §2.4）：`Highest` 取最高分、
 * `Recent` 取最后一次提交；未知取值原样展示。ACM 比赛或未返回时为 null（不渲染）。
 */
const oiScoreTypeText = computed<string | null>(() => {
  const contest = contestStore.contest
  if (contest?.contestType !== 1 || !contest.oiRankScoreType) return null
  switch (contest.oiRankScoreType) {
    case 'Highest':
      return '得分规则：最高分'
    case 'Recent':
      return '得分规则：最近提交'
    default:
      return `得分规则：${contest.oiRankScoreType}`
  }
})
</script>

<template>
  <div
    class="flex flex-wrap items-center justify-between gap-x-3 gap-y-2 border-t border-slate-100 pt-2"
  >
    <div class="flex flex-wrap items-center space-x-3">
      <div class="relative">
        <input
          v-model="input"
          type="text"
          placeholder="搜索队伍 / 学校…"
          title="服务端仅按「学校」或「榜单显示名」匹配；除非榜单显示名规则设为 username，否则搜用户名不会有结果"
          class="w-64 rounded-lg border border-[var(--border-color)] bg-[var(--bg-body)] py-1.5 pl-8 pr-3 text-xs text-[var(--text-primary)] placeholder-[var(--text-muted)] transition-all focus:border-[var(--color-primary)] focus:bg-white focus:outline-none focus:ring-1 focus:ring-[var(--color-primary)]"
        />
        <svg
          class="absolute left-2.5 top-2.5 h-3.5 w-3.5 text-[var(--text-muted)]"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          <path
            d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
          />
        </svg>
      </div>

      <div
        class="flex items-center rounded-lg border border-slate-200/80 bg-slate-100 p-0.5 text-xs"
      >
        <button
          v-for="group in GROUPS"
          :key="group.value"
          type="button"
          class="rounded-md px-3 py-1 transition-colors"
          :class="
            rankStore.groupFilter === group.value
              ? 'bg-white font-medium text-slate-800 shadow-xs'
              : 'text-slate-500 hover:text-slate-800'
          "
          :aria-pressed="rankStore.groupFilter === group.value"
          @click="selectGroup(group.value)"
        >
          {{ group.label }}
        </button>
      </div>
    </div>

    <div class="flex select-none flex-wrap items-center space-x-3 text-xs text-slate-500">
      <span
        v-if="oiScoreTypeText"
        class="rounded-md border border-slate-200 bg-slate-50 px-2 py-0.5 text-[11px] font-medium text-slate-600"
        title="OI 计分规则由比赛属性决定（服务端只读）：最高分 = 取每题最高分；最近提交 = 取最后一次提交的得分"
      >
        {{ oiScoreTypeText }}
      </span>
      <div v-for="item in legend" :key="item.label" class="flex items-center space-x-1.5">
        <span class="inline-block h-2.5 w-2.5 rounded" :class="item.swatch"></span>
        <span class="text-[11px]">{{ item.label }}</span>
      </div>
    </div>
  </div>
</template>
