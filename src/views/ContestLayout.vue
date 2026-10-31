<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import TopBar from '@/components/layout/TopBar.vue'
import StatusBar from '@/components/layout/StatusBar.vue'
import { useAnnouncementStore } from '@/stores/announcementStore'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { getAuthoritativeSchedule, getContestPhase } from '@/utils/contest'
import { useNowMs } from '@/composables/useNowMs'

const router = useRouter()
const route = useRoute()
const auth = useAuthStore()
const contestStore = useContestStore()
const announcementStore = useAnnouncementStore()
const nowMs = useNowMs()
const now = computed(() => Math.floor(nowMs.value / 1000))
const phase = computed(() => getContestPhase(
  getAuthoritativeSchedule(contestStore.contest, contestStore.brief),
  now.value,
))
/// 最小壳：仅「无会话 + 当前路由是设置页」时启用 —— 这条组合只有 `--settings`
///（管理方钥匙）直落才会出现（守卫豁免设置路由；无会话时没有任何 UI 路径可达）。
/// **必须同时看路由**：正常登出/会话失效只把 user 置空（sessionResolved 仍为
/// true），若只判会话，比赛页外壳会在 replace(Login) 的异步守卫完成前闪成最小
/// 壳、当前子视图重挂并发出注定失败的认证请求（评审抓出）。
const bareShell = computed(
  () => auth.sessionResolved && !auth.isLoggedIn && route.name === 'Settings',
)
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
/// 外壳负责拉取一次比赛数据，TopBar / StatusBar 与各视图共享该状态。
/// 最小壳（--settings 直落设置页）时跳过：不发起任何面向 OJ 的请求。
async function bootstrap(): Promise<void> {
  if (bareShell.value) return
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
  void bootstrap()
})

onUnmounted(() => {
  disposed = true
  if (exitTimer) clearTimeout(exitTimer)
  // 轮询器是模块级副作用句柄，离开工作台必须回收
  announcementStore.stopLive()
})

function backToLogin(): void {
  void router.replace({ name: 'Login' })
}
</script>

<template>
  <div class="contest-theme relative flex h-full flex-col overflow-hidden bg-[var(--bg-body)]">
    <!-- 最小壳：仅「无会话 + 设置路由」（--settings 直落）时渲染。自带拖动区 ——
         App 的全局标题栏在 matched 含 Contest 时不渲染，decorations=false 下窗口
         移动全靠它 -->
    <template v-if="bareShell">
      <header
        data-tauri-drag-region
        class="flex h-10 shrink-0 select-none items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-card)] pl-4 pr-3"
      >
        <span data-tauri-drag-region class="text-sm font-semibold text-[var(--text-primary)]">
          Hinina 设置
        </span>
        <button
          type="button"
          class="rounded-sm border border-[var(--border-color)] bg-[var(--bg-card)] px-3 py-1 text-xs text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
          @click="backToLogin"
        >
          返回登录
        </button>
      </header>
      <main class="flex min-h-0 flex-1 flex-col overflow-hidden">
        <router-view />
      </main>
    </template>
    <template v-else>
      <TopBar :navigation-disabled="phase === 'ended' || phase === 'upcoming'" />
      <div class="flex min-h-0 flex-1 overflow-hidden" :inert="phase === 'ended' || phase === 'upcoming'">
        <main class="workspace-main flex min-w-0 flex-1 flex-col overflow-hidden">
          <router-view />
        </main>
      </div>
      <StatusBar />
      <div v-if="phase === 'ended'" class="absolute inset-x-0 bottom-0 top-10 z-50 flex items-center justify-center bg-[#f7f6f1]/85 backdrop-blur-sm">
        <div role="alert" class="max-w-sm border border-[var(--border-color)] bg-[var(--bg-body)] px-12 py-10 text-center shadow-[0_24px_64px_rgba(46,48,51,0.08)]">
          <p class="eyebrow">CONTEST CLOSED</p>
          <h1 class="mt-4 text-2xl font-semibold tracking-wide text-[var(--text-primary)]">比赛已结束</h1>
          <p class="mt-4 text-sm leading-6 text-[var(--text-secondary)]">感谢参赛，即将返回登录入口。</p>
        </div>
      </div>
    </template>
  </div>
</template>
