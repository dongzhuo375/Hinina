<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ContestProblem } from '@/types/contest'
import { useProblemStore } from '@/stores/problemStore'
import { useRankStore } from '@/stores/rankStore'
import { formatLimitsSummary } from '@/utils/limits'
import { formatRankTime } from '@/utils/rank'
import QuickSubmitDialog from '@/components/problem/QuickSubmitDialog.vue'

const props = defineProps<{ problem: ContestProblem }>()
const emit = defineEmits<{ open: [] }>()

const problemStore = useProblemStore()
const rankStore = useRankStore()

/// 快捷提交弹窗开关（弹窗内完成提交与评测跟踪，不离开题目总览）
const quickSubmitOpen = ref(false)

/// 徽章回退调色板（取自设计稿 A–F 六卡配色）：HOJ 气球色可能为空串（组织者未配置），
/// 此时按题号序号循环取色 —— 同一题号恒定同色，轮询刷新时徽章不会跳变
const FALLBACK_PALETTE = ['#8c6a43', '#5f7a6a', '#7d7378', '#6c7565', '#9c8562', '#526b69']

/** 题号序号：字母题号（A/B/C…）映射 0/1/2…，纯数字题号按数值，其余回退 0 */
function displayIdOrdinal(displayId: string): number {
  const id = displayId.trim().toUpperCase()
  if (/^[A-Z]$/.test(id)) return id.charCodeAt(0) - 'A'.charCodeAt(0)
  const n = Number.parseInt(id, 10)
  return Number.isFinite(n) && n > 0 ? n - 1 : 0
}

const badgeColor = computed(() => {
  if (props.problem.color) return props.problem.color
  return FALLBACK_PALETTE[displayIdOrdinal(props.problem.displayId) % FALLBACK_PALETTE.length]
})

const title = computed(() => props.problem.displayTitle || `Problem ${props.problem.displayId}`)

const limits = computed(() => problemStore.limitsOf(props.problem.displayId))

/// 状态 pill：我的提交状态接口与榜单「我的行」可能各自缺失，AC 判定取两者并集；
/// AC 时优先展示榜单里的 AC 用时（设计稿的「AC 00:08」，零额外请求）
const pill = computed(() => {
  const cell = rankStore.myRow?.submissionInfo[props.problem.displayId]
  const rankAc = cell?.isAc === true
  const status = problemStore.statusOf(props.problem.problemId)
  if (status === 1 || rankAc) {
    const text = rankAc && cell?.acTime != null ? `AC ${formatRankTime(cell.acTime)}` : '已通过'
    return { kind: 'ac' as const, text }
  }
  if (status === 2) return { kind: 'attempted' as const, text: '未通过' }
  return { kind: 'none' as const, text: '未作答' }
})
</script>

<template>
  <article class="problem-row">
    <button class="problem-ordinal" :aria-label="`打开题目 ${problem.displayId}`" @click="emit('open')">
      <span>{{ problem.displayId }}</span><span class="balloon-line" :style="{ backgroundColor: badgeColor }" aria-hidden="true"></span>
    </button>
    <div class="problem-description">
      <button class="problem-title" @click="emit('open')">{{ title }}</button>
      <p class="problem-limits">
        <span v-if="limits">{{ formatLimitsSummary(limits) }}</span>
        <span v-else>{{ problemStore.isLimitsLoading ? '正在读取限制…' : '限制 —' }}</span>
      </p>
    </div>
    <div class="problem-ratio"><span>{{ problem.ac }} <span class="text-[var(--text-muted)]">/ {{ problem.total }}</span></span><span class="eyebrow">通过 / 提交</span></div>
    <span class="problem-state" :class="pill.kind"><span aria-hidden="true">{{ pill.kind === 'ac' ? '✓' : pill.kind === 'attempted' ? '·' : '—' }}</span>{{ pill.text }}</span>
    <div class="problem-actions">
      <router-link :to="{ name: 'Submissions', query: { problem: problem.displayId } }" class="action-quiet">评测记录</router-link>
      <button class="action-quiet" @click="quickSubmitOpen = true">快捷提交</button>
      <button class="problem-open" :aria-label="`查看题目 ${problem.displayId}`" @click="emit('open')">查看题目 <span aria-hidden="true">↗</span></button>
    </div>
    <QuickSubmitDialog v-if="quickSubmitOpen" :problem="problem" @close="quickSubmitOpen = false" />
  </article>
</template>
<style scoped>
.problem-row { display: grid; grid-template-columns: 48px minmax(180px, 1fr) 96px 100px auto; align-items: center; gap: calc(20px + var(--ui-step) * 4); min-height: calc(72px + var(--ui-step) * 4); padding: 10px 0; border-top: 1px solid var(--border-color); transition: background 200ms ease-out; }
.problem-row:hover { background: #ffffff70; }
.problem-ordinal { position: relative; align-self: stretch; font: calc(30px + var(--ui-step) * 3) var(--font-mono); text-align: left; padding-left: 8px; }
.balloon-line { position: absolute; bottom: 2px; left: 9px; width: 18px; height: 2px; opacity: .65; }
.problem-description { min-width: 0; }
.problem-title { font-size: calc(15px + var(--ui-step)); font-weight: 600; text-align: left; overflow-wrap: anywhere; line-height: 1.4; }
.problem-title:hover { color: #8c6a43; }
.problem-limits { margin-top: 4px; font: var(--text-xs)/1.4 var(--font-mono); color: var(--text-muted); }
.problem-ratio { display: flex; flex-direction: column; gap: 2px; font: var(--text-sm)/1.5 var(--font-mono); font-variant-numeric: tabular-nums; }
.problem-ratio .eyebrow { font-family: var(--font-sans); font-size: var(--text-xs); letter-spacing: .03em; }
.problem-state { display: flex; align-items: center; gap: 6px; font: var(--text-xs) var(--font-mono); color: var(--text-muted); white-space: nowrap; }
.problem-state.ac { color: var(--color-success); }
.problem-state.attempted { color: var(--color-warning); }
.problem-actions { display: flex; align-items: center; gap: 4px; }
.problem-open { display: flex; align-items: center; gap: 10px; padding: 8px; font-size: var(--text-xs); font-weight: 600; white-space: nowrap; }
.problem-open:hover { color: #8c6a43; }
@media (max-width: 1100px) { .problem-row { grid-template-columns: 44px minmax(130px, 1fr) 85px 95px auto; gap: 12px; } .problem-actions { gap: 0; } .problem-actions .action-quiet { padding: 8px 6px; } }
@media (max-width: 800px) { .problem-row { grid-template-columns: 48px 1fr 90px; padding: 16px 0; } .problem-actions { grid-column: 2 / -1; } .problem-state { grid-column: 2; } .balloon-line { bottom: 0; } }
</style>
