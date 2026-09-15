<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import RankCell from '@/components/rank/RankCell.vue'
import { useContestStore } from '@/stores/contestStore'
import { useRankStore } from '@/stores/rankStore'
import type { ContestRankRow } from '@/types/rank'
import {
  formatPenaltyMinutes,
  formatRankTime,
  resolveDisplayName,
  resolveOiRankCell,
  resolveRowKind,
} from '@/utils/rank'
import type { OiCellKind } from '@/utils/rank'

/// 榜单表格：粘性表头 + 粘性「我的行」+ 粘性前两列（横向滚动时排名/选手不消失）。
const contestStore = useContestStore()
const rankStore = useRankStore()

/**
 * 赛制分流。
 *
 * HOJ 的 ACM / OI 是两套 VO（文档 §2.3 / §2.4），字段语义与单位都不同，必须按
 * `contest.contestType` 分流；`contest` 尚未加载时按 ACM 处理（Hinina 面向 ICPC 场景）。
 */
const isAcm = computed(() => contestStore.contest?.contestType !== 1)
const rankShowName = computed(() => contestStore.contest?.rankShowName ?? 'username')
const problems = computed(() => contestStore.problems)

/// 表头实测高度：粘性「我的行」要贴在表头正下方，写死像素会随字体/缩放/列头换行漂移
const headRef = ref<HTMLElement | null>(null)
const headHeight = ref(45)
let headObserver: ResizeObserver | null = null

function measureHead() {
  const height = headRef.value?.offsetHeight ?? 0
  if (height > 0) headHeight.value = height
}

onMounted(() => {
  measureHead()
  if (typeof ResizeObserver === 'undefined' || !headRef.value) return
  headObserver = new ResizeObserver(() => measureHead())
  headObserver.observe(headRef.value)
})

onBeforeUnmount(() => {
  headObserver?.disconnect()
  headObserver = null
})

interface BodyRow {
  row: ContestRankRow
  isMe: boolean
}

/**
 * 正文行 = 粘性「我的行」+ 当前页可见行。
 *
 * HOJ 会把当前登录用户前置复制到 records（文档 §9.2），因此翻到任意页都能取到 `myRow`，
 * 固定住它选手才不用在几百行里找自己。它在正文中的自然位置仍照常渲染
 * （`rows` 已按 uid 去重，正文不会出现两遍），与 Codeforces 等榜单的浮标行一致。
 */
const bodyRows = computed<BodyRow[]>(() => {
  const list: BodyRow[] = rankStore.visibleRows.map((row) => ({ row, isMe: false }))
  if (rankStore.myRow) list.unshift({ row: rankStore.myRow, isMe: true })
  return list
})

/// 行背景：打星（不计排名）与女生队按 HOJ 文档 §8 `userCellClassName` 着色
function rowTint(row: ContestRankRow): string {
  switch (resolveRowKind(row)) {
    case 'star':
      return 'bg-[var(--color-star-row-bg)]'
    case 'female':
      return 'bg-[var(--color-girls-row-bg)]'
    default:
      return ''
  }
}

/**
 * 粘性单元格必须自带**不透明**背景。
 *
 * `position: sticky` 的 td 会被单独提升绘制，行背景（画在 tr 上）不跟着它横向移动，
 * 右侧题目格滚动时会从它底下透出来。因此按行类型补一层实底色。
 *
 * 行 hover 同理：tr 的 `hover:bg-slate-50/80` 不会作用到被提升绘制的粘性 td，
 * 需要用 `group-hover` 在单元格上补一层等价的实底色，否则悬停时前两列会「不跟着变色」。
 */
function stickyTint({ row, isMe }: BodyRow): string {
  if (isMe) return 'bg-slate-50'
  const hover = 'group-hover:bg-slate-50'
  switch (resolveRowKind(row)) {
    case 'star':
      return `bg-[var(--color-star-row-bg)] ${hover}`
    case 'female':
      return `bg-[var(--color-girls-row-bg)] ${hover}`
    default:
      return `bg-white ${hover}`
  }
}

/**
 * 「我的行」的紫色下划线用**单元格内阴影**而不是 tr 的 border 实现。
 *
 * `border-collapse: collapse` 下边框属于表格的合并边框模型，Chromium 不会让它跟着
 * sticky 行一起绘制，滚动时这条分隔线会消失；内阴影画在单元格自身上，始终可见。
 */
const MY_ROW_EDGE = 'shadow-[inset_0_-2px_0_var(--color-primary)]'
/// 选手列是最后一个粘性列，右侧再加一条竖线，横向滚动时提示「下面还有内容」
const COLUMN_EDGE = 'shadow-[1px_0_0_var(--border-color)]'
const MY_ROW_COLUMN_EDGE =
  'shadow-[1px_0_0_var(--border-color),inset_0_-2px_0_var(--color-primary)]'

function rankCellClass(item: BodyRow): string {
  const edge = item.isMe ? MY_ROW_EDGE : ''
  return `sticky left-0 z-10 px-3 py-2.5 text-center font-bold ${stickyTint(item)} ${edge}`
}

