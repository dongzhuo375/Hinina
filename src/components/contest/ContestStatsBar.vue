<script setup lang="ts">
import { computed } from 'vue'
import { useContestStore } from '@/stores/contestStore'
import { useRankStore } from '@/stores/rankStore'
import { formatPenaltyMinutes } from '@/utils/rank'

/// 统计卡（题目总览与榜单共用）：解题进度 / 实时排名 / 总罚时。
///
/// 数据源是**榜单中「我的行」**（HOJ 会把当前用户前置复制到 records，因此任意页都能取到），
/// 不额外发请求。榜单尚未加载或我没有名次时显示占位符 `—`，**不显示 0** ——
/// 0 会被误读成「一题未解 / 排名第 0」，占位符才明确表达「数据尚未取得」。
const contest = useContestStore()
const rank = useRankStore()

/// OI 赛制的 totalTime 是毫秒且语义为耗时，罚时口径不适用
const isAcm = computed(() => contest.contest?.contestType !== 1)

const solvedText = computed(() => {
  const ac = rank.myRow?.ac
  const total = contest.problems.length
  return {
    value: ac === undefined || ac === null ? '—' : String(ac),
    unit: total > 0 ? `/ ${total} AC` : 'AC',
  }
})

const rankText = computed(() => {
  const my = rank.myRow?.rank
  // rank === -1 表示打星队伍，不参与排名
  const value = my === undefined || my === null || my === -1 ? '—' : `#${my}`
  return { value, unit: rank.participants > 0 ? `/ ${rank.participants}` : '' }
})

const penaltyText = computed(() => {
  const my = rank.myRow
  if (!my) return { label: '总罚时', value: '—' }
  return isAcm.value
    ? { label: '总罚时', value: `${formatPenaltyMinutes(my.totalTime)}m` }
    : { label: '总得分', value: my.totalScore === null ? '—' : String(my.totalScore) }
})
</script>

<template>
  <div
    class="flex shrink-0 items-center gap-4 rounded-xl border border-[var(--border-color)] bg-[var(--bg-body)] px-4 py-2"
    :title="rank.lastUpdated ? `榜单更新于 ${new Date(rank.lastUpdated).toLocaleTimeString('zh-CN', { hour12: false })}` : '榜单数据尚未加载'"
  >
    <div class="px-2 text-center">
      <span class="block text-[11px] font-medium text-[var(--text-secondary)]">解题进度</span>
      <span class="font-mono text-base font-bold text-[var(--color-success)]">
        {{ solvedText.value }}
        <span class="text-xs font-normal text-[var(--text-muted)]">{{ solvedText.unit }}</span>
      </span>
    </div>

    <div class="h-6 w-px bg-[var(--border-color)]"></div>

    <div class="px-2 text-center">
      <span class="block text-[11px] font-medium text-[var(--text-secondary)]">实时排名</span>
      <span class="font-mono text-base font-bold text-[var(--text-primary)]">
        {{ rankText.value }}
        <span class="text-xs font-normal text-[var(--text-muted)]">{{ rankText.unit }}</span>
      </span>
    </div>

    <div class="h-6 w-px bg-[var(--border-color)]"></div>

    <div class="px-2 text-center">
      <span class="block text-[11px] font-medium text-[var(--text-secondary)]">{{ penaltyText.label }}</span>
      <span class="font-mono text-base font-bold text-[var(--text-primary)]">{{ penaltyText.value }}</span>
    </div>
  </div>
</template>
