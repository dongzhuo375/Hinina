<script setup lang="ts">
import '@fontsource-variable/noto-serif-sc/wght.css'
import '@fontsource-variable/roboto-mono/wght.css'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import Countdown from '@/components/login/Countdown.vue'
import EventTitle from '@/components/login/EventTitle.vue'
import LoginForm from '@/components/login/LoginForm.vue'
import { briefRetryDelayMs } from '@/utils/brief-retry'
import { renderMarkdown } from '@/utils/markdown'
import { getAuthoritativeSchedule, getContestPhase } from '@/utils/contest'
import { planNextPrecheck, planRetryDelayMs } from '@/utils/session-check'
import { useNowMs } from '@/composables/useNowMs'
import type { PrecheckReason } from '@/utils/session-check'

const router = useRouter()
const auth = useAuthStore()
const contestStore = useContestStore()

const username = ref('')
const password = ref('')
const isSubmitting = ref(false)
/// 登出（切换账号）请求进行中标记
const isSigningOut = ref(false)
const briefVisible = ref(false)

/// 比赛简报（匿名接口）：状态由 contestStore 持有，登出/切换账号后依然保留
const brief = computed(() => contestStore.brief)
/// 服务器连接状态：idle / connecting / connected / failed / unconfigured
const connState = computed(() => contestStore.briefState)
/// 简报加载失败原因（展示在右下角连接状态下方，便于现场排障）
const briefError = computed(() => contestStore.briefError)
const briefRetryScheduled = ref(false)

/// 版本号构建期注入（vite define ← package.json），与 StatusBar 同源，避免写死后漂移
const version = __APP_VERSION__

/// 当前时间戳（共享毫秒墙钟派生秒，驱动倒计时）
const nowMs = useNowMs()
const now = computed(() => Math.floor(nowMs.value / 1000))
let disposed = false

/// 比赛阶段（判据集中在 utils/contest，与顶部栏共用）
const schedule = computed(() => getAuthoritativeSchedule(contestStore.contest, brief.value))
const contestPhase = computed(() => getContestPhase(schedule.value, now.value))

const eventTitle = computed(() => brief.value?.title || 'Hinina 竞赛')

// ── 会话状态与进入赛场决策 ──

/// 只有比赛进行中才自动进入工作台；已结束的比赛保持在登录入口。
const canEnter = computed(() => auth.isLoggedIn && contestPhase.value === 'running')

/// 比赛进行中后自动进入比赛页面（导航统一走命名路由）
function routeIntoContest() {
  void router.replace({ name: 'Contest' })
}

/// 会话建立后由本侦听器统一决策导航：
/// 比赛进行中 → 自动进入赛场；未开始 → 留在本页等待，已结束 → 不进入赛场。
watch(
  canEnter,
  (ready) => {
    if (ready) routeIntoContest()
  },
  { immediate: true },
)

/// 比赛简介 Markdown 渲染（相对图片 URL 改写为 OJ 绝对地址）
const renderedBrief = computed(() =>
  renderMarkdown(brief.value?.description ?? '', contestStore.briefBaseUrl),
)

/// 加载比赛简报；失败重试由连接状态侦听器统一调度。
function loadBrief() {
  void contestStore.loadBrief()
}

function retryBrief() {
  clearBriefRetry()
  briefRetryCount = 0
  loadBrief()
}

/// 登录表单提交。
/// 成功后不直接跳转：是否进入赛场由 `canEnter` 侦听器按比赛阶段决策，
/// 比赛未开始时留在登录页等待开赛。
async function handleLogin() {
  if (!username.value.trim() || !password.value || isSubmitting.value) return
  isSubmitting.value = true
  try {
    await auth.login(username.value.trim(), password.value)
    // 会话已建立，立即清除内存中的明文密码
    password.value = ''
  } catch {
    // 错误信息由 authStore 写入 auth.error，模板负责展示
  } finally {
    isSubmitting.value = false
  }
}

/// 切换账号：登出当前会话并回到登录表单。
/// `authStore.logout()` 不会 reject，因此无需额外捕获。
async function handleSwitchAccount() {
  if (isSigningOut.value) return
  isSigningOut.value = true
  try {
    await auth.logout()
    password.value = ''
  } finally {
    isSigningOut.value = false
  }
}

// ── 匿名比赛信息重试：最多三次，成功/未配置/卸载时停止 ──

let briefRetryCount = 0
let briefRetryTimer: ReturnType<typeof setTimeout> | null = null

