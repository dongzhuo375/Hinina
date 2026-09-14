<script setup lang="ts">
import { computed } from 'vue'
import { useContestStore } from '@/stores/contestStore'

const contestStore = useContestStore()

/// 连接状态判据：比赛数据已加载且无错误（复用 store 状态，不额外发探测请求）
const connected = computed(() => contestStore.contest !== null && !contestStore.error)
</script>

<template>
  <footer
    class="flex shrink-0 select-none items-center justify-between border-t border-[var(--border-color)] bg-[var(--bg-card)] px-6 py-2"
  >
    <div class="flex items-center gap-1.5 text-xs">
      <span
        class="h-2 w-2 rounded-full"
        :class="connected ? 'animate-pulse bg-[#22c55e]' : 'bg-[var(--color-error)]'"
      ></span>
      <span
        class="font-medium"
        :class="connected ? 'text-[var(--text-secondary)]' : 'text-[var(--color-error)]'"
      >
        {{ connected ? '已连接比赛服务器' : '连接异常' }}
      </span>
    </div>
    <span class="font-mono text-[10px] text-[var(--text-muted)]">Hinina v0.1.0</span>
  </footer>
</template>
