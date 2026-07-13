<script setup lang="ts">
import { computed } from 'vue'
import { NScrollbar } from 'naive-ui'
import { CheckmarkCircle, CloseCircle, HelpCircle } from '@vicons/ionicons5'
import type { ContestProblem } from '@/types/contest'

const props = defineProps<{
  problems: ContestProblem[]
  currentId: string | null
}>()

const emit = defineEmits<{
  select: [problemId: string]
}>()

/// 题目状态图标（当前无真实 AC 数据，MVP 用占位）
function getStatusIcon(_p: ContestProblem) {
  // TODO: 对接 HOJ 用户题目状态后显示真实 AC/WA
  return null
}

function isActive(p: ContestProblem): boolean {
  return p.problemId === props.currentId || p.displayId === props.currentId
}
</script>

<template>
  <aside class="flex w-[220px] shrink-0 flex-col border-r border-[var(--border-color)] bg-[var(--bg-sidebar)]">
    <div class="px-4 py-3 text-xs font-semibold uppercase tracking-wider text-[var(--text-secondary)]">
      题目列表
      <span class="ml-1 text-[var(--color-primary)]">{{ problems.length }}</span>
    </div>
    <n-scrollbar class="flex-1">
      <div class="flex flex-col gap-0.5 px-2 pb-2">
        <button
          v-for="p in problems"
          :key="p.id"
          class="flex items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors hover:bg-[var(--bg-card)]"
          :class="isActive(p) ? 'bg-[var(--color-primary)]/10 text-[var(--color-primary)] font-medium' : 'text-[var(--text-primary)]'"
          @click="emit('select', p.problemId)"
        >
          <span class="flex h-6 w-6 items-center justify-center rounded text-xs font-mono font-bold"
            :class="isActive(p) ? 'bg-[var(--color-primary)] text-white' : 'bg-[var(--border-color)] text-[var(--text-secondary)]'"
          >
            {{ p.displayId }}
          </span>
          <span class="flex-1 truncate">{{ p.displayTitle || `Problem ${p.displayId}` }}</span>
        </button>
      </div>
    </n-scrollbar>
  </aside>
</template>
