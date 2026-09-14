<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { NButton, NTag, NPopconfirm } from 'naive-ui'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { getContestPhase } from '@/utils/contest'

const router = useRouter()
const auth = useAuthStore()
const contest = useContestStore()

const now = ref(Math.floor(Date.now() / 1000))
/// 登出请求进行中标记，防止重复点击
const isLoggingOut = ref(false)

onMounted(() => {
  const timer = setInterval(() => {
    now.value = Math.floor(Date.now() / 1000)
  }, 1000)
  onUnmounted(() => clearInterval(timer))
})

/// 比赛阶段（判据集中在 utils/contest，与登录页共用）
const phase = computed(() => getContestPhase(contest.contest, now.value))

/// 比赛剩余时间文本
const timeStatus = computed(() => {
  const c = contest.contest
  if (!c) return ''
  switch (phase.value) {
    case 'upcoming':
      return `距开始 ${formatDuration(c.startTime - now.value)}`
    case 'running':
      return `剩余 ${formatDuration(c.endTime - now.value)}`
    default:
      return '已结束'
  }
})

const timeStatusType = computed<'info' | 'success' | 'warning'>(() => {
  switch (phase.value) {
    case 'upcoming':
      return 'warning'
    case 'running':
      return 'success'
    default:
      return 'info'
  }
})

function formatDuration(seconds: number): string {
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  const s = seconds % 60
  if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
  return `${m}:${String(s).padStart(2, '0')}`
}

/**
 * 登出并返回登录页。
 *
 * `authStore.logout()` 不会 reject（后端失败也会清理本地会话），
 * 因此 `finally` 中的跳转必然执行，不会把用户滞留在比赛页面。
 */
async function handleLogout() {
  if (isLoggingOut.value) return
  isLoggingOut.value = true
  try {
    await auth.logout()
  } finally {
    isLoggingOut.value = false
    await router.replace({ name: 'Login' })
  }
}
</script>

<template>
  <header class="flex h-14 items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-card)] px-4 shrink-0">
    <!-- 左侧：比赛标题 + 状态 -->
    <div class="flex items-center gap-3">
      <template v-if="contest.contest">
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
          <n-button text size="small" :loading="isLoggingOut" :disabled="isLoggingOut">
            {{ isLoggingOut ? '登出中…' : '登出' }}
          </n-button>
        </template>
        确定要登出吗？
      </n-popconfirm>
    </div>
  </header>
</template>
