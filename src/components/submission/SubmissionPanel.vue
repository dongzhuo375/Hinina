<script setup lang="ts">
import { NScrollbar, NTag, NEmpty } from 'naive-ui'
import type { JudgementStatus } from '@/types/submission'

interface SubmissionEntry {
  id: string
  problemId: string
  status: JudgementStatus
  time?: number
  memory?: number
}

defineProps<{
  submissions: SubmissionEntry[]
}>()

/// 状态 → Naive UI Tag 类型 + 中文
const statusConfig: Record<string, { type: 'default' | 'info' | 'success' | 'warning' | 'error'; label: string }> = {
  Pending: { type: 'info', label: '等待中' },
  Compiling: { type: 'info', label: '编译中' },
  Running: { type: 'info', label: '运行中' },
  Accepted: { type: 'success', label: 'AC' },
  WrongAnswer: { type: 'error', label: 'WA' },
  TimeLimitExceeded: { type: 'warning', label: 'TLE' },
  MemoryLimitExceeded: { type: 'warning', label: 'MLE' },
  RuntimeError: { type: 'error', label: 'RE' },
  CompilationError: { type: 'warning', label: 'CE' },
  Unknown: { type: 'default', label: '?' },
}
</script>

<template>
  <div class="flex flex-col h-full border-t border-[var(--border-color)] bg-[var(--bg-card)]">
    <div class="px-4 py-2 text-xs font-semibold uppercase tracking-wider text-[var(--text-secondary)] border-b border-[var(--border-color)]">
      提交记录
    </div>

    <n-scrollbar v-if="submissions.length > 0" class="flex-1">
      <div class="divide-y divide-[var(--border-color)]">
        <div
          v-for="sub in submissions"
          :key="sub.id"
          class="flex items-center gap-3 px-4 py-2.5 text-sm"
        >
          <n-tag
            :type="statusConfig[sub.status]?.type ?? 'default'"
            size="small"
            :bordered="false"
          >
            {{ statusConfig[sub.status]?.label ?? sub.status }}
          </n-tag>
          <span class="text-xs text-[var(--text-secondary)] font-mono">{{ sub.id }}</span>
          <span v-if="sub.time !== undefined" class="text-xs text-[var(--text-secondary)]">
            {{ sub.time }}ms / {{ ((sub.memory ?? 0) / 1024).toFixed(1) }}MB
          </span>
        </div>
      </div>
    </n-scrollbar>

    <div v-else class="flex-1 flex items-center justify-center">
      <n-empty description="暂无提交记录" size="small" />
    </div>
  </div>
</template>
