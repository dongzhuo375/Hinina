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
const FALLBACK_PALETTE = ['#dc2626', '#00c853', '#2563eb', '#9333ea', '#f59e0b', '#0891b2']

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
  if (status === 2) return { kind: 'attempted' as const, text: '尝试过' }
  return { kind: 'none' as const, text: '未作答' }
})
</script>

<template>
  <article
    class="group flex min-h-[168px] flex-col justify-between border-t border-[var(--border-color)] px-5 py-4 transition-colors duration-200 ease-out hover:bg-[var(--bg-card)]"
  >
    <!-- 上区：气球色字块 + 标题/限制 + 状态 -->
    <div class="flex items-start justify-between gap-3">
      <div class="flex min-w-0 items-start gap-3.5">
        <!-- 字母字块：56px 方角，底色优先用 HOJ 气球色 -->
        <div
          class="flex h-14 w-14 shrink-0 items-center justify-center rounded-[2px] font-mono text-2xl font-bold text-white"
          :style="{ backgroundColor: badgeColor }"
        >
          {{ problem.displayId }}
        </div>
        <!-- min-w-0：三列窄卡片下允许标题块收缩换行，而不是撑破卡片 -->
        <div class="min-w-0">
          <h3
            class="cursor-pointer text-[15px] font-semibold leading-6 tracking-tight text-[var(--text-primary)] transition-colors duration-200 ease-out group-hover:text-[#8c6a43]"
            @click="emit('open')"
          >
            {{ title }}
          </h3>
          <p class="mt-1 font-mono text-[10px] tracking-[0.15em] text-[var(--text-muted)]">
            LIMITS
            <span v-if="limits" class="ml-1 tracking-normal text-[var(--text-secondary)]">{{ formatLimitsSummary(limits) }}</span>
            <!-- limits 未到达：加载中显示脉冲骨架，失败定格为 —，不编造默认值 -->
            <span
              v-else-if="problemStore.isLimitsLoading"
              class="ml-1 inline-block h-2.5 w-16 animate-pulse bg-[var(--bg-sidebar)] align-middle"
            ></span>
            <span v-else class="ml-1 tracking-normal text-[var(--text-secondary)]">—</span>
          </p>
        </div>
      </div>

      <!-- 右上状态：已通过（竹青）/ 尝试过（木色）/ 未作答（灰），纯文字层级无 pill 框 -->
      <div v-if="pill.kind === 'ac'" class="flex shrink-0 items-center gap-1 font-mono text-[11px] font-medium text-[#4a6153]">
        <svg class="h-3.5 w-3.5" fill="currentColor" viewBox="0 0 20 20">
          <path
            clip-rule="evenodd"
            d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
            fill-rule="evenodd"
          ></path>
        </svg>
        <span>{{ pill.text }}</span>
      </div>
      <div v-else-if="pill.kind === 'attempted'" class="flex shrink-0 items-center gap-1.5 font-mono text-[11px] font-medium text-[#8c6a43]">
        <span class="h-1.5 w-1.5 animate-ping rounded-full bg-[#8c6a43]"></span>
        <span>{{ pill.text }}</span>
      </div>
      <span v-else class="shrink-0 font-mono text-[11px] text-[var(--text-muted)]">{{ pill.text }}</span>
    </div>

    <!-- 中区：通过数据（仪器行） -->
    <div class="mt-3 flex items-baseline gap-2 border-t border-[var(--border-color)] pt-3 font-mono">
      <span class="text-[9px] tracking-[0.25em] text-[var(--text-muted)]">SOLVED</span>
      <span class="tabular-nums text-sm font-semibold text-[#5f7a6a]">{{ problem.ac }}</span>
      <span class="tabular-nums text-[11px] text-[var(--text-muted)]">/ {{ problem.total }}</span>
    </div>

    <!-- 下区：操作行（发丝线分隔） -->
    <div class="mt-3 flex items-center justify-between border-t border-[var(--border-color)] pt-2.5">
      <div class="flex items-center gap-4">
        <button
          class="flex cursor-pointer items-center gap-1 text-xs font-medium text-[var(--text-muted)] transition-colors duration-200 ease-out hover:text-[#8c6a43]"
          @click="quickSubmitOpen = true"
        >
          <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
            ></path>
          </svg>
          <span>快捷提交</span>
        </button>
        <!-- 评测记录：跳到评测页并携带题目筛选（?problem=displayId 自动过滤本题） -->
        <router-link
          :to="{ name: 'Submissions', query: { problem: problem.displayId } }"
          class="flex items-center gap-1.5 text-xs font-medium text-[var(--text-muted)] transition-colors duration-200 ease-out hover:text-[#8c6a43]"
        >
          <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
            ></path>
          </svg>
          <span>评测记录</span>
        </router-link>
      </div>
      <button
        class="flex items-center gap-1 rounded-[2px] bg-[#2e3033] px-3.5 py-1.5 text-xs font-medium text-[#f7f6f1] transition-colors duration-200 ease-out hover:bg-[#232527]"
        @click="emit('open')"
      >
        <span>查看题目 →</span>
      </button>
    </div>

    <!-- 快捷提交弹窗：v-if 保证同一时刻至多挂载一个实例 -->
    <QuickSubmitDialog v-if="quickSubmitOpen" :problem="problem" @close="quickSubmitOpen = false" />
  </article>
</template>
