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
    class="group flex min-h-[175px] flex-col justify-between rounded-xl border border-[var(--border-color)] bg-[var(--bg-card)] p-5 shadow-sm transition-all duration-200 hover:border-slate-300 hover:shadow-md"
  >
    <div class="flex items-start justify-between gap-3">
      <div class="flex min-w-0 items-start space-x-3.5">
        <!-- 字母徽章：40px 圆角方块，底色优先用 HOJ 气球色 -->
        <div
          class="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl font-mono text-lg font-bold text-white shadow-xs"
          :style="{ backgroundColor: badgeColor }"
        >
          {{ problem.displayId }}
        </div>
        <!-- min-w-0：三列窄卡片下允许标题块收缩换行，而不是撑破卡片 -->
        <div class="min-w-0">
          <h3
            class="cursor-pointer text-base font-bold tracking-tight text-[var(--text-primary)] transition group-hover:text-[var(--color-primary)]"
            @click="emit('open')"
          >
            {{ title }}
          </h3>
          <div class="mt-1 space-y-1 font-mono">
            <div class="flex items-center gap-1 text-[11px] text-[var(--text-secondary)]">
              <span>Limits:</span>
              <span v-if="limits">{{ formatLimitsSummary(limits) }}</span>
              <!-- limits 未到达：加载中显示脉冲骨架，失败定格为 —，不编造默认值 -->
              <span
                v-else-if="problemStore.isLimitsLoading"
                class="h-3 w-20 animate-pulse rounded bg-slate-200"
              ></span>
              <span v-else>—</span>
            </div>
            <div class="flex items-center gap-1 text-xs font-medium text-emerald-600">
              <span>{{ problem.ac }} / {{ problem.total }}</span>
              <span class="font-sans text-[var(--text-muted)]">通过</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 右上状态 pill：已通过（绿）/ 尝试过（amber）/ 未作答（灰） -->
      <div
        v-if="pill.kind === 'ac'"
        class="flex shrink-0 items-center space-x-1 rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-1 font-mono text-xs font-medium text-emerald-600"
      >
        <svg class="h-3.5 w-3.5" fill="currentColor" viewBox="0 0 20 20">
          <path
            clip-rule="evenodd"
            d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
            fill-rule="evenodd"
          ></path>
        </svg>
        <span>{{ pill.text }}</span>
      </div>
      <div
        v-else-if="pill.kind === 'attempted'"
        class="flex shrink-0 items-center space-x-1.5 rounded-full border border-amber-200 bg-amber-50 px-2.5 py-1 font-mono text-xs font-medium text-amber-600"
      >
        <span class="h-1.5 w-1.5 animate-ping rounded-full bg-amber-500"></span>
        <span>{{ pill.text }}</span>
      </div>
      <span
        v-else
        class="shrink-0 rounded-full border border-[var(--border-color)] bg-[var(--bg-body)] px-2.5 py-1 font-mono text-xs font-medium text-[var(--text-muted)]"
      >
        {{ pill.text }}
      </span>
    </div>

    <div class="mt-4 flex flex-col justify-end">
      <div class="flex justify-end pb-2">
        <button
          class="flex cursor-pointer items-center gap-1 text-xs font-medium text-[var(--text-muted)] transition hover:text-[var(--text-secondary)]"
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
      </div>
      <div class="flex items-center justify-between border-t border-slate-100 pt-2.5">
        <!-- 评测记录：跳到评测页并携带题目筛选（?problem=displayId 自动过滤本题） -->
        <router-link
          :to="{ name: 'Submissions', query: { problem: problem.displayId } }"
          class="flex items-center gap-1.5 text-xs font-medium text-[var(--text-muted)] transition hover:text-[var(--text-secondary)]"
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
        <button
          class="flex items-center gap-1 rounded-lg bg-emerald-600 px-3.5 py-1.5 text-xs font-medium text-white shadow-xs transition hover:bg-emerald-700"
          @click="emit('open')"
        >
          <span>查看题目 →</span>
        </button>
      </div>
    </div>

    <!-- 快捷提交弹窗：v-if 保证同一时刻至多挂载一个实例 -->
    <QuickSubmitDialog v-if="quickSubmitOpen" :problem="problem" @close="quickSubmitOpen = false" />
  </article>
</template>
