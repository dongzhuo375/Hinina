<script setup lang="ts">
import { computed } from 'vue'
import { NButton, NTag, NPopconfirm } from 'naive-ui'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'

const auth = useAuthStore()
const contest = useContestStore()

/// 比赛剩余时间文本
const timeStatus = computed(() => {
  const c = contest.contest
  if (!c) return ''
  const now = Math.floor(Date.now() / 1000)
  if (c.status === 1) return '已结束'
  if (now < c.startTime) {
    const diff = c.startTime - now
    return `距开始 ${formatDuration(diff)}`
  }
  if (now < c.endTime) {
    const diff = c.endTime - now
    return `剩余 ${formatDuration(diff)}`
  }
  return '已结束'
})

const timeStatusType = computed<'info' | 'success' | 'warning'>(() => {
  const c = contest.contest
  if (!c) return 'info'
  if (c.status === 1) return 'info'
  const now = Math.floor(Date.now() / 1000)
  if (now < c.startTime) return 'warning'
  if (now < c.endTime) return 'success'
  return 'info'
})

function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
  return `${m}:${String(s).padStart(2, '0')}`
}

function handleLogout() {
  auth.logout()
}
</script>

<template>
  <header class="flex h-14 items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-card)] px-4 shrink-0">
    <!-- 左侧：Logo + 比赛标题 -->
    <div class="flex items-center gap-3">
      <span class="text-lg font-bold tracking-tight text-[var(--color-primary)]">Hinina</span>
      <template v-if="contest.contest">
        <span class="text-sm text-[var(--text-secondary)]">/</span>
        <span class="text-sm font-medium truncate max-w-[300px]">{{ contest.contest.title }}</span>
        <n-tag :type="timeStatusType" size="small" :bordered="false">
          {{ timeStatus }}
        </n-tag>
      </template>
    </div>

    <!-- 右侧：用户 + 登出 -->
    <div class="flex items-center gap-3">
      <span v-if="auth.user" class="text-sm text-[var(--text-secondary)]">{{ auth.user.username }}</span>
      <n-popconfirm @positive-click="handleLogout">
        <template #trigger>
          <n-button text size="small">登出</n-button>
        </template>
        确定要登出吗？
      </n-popconfirm>
    </div>
  </header>
</template>
