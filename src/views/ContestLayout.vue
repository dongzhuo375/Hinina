<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import TopBar from '@/components/layout/TopBar.vue'
import StatusBar from '@/components/layout/StatusBar.vue'
import { useAnnouncementStore } from '@/stores/announcementStore'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { systemService } from '@/services/system.service'
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
/// 钥匙模式且正停在设置页：设置页两项豁免的**共同判据**（阶段闸门短路、最小壳）。
/// 赛程闸门只防「滞留在过期赛场看旧数据」——设置页只碰本地配置，不在其射程内；
/// 钥匙场景把维护者踢回登录页等于重新锁死（firstNavigation 已消费、无 UI 回入口）。
const keyedSettings = computed(
  () => systemService.launchFlags.value.settings && route.name === 'Settings',
)
/// 最小壳：keyedSettings 再加无会话。**刻意不含 `sessionResolved`**（评审四
/// P2-1）：钥匙 + 存量过期会话启动时，首个认证请求 401 → invalidateSession 把
/// sessionResolved 复位为 false（后端登出失败时 backendCleared=false）——此刻
/// 正是「钥匙 + 无会话 + 设置页」形态，最小壳必须接管，否则正常外壳包着无会话
/// 设置页继续发注定失败的请求。守卫在导航前已完成 checkSession，挂载时机不受
/// 影响；快照由守卫首航填充，同步判定无闪帧。
const bareShell = computed(() => keyedSettings.value && !auth.isLoggedIn)
/// 赛程闸门的**模板判定**：非钥匙设置页且赛程处于 ended/upcoming 时，结束遮罩、
/// 内容区 inert 与顶栏导航禁用一并生效。评审六：3e30f16 只短路了 watcher 的
/// 跳转，模板仍按裸 phase 判定——钥匙模式（有会话）的设置页被 inert 锁死、
/// 已结束还叠着「即将返回登录」遮罩，设置页实际不可用。统一收敛到此一处。
const contestGated = computed(
  () => !keyedSettings.value && (phase.value === 'ended' || phase.value === 'upcoming'),
)
let exitTimer: ReturnType<typeof setTimeout> | null = null
let disposed = false

watch([phase, keyedSettings], ([value, keyed]) => {
  if (exitTimer) clearTimeout(exitTimer)
  exitTimer = null
  // 钥匙模式下正停在设置页（无论有无会话）不参与赛程闸门：登出清理保留的
  // 匿名简报 / 已结束赛程都可能让 phase 非 none——跳回登录页会把钥匙场景
  // 重新锁死（评审四 P2-2 + 评审五补齐有会话分支）；离开设置页时本监听
  // 重评，赛程闸门照常恢复。
  if (keyed) return
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
    <!-- 顶栏按形态切换（最小壳的拖动区标题 App 全局标题栏在 matched 含 Contest 时
         不渲染，decorations=false 下窗口移动全靠它），内容区（router-view）**两形态
         共享同一实例**：最小壳↔完整外壳翻转（会话失效/登出）时不重挂设置视图，
         未保存草稿得以保留（评审六） -->
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
    </template>
    <TopBar v-else :navigation-disabled="contestGated" />
    <div class="flex min-h-0 flex-1 overflow-hidden" :inert="contestGated">
      <main class="workspace-main flex min-w-0 flex-1 flex-col overflow-hidden">
        <router-view />
      </main>
    </div>
    <StatusBar v-if="!bareShell" />
    <div v-if="contestGated && phase === 'ended'" class="absolute inset-x-0 bottom-0 top-10 z-50 flex items-center justify-center bg-[#f7f6f1]/85 backdrop-blur-sm">
      <div role="alert" class="max-w-sm border border-[var(--border-color)] bg-[var(--bg-body)] px-12 py-10 text-center shadow-[0_24px_64px_rgba(46,48,51,0.08)]">
        <p class="eyebrow">CONTEST CLOSED</p>
        <h1 class="mt-4 text-2xl font-semibold tracking-wide text-[var(--text-primary)]">比赛已结束</h1>
        <p class="mt-4 text-sm leading-6 text-[var(--text-secondary)]">感谢参赛，即将返回登录入口。</p>
      </div>
    </div>
  </div>
</template>