function clearBriefRetry() {
  if (briefRetryTimer) {
    clearTimeout(briefRetryTimer)
    briefRetryTimer = null
  }
  briefRetryScheduled.value = false
}

watch(connState, (state) => {
  clearBriefRetry()

  if (state === 'connected' || state === 'unconfigured') {
    briefRetryCount = 0
    return
  }
  if (state !== 'failed') return

  const delay = briefRetryDelayMs(briefRetryCount)
  if (delay === null) return
  briefRetryCount += 1
  briefRetryScheduled.value = true
  briefRetryTimer = setTimeout(() => {
    briefRetryTimer = null
    briefRetryScheduled.value = false
    loadBrief()
  }, delay)
})

// ── 赛前会话预检（错峰调度，策略见 utils/session-check） ──

let precheckTimer: ReturnType<typeof setTimeout> | null = null
/// 本轮排程是否已因 unknown 重试过（只重试一次，避免重试风暴）
let precheckRetried = false

/// 重新排程：仅"已登录 + 比赛未开始"时预检，其余状态取消
function reschedulePrecheck() {
  clearPrecheck()
  precheckRetried = false

  const startSecs = schedule.value?.startTime
  if (!auth.isLoggedIn || contestPhase.value !== 'upcoming' || !startSecs) return
  const plan = planNextPrecheck(Date.now(), startSecs * 1000)
  if (!plan) return
  precheckTimer = setTimeout(() => {
    void runPrecheck(plan.reason)
  }, plan.delayMs)
}

/// 执行一次预检。
/// `invalid` 由 authStore 就地清理会话并退回登录表单；`unknown` 只重试一次。
async function runPrecheck(reason: PrecheckReason) {
  precheckTimer = null
  const validity = await auth.validateSession()
  if (disposed || !auth.isLoggedIn || contestPhase.value !== 'upcoming') return

  if (validity === 'unknown' && !precheckRetried) {
    precheckRetried = true
    precheckTimer = setTimeout(() => {
      void runPrecheck(reason)
    }, planRetryDelayMs())
    return
  }

  // 周期复检继续排程；窗口内/立即预检为一次性，之后交给进场流程与全局 401 兜底
  if (reason === 'periodic') reschedulePrecheck()
}

/// 取消待执行的预检
function clearPrecheck() {
  if (precheckTimer) {
    clearTimeout(precheckTimer)
    precheckTimer = null
  }
}

/// 登录态或比赛阶段变化时重新排程（登录成功、登出、简报到达、开赛）
watch([() => auth.isLoggedIn, contestPhase], reschedulePrecheck)

onMounted(async () => {
  loadBrief()
  // 恢复既有会话（路由守卫可能已确认过，跳过以避免重复 IPC）；
  // 恢复后的导航同样交由 canEnter 侦听器决策
  if (!auth.sessionResolved) await auth.checkSession()
  if (!disposed) reschedulePrecheck()
})

onUnmounted(() => {
  disposed = true
  clearBriefRetry()
  clearPrecheck()
})
</script>

