<script setup lang="ts">
import { onMounted } from 'vue'
import TopBar from '@/components/layout/TopBar.vue'
import ActivityBar from '@/components/layout/ActivityBar.vue'
import StatusBar from '@/components/layout/StatusBar.vue'
import { useContestStore } from '@/stores/contestStore'

const contestStore = useContestStore()

/// 会话有效性已由路由守卫（meta.requiresAuth）保证；
/// 外壳负责拉取一次比赛数据，TopBar / StatusBar 与各视图共享该状态
onMounted(() => {
  if (!contestStore.contest && !contestStore.isLoading) {
    contestStore.loadContest().catch(() => {
      // 失败原因已由 store 写入 error，StatusBar 与各视图负责兜底展示
    })
  }
})
</script>

<template>
  <div class="flex h-full flex-col overflow-hidden bg-[var(--bg-body)]">
    <TopBar />
    <div class="flex min-h-0 flex-1 overflow-hidden">
      <ActivityBar />
      <main class="flex min-w-0 flex-1 flex-col overflow-hidden">
        <router-view />
      </main>
    </div>
    <StatusBar />
  </div>
</template>
