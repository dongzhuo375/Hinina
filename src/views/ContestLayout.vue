<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import TopBar from '@/components/layout/TopBar.vue'
import StatusBar from '@/components/layout/StatusBar.vue'
import { useAnnouncementStore } from '@/stores/announcementStore'
import { useContestStore } from '@/stores/contestStore'
import { getAuthoritativeSchedule, getContestPhase } from '@/utils/contest'

const router = useRouter()
const contestStore = useContestStore()
const announcementStore = useAnnouncementStore()
const now = ref(Math.floor(Date.now() / 1000))
const phase = computed(() => getContestPhase(
  getAuthoritativeSchedule(contestStore.contest, contestStore.brief),
  now.value,
))
let clockTimer: ReturnType<typeof setInterval> | null = null
let exitTimer: ReturnType<typeof setTimeout> | null = null
let disposed = false

watch(phase, (value) => {
  if (exitTimer) clearTimeout(exitTimer)
  exitTimer = null
  if (value === 'upcoming') {
    void router.replace({ name: 'Login' })
  } else if (value === 'ended') {
    announcementStore.stopLive()
    // 保留会话与工作区，让现有离页保存链路处理代码落盘。
    exitTimer = setTimeout(() => {
      void router.replace({ name: 'Login' })
    }, 2_000)
  }
}, { immediate: true })

/// 会话有效性已由路由守卫（meta.requiresAuth）保证；
/// 外壳负责拉取一次比赛数据，TopBar / StatusBar 与各视图共享该状态
async function bootstrap(): Promise<void> {
  try {
    // whenLoaded 复用在途请求（P59 统一入口），子视图先触发加载时也走同一条链路
    await contestStore.whenLoaded()
    if (disposed) return

    // 登录页简报可能已经切换到下一场比赛，而 contest 仍保留上一场的缓存。
    // 此时 whenLoaded 会因 contest 非空直接返回，必须主动刷新认证数据，
    // 否则题目、榜单、提交和公告都会继续绑定旧比赛。
    if (
      contestStore.contest?.id &&
      contestStore.brief?.id &&
      contestStore.contest.id !== contestStore.brief.id
    ) {
      await contestStore.loadContest()
    }
  } catch {
    // 失败原因已由 store 写入 error，StatusBar 与各视图负责兜底展示
    return
  }
  const contestId = contestStore.contest?.id
  if (disposed || !contestId || phase.value !== 'running') return
  // 公告轮询由外壳统一持有，使 ActivityBar 未读红点在全部页面保持鲜活；
  // status === 1（比赛已结束）时暂停请求。刷新失败由 store.refresh 内部吞掉，不影响外壳
  announcementStore.startLive(contestId, () => contestStore.contest?.status === 1)
}

onMounted(() => {
  clockTimer = setInterval(() => {
    now.value = Math.floor(Date.now() / 1000)
  }, 1_000)
  void bootstrap()
})

onUnmounted(() => {
  disposed = true
  if (clockTimer) clearInterval(clockTimer)
  if (exitTimer) clearTimeout(exitTimer)
  // 轮询器是模块级副作用句柄，离开工作台必须回收
  announcementStore.stopLive()
})
</script>

<template>
  <div class="contest-theme relative flex h-full flex-col overflow-hidden bg-[var(--bg-body)]">
    <TopBar />
    <div class="flex min-h-0 flex-1 overflow-hidden" :inert="phase === 'ended' || phase === 'upcoming'">
      <main class="workspace-main flex min-w-0 flex-1 flex-col overflow-hidden">
        <router-view />
      </main>
    </div>
    <StatusBar />
    <div v-if="phase === 'ended'" class="absolute inset-x-0 bottom-0 top-12 z-50 flex items-center justify-center bg-[#f7f6f1]/85 backdrop-blur-sm">
      <div role="alert" class="max-w-sm border border-[#b9bfbc] bg-[#f7f6f1] px-12 py-10 text-center shadow-[0_24px_64px_rgba(46,48,51,0.08)]">
        <p class="font-mono text-[10px] tracking-[0.25em] text-[#8c6a43]">CONTEST CLOSED</p>
        <h1 class="mt-4 text-2xl font-semibold tracking-wide text-[#2e3033]">比赛已结束</h1>
        <p class="mt-4 text-sm leading-6 text-[#68706f]">感谢参赛，即将返回登录入口。</p>
      </div>
    </div>
  </div>
</template>