<template>
  <div class="login-page relative isolate h-full overflow-y-auto bg-[#f7f6f1] text-[#2e3033]">
    <div class="login-geometry pointer-events-none absolute inset-0 overflow-hidden" aria-hidden="true">
      <span class="geometry-axis"></span>
      <span class="geometry-plane"></span>
    </div>

    <main class="login-main relative z-10 mx-auto grid w-full max-w-[1600px] gap-12">
      <div class="event-column flex min-w-0 flex-col">
        <EventTitle :title="eventTitle" />
        <Countdown
          class="event-clock"
          :phase="contestPhase"
          :start-time="schedule?.startTime ?? null"
          :end-time="schedule?.endTime ?? null"
          :now="now"
          :is-logged-in="auth.isLoggedIn"
        />
      </div>

      <div class="entry-column flex min-w-0 items-center justify-center">
        <LoginForm
          v-model:username="username"
          v-model:password="password"
          :is-submitting="isSubmitting"
          :is-signing-out="isSigningOut"
          :is-logged-in="auth.isLoggedIn"
          :account-name="auth.username"
          :auth-error="auth.error"
          :phase="contestPhase"
          :brief-html="renderedBrief"
          :brief-visible="briefVisible"
          @submit="handleLogin"
          @switch-account="handleSwitchAccount"
          @toggle-brief="briefVisible = !briefVisible"
        />
      </div>
    </main>

    <footer class="login-footer relative z-10 mx-auto flex w-full max-w-[1600px] flex-wrap items-end justify-end gap-4">
      <div class="min-w-0 text-right" role="status" aria-live="polite">
        <div class="flex flex-wrap items-center justify-end gap-x-4 gap-y-1 text-[13px] text-[#6b7280]">
          <span class="flex items-center gap-2">
            <span
              class="h-1 w-1 rounded-full"
              :class="connState === 'connected' ? 'bg-[#5f7a6a]' : connState === 'failed' ? 'bg-[#a33b2a]' : 'bg-[#8c6a43]'"
              aria-hidden="true"
            ></span>
            {{
              connState === 'connected' ? '已连接比赛服务器'
              : connState === 'connecting' ? '正在连接服务器…'
              : connState === 'failed' ? (briefRetryScheduled ? '连接失败，正在自动重试' : '服务器连接失败')
              : connState === 'unconfigured' ? '比赛服务器待配置'
              : '等待连接比赛服务器'
            }}
          </span>
          <span class="font-sans text-[9px] tracking-[1px]">Hinina v{{ version }}</span>
        </div>
        <p v-if="connState === 'failed' && briefError" class="mt-1 max-w-md break-words text-[13px] leading-5 text-[#a33b2a]" :title="briefError">
          {{ briefError }}
        </p>
        <button
          v-if="connState === 'failed'"
          type="button"
          class="mt-2 text-[13px] font-medium text-[#6b7280] underline decoration-[#9ca3af] underline-offset-4 transition-colors hover:text-[#8c6a43] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#8c6a43]"
          @click="retryBrief"
        >
          重试连接
        </button>
      </div>
    </footer>
  </div>
</template>

<style scoped>
.login-page {
  display: flex;
  flex-direction: column;
  --login-font-body: "Inter Variable", "Inter", "Noto Sans SC Variable", "Noto Sans SC", "Microsoft YaHei", sans-serif;
  --login-font-title: "Noto Serif SC Variable", "Noto Serif SC", "Source Han Serif SC", "STSong", "SimSun", serif;
  --login-font-mono: "Roboto Mono Variable", "Roboto Mono", "Space Mono", monospace;
  font-family: var(--login-font-body);
  font-synthesis: none;
}

/* 登录页拥有独立字体系统，避免工作台全局字体调整改变入口页的文化气质。 */
:global(.login-page .font-sans) { font-family: var(--login-font-body); }
:global(.login-page .font-mono) { font-family: var(--login-font-mono); }
:global(.login-page .login-title) {
  font-family: var(--login-font-title);
  font-weight: 600;
  font-synthesis: none;
}

/* 登录页文本选中态与木色交互强调保持一致，覆盖浏览器默认的紫色高亮。 */
:global(.login-page::selection),
:global(.login-page ::selection) {
  background: #8c6a43;
  color: #ffffff;
}

.login-main {
  flex: 1;
  grid-template-columns: minmax(0, 1.15fr) minmax(0, 0.85fr);
  padding: clamp(32px, 5vh, 64px) clamp(32px, 5.7vw, 96px) 40px;
  column-gap: clamp(36px, 5vw, 80px);
}

.event-clock {
  margin-top: clamp(110px, 18vh, 180px);
}

.entry-column {
  padding-top: 38px;
}

.login-footer {
  padding: 18px clamp(32px, 5.7vw, 96px) 22px;
}

.geometry-axis {
  position: absolute;
  top: 0;
  bottom: 0;
  left: 4.5%;
  border-left: 1px solid rgba(108, 122, 118, 0.12);
}

.geometry-plane {
  position: absolute;
  bottom: 0;
  left: 0;
  width: 55%;
  height: 22%;
  clip-path: polygon(0 0, 100% 100%, 48% 100%);
  background: linear-gradient(135deg, rgba(255, 255, 255, 0.95), rgba(255, 255, 255, 0.1));
}

@media (min-width: 801px) and (max-height: 760px) {
  .login-main { padding-top: 24px; padding-bottom: 24px; }
  .entry-column { padding-top: 20px; }
}

@media (min-width: 801px) and (min-height: 801px) {
  .event-clock { margin-top: max(clamp(110px, 18vh, 180px), calc(50vh - 256px)); }
}

@media (max-width: 800px) {
  .login-main {
    grid-template-columns: minmax(0, 1fr);
    padding: 32px 24px;
  }

  .event-clock { margin-top: 72px; }
  .entry-column { padding-top: 0; }
  .login-footer { padding: 18px 24px; }
}
</style>
