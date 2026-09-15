<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import TopBar from '@/components/layout/TopBar.vue'
import ActivityBar from '@/components/layout/ActivityBar.vue'
import StatusBar from '@/components/layout/StatusBar.vue'
import { useAnnouncementStore } from '@/stores/announcementStore'
import { useContestStore } from '@/stores/contestStore'

const contestStore = useContestStore()
const announcementStore = useAnnouncementStore()

/// 会话有效性已由路由守卫（meta.requiresAuth）保证；
/// 外壳负责拉取一次比赛数据，TopBar / StatusBar 与各视图共享该状态
async function bootstrap(): Promise<void> {
  try {
    // whenLoaded 复用在途请求（P59 统一入口），子视图先触发加载时也走同一条链路
    await contestStore.whenLoaded()
  } catch {
    // 失败原因已由 store 写入 error，StatusBar 与各视图负责兜底展示
    return
  }
  const contestId = contestStore.contest?.id
  if (!contestId) return
  // 公告轮询由外壳统一持有，使 ActivityBar 未读红点在全部页面保持鲜活；
  // status === 1（比赛已结束）时暂停请求。刷新失败由 store.refresh 内部吞掉，不影响外壳
  announcementStore.startLive(contestId, () => contestStore.contest?.status === 1)
}

onMounted(() => {
  void bootstrap()
})

onUnmounted(() => {
  // 轮询器是模块级副作用句柄，离开工作台必须回收
  announcementStore.stopLive()
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