function nameCellClass(item: BodyRow): string {
  const edge = item.isMe ? MY_ROW_COLUMN_EDGE : COLUMN_EDGE
  return `sticky left-16 z-10 px-4 py-2.5 font-sans ${stickyTint(item)} ${edge}`
}

/// 普通（非粘性）单元格在「我的行」里也要带下划线，否则紫线只画出前两列
function plainCellClass(item: BodyRow): string {
  return item.isMe ? MY_ROW_EDGE : ''
}

/// 名次文本：`rank === -1` 是打星队伍（不参与排名），按 ICPC 习惯显示 `*`
function rankLabel({ row, isMe }: BodyRow): string {
  if (row.rank === -1) return '*'
  return isMe ? `#${row.rank}` : String(row.rank)
}

/// 前 3 名用琥珀色圆形徽章（设计稿给前 10 名都上了色，这里按需求收敛到领奖台三名）
function isPodium(row: ContestRankRow): boolean {
  return row.rank >= 1 && row.rank <= 3
}

/// 解题列：ACM 是 AC 题数，OI 是总得分
function solvedText(row: ContestRankRow): string {
  if (isAcm.value) return String(row.ac)
  return row.totalScore === null ? '—' : String(row.totalScore)
}

/**
 * 总用时列 —— 两套 VO 的 `totalTime` 单位不同，混用会把 OI 用时放大 1000 倍：
 * - ACM：**总罚时（秒）** = Σ(errorNum × 20min + ACTime)，按 ICPC 习惯换算成分钟整数；
 * - OI：**AC 提交耗时之和（毫秒）**，与罚时无关，须先 ÷1000 再按时长格式化；
 *   没有 AC 提交时为 0，此时显示 `—` 而不是 `00:00`（0 会被误读成「用时极短」）。
 */
function totalTimeText(row: ContestRankRow): string {
  if (isAcm.value) return formatPenaltyMinutes(row.totalTime)
  if (!row.totalTime || row.totalTime <= 0) return '—'
  return formatRankTime(Math.floor(row.totalTime / 1000))
}

/// 气球色回退调色板：HOJ 允许题目未配色（`color` 为空串），按列序号取色保证列头永远可辨
const BALLOON_FALLBACK: readonly string[] = [
  '#e11d48',
  '#10b981',
  '#2563eb',
  '#9333ea',
  '#f59e0b',
  '#0891b2',
  '#db2777',
  '#65a30d',
  '#7c3aed',
  '#ea580c',
]

function balloonColor(index: number, color: string): string {
  const trimmed = color.trim()
  return trimmed ? trimmed : BALLOON_FALLBACK[index % BALLOON_FALLBACK.length]
}

interface OiCellView {
  score: string
  time: string
  box: string
  hint: string
}

/// 档位 → 配色。判档与文本由 `utils/rank.resolveOiRankCell` 给出（纯函数、有测试），组件只管样式
const OI_BOX: Record<OiCellKind, string> = {
  full: 'border border-[var(--color-ac-border)] bg-[var(--color-ac-bg)] text-emerald-700',
  partial:
    'border border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-amber-700',
  zero: 'border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] text-rose-600',
  none: '',
}

/**
 * OI 单元格视图。
 *
 * 不能复用 `RankCell.vue`：OI 的 `submissionInfo` 值是整数得分（Adapter 归一到
 * `RankCell.score`，其余字段取默认值），走 ACM 判据会全部落到 `none`、整张榜单变成一片 `-`。
 */
function oiCellView(row: ContestRankRow, displayId: string): OiCellView {
  const resolved = resolveOiRankCell(row.submissionInfo[displayId], row.timeInfo[displayId])
  return {
    score: resolved.scoreText,
    time: resolved.timeText ?? '',
    box: OI_BOX[resolved.kind],
    hint: resolved.hint,
  }
}

/// 按行预计算 OI 单元格：模板里逐格调函数会在每次重渲染时重复构造对象（每行 N 题）
const oiCellsByUid = computed(() => {
  const map = new Map<string, OiCellView[]>()
  if (isAcm.value) return map
  for (const { row } of bodyRows.value) {
    map.set(
      row.uid,
      problems.value.map((problem) => oiCellView(row, problem.displayId)),
    )
  }
  return map
})

const EMPTY_CELLS: OiCellView[] = []

function oiCells(row: ContestRankRow): OiCellView[] {
  return oiCellsByUid.value.get(row.uid) ?? EMPTY_CELLS
}
</script>

