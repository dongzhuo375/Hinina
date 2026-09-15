<script setup lang="ts">
import { computed } from 'vue'
import type { RankCell } from '@/types/rank'
import { resolveRankCell } from '@/utils/rank'
import type { RankCellKind } from '@/utils/rank'

/// 榜单单元格（ACM 赛制）。
///
/// 判据（一血 / 通过 / 赛后通过 / 封榜 / 未通过）全部由 `utils/rank.resolveRankCell`
/// 给出，本组件只做「类别 → 配色 + 两行文案」的呈现，不重复实现任何规则。
const props = defineProps<{ cell: RankCell | undefined }>()

const resolved = computed(() => resolveRankCell(props.cell))

/**
 * 类别 → 配色。色值一律取 `global.css` 的榜单状态色变量，与设计稿 arena.* 对齐。
 *
 * `after-ac` 复用 `ac` 的配色：赛后通过与赛中通过在榜单上同级展示，
 * 差异只由 `timeText` 的 `*` 前缀与 title 承担（HOJ 文档 §8 `isAfterContest`）。
 */
const SKIN: Record<RankCellKind, { box: string; top: string; bottom: string }> = {
  'first-ac': {
    box: 'bg-[var(--color-first-ac)] shadow-xs',
    top: 'text-white',
    bottom: 'text-emerald-100',
  },
  ac: {
    box: 'border border-[var(--color-ac-border)] bg-[var(--color-ac-bg)]',
    top: 'text-emerald-700',
    bottom: 'text-emerald-600',
  },
  'after-ac': {
    box: 'border border-[var(--color-ac-border)] bg-[var(--color-ac-bg)]',
    top: 'text-emerald-700',
    bottom: 'text-emerald-600',
  },
  sealed: {
    box: 'border border-[var(--color-frozen-border)] bg-[var(--color-frozen-bg)]',
    top: 'text-slate-600',
    bottom: 'text-slate-500',
  },
  wa: {
    box: 'border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)]',
    top: 'text-rose-600',
    bottom: 'text-rose-500',
  },
  none: { box: '', top: '', bottom: '' },
}

const skin = computed(() => SKIN[resolved.value.kind])

/**
 * 两行文案。
 *
 * AC 类是「通过时间 / 尝试次数」；`wa` 与 `sealed` 没有时间可显示，
 * 把尝试次数提到首行（与设计稿一致：`-4` 大字 + `WA 未过` 小字），
 * 否则首行会空掉、单元格高度塌陷导致整行错位。
 */
const lines = computed(() => {
  const r = resolved.value
  if (r.kind === 'wa') return { top: r.triesText, bottom: '未通过' }
  if (r.kind === 'sealed') return { top: r.triesText, bottom: '封榜' }
  return { top: r.timeText, bottom: r.triesText }
})

/// 悬浮说明：赛场上选手需要一眼看懂格子里的数字是什么口径（尤其封榜与赛后提交）
const hint = computed(() => {
  const cell = props.cell
  const tries = (cell?.errorNum ?? 0) + 1
  switch (resolved.value.kind) {
    case 'first-ac':
      return `一血（全场首个通过）· ${tries} 次尝试`
    case 'ac':
      return `已通过，${tries} 次尝试`
    case 'after-ac':
      return `比赛结束后提交通过（不计入罚时），${tries} 次尝试`
    case 'sealed':
      return `封榜期间提交 ${cell?.tryNum ?? 0} 次，结果赛后才揭晓`
    case 'wa':
      return `尝试 ${cell?.errorNum ?? 0} 次未通过`
    default:
      return '暂无提交'
  }
})
</script>

<template>
  <div
    v-if="resolved.kind === 'none'"
    class="text-center font-sans text-[11px] text-slate-300"
    :title="hint"
  >
    -
  </div>
  <div v-else class="rounded p-1" :class="skin.box" :title="hint">
    <div class="font-mono text-[11px] font-bold leading-tight" :class="skin.top">
      {{ lines.top }}
    </div>
    <div
      class="flex items-center justify-center gap-0.5 font-sans text-[10px] font-medium leading-tight"
      :class="skin.bottom"
    >
      <span>{{ lines.bottom }}</span>
      <svg
        v-if="resolved.kind === 'first-ac'"
        class="h-2.5 w-2.5 shrink-0"
        viewBox="0 0 24 24"
        fill="currentColor"
        aria-hidden="true"
      >
        <path d="M13 2 4.5 13.5H11L10 22l8.5-11.5H12L13 2z" />
      </svg>
    </div>
  </div>
</template>