<template>
  <div class="relative min-h-0 flex-1 overflow-auto">
    <table class="w-full border-collapse text-left text-xs">
      <thead ref="headRef" class="sticky top-0 z-30 shadow-xs">
        <tr
          class="border-b border-[var(--border-color)] bg-slate-50/95 text-[var(--text-secondary)] backdrop-blur-sm select-none"
        >
          <th
            class="sticky left-0 z-10 w-16 min-w-[64px] bg-[#f8fafc] px-3 py-2.5 text-center font-semibold"
          >
            排名
          </th>
          <th
            class="sticky left-16 z-10 min-w-[220px] bg-[#f8fafc] px-4 py-2.5 font-semibold shadow-[1px_0_0_var(--border-color)]"
          >
            选手 / 团队
          </th>
          <th class="w-16 px-2 py-2.5 text-center font-semibold text-[var(--text-primary)]">
            {{ isAcm ? '解题' : '得分' }}
          </th>
          <th class="w-20 px-2 py-2.5 text-center font-semibold text-[var(--text-primary)]">
            {{ isAcm ? '总用时' : '总耗时' }}
          </th>
          <th
            v-for="(problem, index) in problems"
            :key="problem.displayId"
            class="w-20 px-1.5 py-2 text-center"
          >
            <div class="flex flex-col items-center">
              <span
                class="flex h-6 w-6 items-center justify-center rounded-md text-xs font-bold text-white shadow-xs"
                :style="{ backgroundColor: balloonColor(index, problem.color) }"
                :title="problem.displayTitle"
              >
                {{ problem.displayId }}
              </span>
              <span class="mt-0.5 font-mono text-[10px] font-normal text-[var(--text-muted)]">
                {{ problem.ac }}/{{ problem.total }}
              </span>
            </div>
          </th>
        </tr>
      </thead>

      <tbody class="divide-y divide-slate-100 font-mono">
        <tr
          v-for="item in bodyRows"
          :key="`${item.isMe ? 'me' : 'row'}-${item.row.uid}`"
          class="group transition-colors"
          :class="
            item.isMe
              ? 'sticky z-20 bg-slate-50'
              : `${rowTint(item.row)} hover:bg-slate-50/80`
          "
          :style="item.isMe ? { top: `${headHeight}px` } : undefined"
        >
          <td :class="rankCellClass(item)">
            <div class="inline-flex items-center justify-center gap-1">
              <span
                v-if="item.isMe"
                class="rounded bg-[var(--color-primary)] px-1 font-sans text-[9px] font-bold leading-4 text-white"
                title="我的队伍"
              >
                我
              </span>
              <span
                v-if="isPodium(item.row)"
                class="inline-flex h-6 w-6 items-center justify-center rounded-full border border-amber-300 bg-amber-100 font-sans text-xs font-bold text-amber-800 shadow-xs"
              >
                {{ rankLabel(item) }}
              </span>
              <span
                v-else
                class="font-mono text-xs font-bold"
                :class="item.row.rank === -1 ? 'text-[var(--text-muted)]' : 'text-slate-700'"
                :title="item.row.rank === -1 ? '打星队伍，不参与排名' : undefined"
              >
                {{ rankLabel(item) }}
              </span>
            </div>
          </td>

          <td :class="nameCellClass(item)">
            <div class="flex items-center space-x-2">
              <span
                class="font-bold"
                :class="item.isMe ? 'text-[var(--color-primary)]' : 'text-slate-900'"
              >
                {{ resolveDisplayName(item.row, rankShowName) }}
              </span>
              <template v-if="item.row.school">
                <span class="text-slate-300">·</span>
                <span class="font-mono text-xs text-slate-500">{{ item.row.school }}</span>
              </template>
            </div>
          </td>

          <td
            class="px-2 py-2.5 text-center text-sm font-bold"
            :class="[isAcm ? 'text-emerald-600' : 'text-slate-700', plainCellClass(item)]"
          >
            {{ solvedText(item.row) }}
          </td>
          <td
            class="px-2 py-2.5 text-center font-medium text-slate-600"
            :class="plainCellClass(item)"
          >
            {{ totalTimeText(item.row) }}
          </td>

          <template v-if="isAcm">
            <td
              v-for="problem in problems"
              :key="problem.displayId"
              class="p-1 text-center"
              :class="plainCellClass(item)"
            >
              <RankCell :cell="item.row.submissionInfo[problem.displayId]" />
            </td>
          </template>
          <template v-else>
            <td
              v-for="(cellView, index) in oiCells(item.row)"
              :key="index"
              class="p-1 text-center"
              :class="plainCellClass(item)"
            >
              <div v-if="cellView.box" class="rounded p-1" :class="cellView.box" :title="cellView.hint">
                <div class="font-mono text-[11px] font-bold leading-tight">{{ cellView.score }}</div>
                <div class="font-sans text-[10px] leading-tight opacity-80">
                  {{ cellView.time || '未 AC' }}
                </div>
              </div>
              <div v-else class="font-sans text-[11px] text-slate-300" :title="cellView.hint">
                {{ cellView.score }}
              </div>
            </td>
          </template>
        </tr>

        <tr v-if="bodyRows.length === 0">
          <td
            :colspan="4 + problems.length"
            class="px-4 py-12 text-center font-sans text-sm text-[var(--text-muted)]"
          >
            当前筛选条件下没有榜单记录
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
