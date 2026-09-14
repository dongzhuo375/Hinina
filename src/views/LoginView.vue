<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { renderMarkdown } from '@/utils/markdown'
import { getContestPhase, hasContestStarted } from '@/utils/contest'
import { planNextPrecheck, planRetryDelayMs } from '@/utils/session-check'
import type { PrecheckReason } from '@/utils/session-check'

const router = useRouter()
const auth = useAuthStore()
const contestStore = useContestStore()

const username = ref('')
const password = ref('')
const isSubmitting = ref(false)
/// 登出（切换账号）请求进行中标记
const isSigningOut = ref(false)
const showPassword = ref(false)
const briefVisible = ref(false)

/// 比赛简报（匿名接口）：状态由 contestStore 持有，登出/切换账号后依然保留
const brief = computed(() => contestStore.brief)
/// 服务器连接状态：idle / connecting / connected / failed / unconfigured
const connState = computed(() => contestStore.briefState)
/// 简报加载失败原因（展示在左下角连接状态下方，便于现场排障）
const briefError = computed(() => contestStore.briefError)

/// 当前时间戳（每秒刷新，驱动倒计时）
const now = ref(Math.floor(Date.now() / 1000))
let timer: ReturnType<typeof setInterval> | null = null

/// 倒计时各段（天/时/分/秒）
const countdown = computed(() => {
  const c = brief.value
  if (!c) return { days: 0, hours: 0, mins: 0, secs: 0 }
  const diff = Math.max(0, c.startTime - now.value)
  return {
    days: Math.floor(diff / 86400),
    hours: Math.floor((diff % 86400) / 3600),
    mins: Math.floor((diff % 3600) / 60),
    secs: diff % 60,
  }
})

/// 比赛阶段（判据集中在 utils/contest，与顶部栏共用）
const contestPhase = computed(() => getContestPhase(brief.value, now.value))

/// 倒计时数字段配色：非零紫色，归零黑色
function segColor(v: number): string {
  return v > 0 ? 'text-[#6845f5]' : 'text-[var(--text-primary)]'
}

/// 倒计时四段（天/时/分/秒），供模板 v-for 渲染
const countdownSegs = computed(() => [
  { value: countdown.value.days, label: 'DAYS 天' },
  { value: countdown.value.hours, label: 'HOURS 时' },
  { value: countdown.value.mins, label: 'MINS 分' },
  { value: countdown.value.secs, label: 'SECS 秒' },
])

/// 比赛阶段徽章文案
const phaseLabel = computed(() => {
  switch (contestPhase.value) {
    case 'upcoming':
      return 'ACM / ICPC 赛制 · 即将开始'
    case 'running':
      return '比赛进行中'
    case 'ended':
      return '比赛已结束'
    default:
      return 'ACM / ICPC 赛制'
  }
})

/// 比赛阶段副标题文案（已登录时给出会话侧的引导语）
const phaseDesc = computed(() => {
  switch (contestPhase.value) {
    case 'upcoming':
      return auth.isLoggedIn
        ? '账号已就绪，倒计时归零后自动进入赛场'
        : '竞赛倒计时结束后自动解锁题目并开启实时评测系统'
    case 'running':
      return auth.isLoggedIn ? '比赛进行中，正在进入赛场…' : '比赛已开始，登录后即可进入赛场'
    case 'ended':
      return '比赛已结束'
    default:
      return 'Hinina 竞赛客户端'
  }
})

// ── 会话状态与进入赛场决策 ──

/// 是否允许进入赛场：已登录且比赛已开始（进行中/已结束）。
/// 比赛未开始时恒为 false —— 登录成功后留在本页等待倒计时归零。
const canEnter = computed(() => auth.isLoggedIn && hasContestStarted(contestPhase.value))

/// 比赛信息是否仍在加载（用于区分 `none` 阶段的"加载中"与"加载失败"）
const isBriefLoading = computed(() => connState.value === 'idle' || connState.value === 'connecting')

/// 已登录状态主标题
const sessionTitle = computed(() => {
  switch (contestPhase.value) {
    case 'upcoming':
      return '已就绪，等待开赛'
    case 'running':
      return '比赛进行中'
    case 'ended':
      return '比赛已结束'
    default:
      return isBriefLoading.value ? '正在同步赛程…' : '登录成功'
  }
})

/// 已登录状态说明文案
const sessionDesc = computed(() => {
  switch (contestPhase.value) {
    case 'upcoming':
      return '参赛账号已通过验证，比赛开始后客户端将自动进入赛场，请保持窗口开启'
    case 'running':
      return '正在进入赛场…'
    case 'ended':
      return '本场比赛已结束，可进入赛场查看题目与提交记录'
    default:
      if (isBriefLoading.value) return '正在获取比赛信息，随后将按赛程自动进入赛场'
      return connState.value === 'unconfigured'
        ? '客户端尚未配置比赛 ID，请完成配置后重新加载'
        : '未能获取比赛信息，请重新加载后确认赛程，或直接进入赛场'
  }
})

/// 左侧等待卡片的紧凑倒计时文本（超过一天时附带天数）
const countdownText = computed(() => {
  const { days, hours, mins, secs } = countdown.value
  const pad = (n: number) => String(n).padStart(2, '0')
  const hms = `${pad(hours)}:${pad(mins)}:${pad(secs)}`
  return days > 0 ? `${days} 天 ${hms}` : hms
})

/// 进入比赛页面（导航统一走命名路由，避免路径硬编码）
function enterContest() {
  void router.replace({ name: 'Contest' })
}

/// 会话建立后由本侦听器统一决策导航：
/// 比赛已开始 → 自动进入赛场；未开始 → 停留在登录页，倒计时归零时本侦听器再次触发。
watch(
  canEnter,
  (ready) => {
    if (ready) enterContest()
  },
  { immediate: true },
)

/// 比赛简介 Markdown 渲染（相对图片 URL 改写为 OJ 绝对地址）
const renderedBrief = computed(() =>
  renderMarkdown(brief.value?.description ?? '', contestStore.briefBaseUrl),
)

/// 重新加载比赛简报（连接失败/未配置时的重试入口）
function reloadBrief() {
  void contestStore.loadBrief()
}

/// 登录表单提交。
/// 成功后不直接跳转：是否进入赛场由 `canEnter` 侦听器按比赛阶段决策，
/// 比赛未开始时留在登录页等待开赛。
async function handleLogin() {
  if (!username.value || !password.value || isSubmitting.value) return
  isSubmitting.value = true
  try {
    await auth.login(username.value, password.value)
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

// ── 赛前会话预检（错峰调度，策略见 utils/session-check） ──

let precheckTimer: ReturnType<typeof setTimeout> | null = null
/// 本轮排程是否已因 unknown 重试过（只重试一次，避免重试风暴）
let precheckRetried = false
/// 最近一次预检结果（展示用，让选手知道客户端在持续校验会话）
const lastPrecheckText = ref('')

/// 重新排程：仅"已登录 + 比赛未开始"时预检，其余状态取消并清空提示
function reschedulePrecheck() {
  clearPrecheck()
  precheckRetried = false

  const startSecs = brief.value?.startTime
  if (!auth.isLoggedIn || contestPhase.value !== 'upcoming' || !startSecs) {
    lastPrecheckText.value = ''
    return
  }
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

  if (validity === 'valid') {
    const at = new Date().toLocaleTimeString('zh-CN', { hour12: false })
    lastPrecheckText.value = `会话校验通过 · ${at}`
  } else if (validity === 'unknown') {
    lastPrecheckText.value = '会话校验未完成（网络异常）'
    if (!precheckRetried) {
      precheckRetried = true
      precheckTimer = setTimeout(() => {
        void runPrecheck(reason)
      }, planRetryDelayMs())
      return
    }
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
  reloadBrief()
  timer = setInterval(() => {
    now.value = Math.floor(Date.now() / 1000)
  }, 1000)
  // 恢复既有会话（路由守卫可能已确认过，跳过以避免重复 IPC）；
  // 恢复后的导航同样交由 canEnter 侦听器决策
  if (!auth.sessionResolved) await auth.checkSession()
  reschedulePrecheck()
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
  clearPrecheck()
})
</script>

<template>
  <div class="flex h-full w-full select-none items-center justify-center bg-[var(--bg-body)] p-4 sm:p-6">
    <!-- 主窗口卡片 -->
    <div class="relative flex h-full w-full overflow-hidden rounded-2xl border border-[var(--border-color)] bg-[var(--bg-card)] shadow-xl">
      <!-- ═══════ 左侧：登录 / 会话状态栏（400px） ═══════ -->
      <section class="flex w-[400px] shrink-0 flex-col border-r border-[var(--border-color)] px-10 py-8">
        <!-- Logo -->
        <div class="mb-14 flex items-center gap-3">
          <div class="flex h-9 w-9 items-center justify-center rounded-lg bg-gradient-to-tr from-[#5631e0] to-[#7c5cff] text-white shadow-sm shadow-[#7c5cff]/30">
            <svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
              <path d="M12 2C9.5 5 7 7.5 7 11.5C7 14.54 9.46 17 12.5 17C10.5 15.5 10 13.5 10.5 11.8C11.5 14 13.5 14.5 14.5 13.2C15.8 11.5 15.5 9 14 7C16.8 8.8 18 11.4 18 14C18 17.87 14.87 21 11 21C6.03 21 2 16.97 2 12C2 7.8 5.5 4 8.5 2.5C9.5 2 10.8 1.5 12 2Z"></path>
            </svg>
          </div>
          <span class="text-lg font-bold tracking-tight text-[var(--text-primary)]">Hinina</span>
        </div>

        <!-- ══ 已登录：会话状态 + 等待开赛 / 进入赛场 ══ -->
        <div v-if="auth.isLoggedIn" class="flex-1">
          <div class="mb-3 inline-flex items-center gap-1.5 rounded-full border border-[#dcfce7] bg-[#f0fdf4] px-2.5 py-1 text-[11px] font-medium text-[#16a34a]">
            <span class="h-1.5 w-1.5 rounded-full bg-[var(--color-success)]"></span>
            会话已建立 · Hinina OJ
          </div>
          <h1 class="text-2xl font-bold tracking-tight text-[var(--text-primary)]">{{ sessionTitle }}</h1>
          <p class="mt-2.5 text-xs leading-relaxed text-[var(--text-secondary)]">{{ sessionDesc }}</p>

          <!-- 账号信息卡 -->
          <div class="mt-8 rounded-xl border border-[var(--border-color)] bg-[var(--bg-body)]/60 p-4">
            <div class="flex items-center justify-between">
              <span class="text-xs font-semibold tracking-wide text-[var(--text-secondary)]">参赛账号 / Seat UID</span>
              <span class="rounded bg-[#f5f3ff] px-1.5 py-0.5 font-mono text-[10px] text-[#6845f5]">已验证</span>
            </div>
            <p class="mt-2 truncate font-mono text-sm font-medium text-[var(--text-primary)]">{{ auth.username }}</p>

            <!-- 等待开赛时展示紧凑倒计时与最近一次会话预检结果 -->
            <div v-if="contestPhase === 'upcoming'" class="mt-4 border-t border-[var(--border-color)] pt-3">
              <div class="flex items-center justify-between">
                <span class="text-[11px] font-medium tracking-wide text-[var(--text-muted)]">距离开赛</span>
                <span class="font-mono text-sm font-semibold tabular-nums text-[#6845f5]">{{ countdownText }}</span>
              </div>
              <p v-if="lastPrecheckText" class="mt-2 text-right text-[10px] text-[var(--text-muted)]">
                {{ lastPrecheckText }}
              </p>
            </div>
          </div>

          <!-- 操作区 -->
          <div class="mt-6 space-y-3">
            <!-- 比赛已开始：自动跳转，同时保留手动入口 -->
            <button
              v-if="canEnter"
              type="button"
              class="group flex h-12 w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-[#6845f5] to-[#7c5cff] text-sm font-semibold text-white shadow-[0_0_25px_-5px_rgba(124,92,255,0.4)] transition-all hover:from-[#5631e0] hover:to-[#6845f5] active:scale-[0.99]"
              @click="enterContest"
            >
              <span class="tracking-wide">进入赛场</span>
              <svg class="h-4 w-4 stroke-current stroke-2 transition-transform group-hover:translate-x-1" fill="none" viewBox="0 0 24 24">
                <path d="M5 12h14M12 5l7 7-7 7"></path>
              </svg>
            </button>

            <!-- 比赛信息缺失：加载中占位 / 失败后重试 + 手动兜底入口 -->
            <template v-else-if="contestPhase === 'none'">
              <p v-if="isBriefLoading" class="flex items-center justify-center gap-2 rounded-xl border border-dashed border-[var(--border-color)] py-3.5 text-xs text-[var(--text-secondary)]">
                <span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-[#7c5cff]/30 border-t-[#7c5cff]"></span>
                正在同步比赛信息…
              </p>
              <template v-else>
                <button
                  type="button"
                  class="flex h-12 w-full items-center justify-center rounded-xl border border-[var(--border-color)] text-sm font-medium text-[var(--text-secondary)] transition-colors hover:border-[#7c5cff] hover:text-[#6845f5]"
                  @click="reloadBrief"
                >
                  重新加载比赛信息
                </button>
                <button
                  type="button"
                  class="w-full text-center text-xs font-medium text-[var(--text-muted)] transition-colors hover:text-[#6845f5]"
                  @click="enterContest"
                >
                  直接进入赛场
                </button>
              </template>
            </template>

            <!-- 比赛未开始：留在登录页等待开赛 -->
            <p v-else class="flex items-center justify-center gap-2 rounded-xl border border-dashed border-[var(--border-color)] py-3.5 text-xs text-[var(--text-secondary)]">
              <span class="h-1.5 w-1.5 animate-pulse rounded-full bg-[#7c5cff]"></span>
              等待比赛开始，开赛后自动进入赛场
            </p>

            <!-- 切换账号 -->
            <button
              type="button"
              class="flex h-10 w-full items-center justify-center text-xs font-medium text-[var(--text-muted)] transition-colors hover:text-[var(--color-error)] disabled:cursor-not-allowed disabled:opacity-50"
              :disabled="isSigningOut"
              @click="handleSwitchAccount"
            >
              {{ isSigningOut ? '正在登出…' : '切换账号 / 登出' }}
            </button>
          </div>

          <!-- 登出异常提示 -->
          <p v-if="auth.error" class="mt-4 text-xs text-[var(--color-error)]">{{ auth.error }}</p>
        </div>

        <!-- ══ 未登录：登录表单 ══ -->
        <div v-else class="flex-1">
          <div class="mb-3 inline-flex items-center gap-1.5 rounded-full border border-[#ede9fe] bg-[#f5f3ff] px-2.5 py-1 text-[11px] font-medium text-[#6845f5]">
            <span class="h-1.5 w-1.5 animate-pulse rounded-full bg-[#7c5cff]"></span>
            比赛验证通道 · Hinina OJ
          </div>
          <h1 class="text-2xl font-bold tracking-tight text-[var(--text-primary)]">欢迎参赛</h1>
          <p class="mt-2.5 text-xs leading-relaxed text-[var(--text-secondary)]">请使用比赛专用账号登录，赛前将会分发账号密码</p>

          <form class="mt-8 space-y-5" @submit.prevent="handleLogin">
            <!-- 账号 -->
            <div>
              <label for="contest-username" class="mb-2 block text-xs font-semibold tracking-wide text-[var(--text-secondary)]">参赛账号 / Seat UID</label>
              <div class="flex items-center rounded-xl border border-[var(--border-color)] bg-[var(--bg-body)]/60 transition-all focus-within:border-[#7c5cff] focus-within:ring-[3px] focus-within:ring-[#7c5cff]/10">
                <span class="flex items-center pl-3.5 text-[var(--text-muted)]">
                  <svg class="h-4 w-4 stroke-current stroke-2" fill="none" viewBox="0 0 24 24">
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"></path>
                    <circle cx="12" cy="7" r="4"></circle>
                  </svg>
                </span>
                <input
                  id="contest-username"
                  v-model="username"
                  type="text"
                  placeholder="请输入参赛机位账号"
                  autocomplete="username"
                  class="w-full bg-transparent py-2.5 pl-3 pr-3.5 font-mono text-sm font-medium text-[var(--text-primary)] placeholder-[var(--text-muted)] focus:outline-none"
                  :disabled="isSubmitting"
                />
              </div>
            </div>

            <!-- 密码 -->
            <div>
              <label for="contest-password" class="mb-2 block text-xs font-semibold tracking-wide text-[var(--text-secondary)]">参赛密码 / Contest PIN</label>
              <div class="flex items-center rounded-xl border border-[var(--border-color)] bg-[var(--bg-body)]/60 transition-all focus-within:border-[#7c5cff] focus-within:ring-[3px] focus-within:ring-[#7c5cff]/10">
                <span class="flex items-center pl-3.5 text-[var(--text-muted)]">
                  <svg class="h-4 w-4 stroke-current stroke-2" fill="none" viewBox="0 0 24 24">
                    <rect height="11" rx="2" width="18" x="3" y="11"></rect>
                    <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                  </svg>
                </span>
                <input
                  id="contest-password"
                  v-model="password"
                  :type="showPassword ? 'text' : 'password'"
                  placeholder="输入比赛认证密码"
                  autocomplete="current-password"
                  class="w-full bg-transparent py-2.5 pl-3 pr-3.5 font-mono text-sm tracking-widest text-[var(--text-primary)] placeholder-[var(--text-muted)] focus:outline-none"
                  :disabled="isSubmitting"
                  @keyup.enter="handleLogin"
                />
                <button type="button" aria-label="显示/隐藏密码" class="flex items-center pr-3.5 text-[var(--text-muted)] transition-colors hover:text-[var(--text-secondary)]" @click="showPassword = !showPassword">
                  <svg v-if="!showPassword" class="h-4 w-4 stroke-current stroke-2" fill="none" viewBox="0 0 24 24">
                    <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z"></path>
                    <circle cx="12" cy="12" r="3"></circle>
                  </svg>
                  <svg v-else class="h-4 w-4 stroke-current stroke-2" fill="none" viewBox="0 0 24 24">
                    <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24"></path>
                    <line x1="1" x2="23" y1="1" y2="23"></line>
                  </svg>
                </button>
              </div>
            </div>

            <!-- 错误提示 -->
            <p v-if="auth.error" class="text-sm text-[var(--color-error)]">{{ auth.error }}</p>

            <!-- 登录按钮 -->
            <div class="pt-2">
              <button
                type="submit"
                class="group flex h-12 w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-[#6845f5] to-[#7c5cff] text-sm font-semibold text-white shadow-[0_0_25px_-5px_rgba(124,92,255,0.4)] transition-all hover:from-[#5631e0] hover:to-[#6845f5] active:scale-[0.99] disabled:cursor-not-allowed disabled:opacity-50"
                :disabled="isSubmitting || !username || !password"
              >
                <span v-if="isSubmitting" class="h-4 w-4 animate-spin rounded-full border-2 border-white/40 border-t-white"></span>
                <span v-else class="tracking-wide">登录</span>
                <svg v-if="!isSubmitting" class="h-4 w-4 stroke-current stroke-2 transition-transform group-hover:translate-x-1" fill="none" viewBox="0 0 24 24">
                  <path d="M5 12h14M12 5l7 7-7 7"></path>
                </svg>
              </button>
            </div>
          </form>
        </div>

        <!-- 底部连接状态 -->
        <div class="flex flex-col gap-2 border-t border-[var(--border-color)] pt-4">
          <div class="flex items-center justify-between text-[11px] text-[var(--text-secondary)]">
            <div class="flex items-center gap-1.5">
              <span
                class="h-2 w-2 rounded-full"
                :class="{
                  'bg-[#22c55e]': connState === 'connected',
                  'bg-[#f59e0b] animate-pulse': connState === 'connecting',
                  'bg-[#ef4444]': connState === 'failed',
                  'bg-[var(--text-muted)]': connState === 'unconfigured' || connState === 'idle',
                }"
              ></span>
              <span class="font-medium">
                {{
                  connState === 'connected' ? '已连接比赛服务器'
                  : connState === 'connecting' ? '正在连接服务器…'
                  : connState === 'failed' ? '服务器连接失败'
                  : '比赛服务器待配置'
                }}
              </span>
            </div>
            <span class="font-mono text-[10px] text-[var(--text-muted)]">Hinina v0.1.0</span>
          </div>
          <!-- 失败原因（IPC/网络错误的归一化消息，便于现场排障） -->
          <p
            v-if="connState === 'failed' && briefError"
            class="truncate text-[10px] leading-relaxed text-[var(--color-error)]"
            :title="briefError"
          >
            {{ briefError }}
          </p>
        </div>
      </section>

      <!-- ═══════ 右侧：氛围区（几何 SVG + 比赛信息 + 倒计时） ═══════ -->
      <section class="relative flex flex-1 flex-col justify-between overflow-hidden bg-[var(--bg-body)]/50 p-12 lg:p-14">
        <!-- 抽象几何 SVG -->
        <div class="pointer-events-none absolute inset-0 flex items-center justify-end pr-6">
          <svg class="absolute inset-0 h-full w-full" viewBox="0 0 700 700" fill="none">
            <defs>
              <linearGradient id="g1" x1="200" y1="200" x2="600" y2="600" gradientUnits="userSpaceOnUse">
                <stop offset="0%" stop-color="#7C5CFF" stop-opacity="0.18"></stop>
                <stop offset="50%" stop-color="#3B82F6" stop-opacity="0.08"></stop>
                <stop offset="100%" stop-color="#94A3B8" stop-opacity="0.02"></stop>
              </linearGradient>
              <linearGradient id="g2" x1="300" y1="100" x2="650" y2="500" gradientUnits="userSpaceOnUse">
                <stop offset="0%" stop-color="#6366F1" stop-opacity="0.12"></stop>
                <stop offset="100%" stop-color="#CBD5E1" stop-opacity="0.02"></stop>
              </linearGradient>
            </defs>
            <g transform="translate(120, -40)">
              <circle cx="420" cy="350" r="340" stroke="url(#g1)" stroke-width="1.2" stroke-dasharray="6 6" opacity="0.6"></circle>
              <circle cx="420" cy="350" r="280" stroke="url(#g1)" stroke-width="1.2" opacity="0.5"></circle>
              <circle cx="420" cy="350" r="220" stroke="url(#g2)" stroke-width="1.5" opacity="0.7"></circle>
              <circle cx="420" cy="350" r="160" stroke="url(#g1)" stroke-width="1" stroke-dasharray="3 8" opacity="0.8"></circle>
              <circle cx="420" cy="350" r="100" stroke="url(#g2)" stroke-width="1.2" opacity="0.6"></circle>
              <circle cx="420" cy="350" r="40" stroke="#7C5CFF" stroke-width="1.5" stroke-dasharray="2 4" opacity="0.4"></circle>
              <line x1="80" y1="350" x2="420" y2="350" stroke="url(#g1)" stroke-width="1" stroke-dasharray="4 6" opacity="0.4"></line>
              <line x1="420" y1="70" x2="420" y2="350" stroke="url(#g1)" stroke-width="1" stroke-dasharray="4 6" opacity="0.4"></line>
              <circle cx="420" cy="130" r="3" fill="#7C5CFF" opacity="0.4"></circle>
              <circle cx="200" cy="350" r="3" fill="#3B82F6" opacity="0.4"></circle>
            </g>
          </svg>
        </div>

        <!-- 比赛简介折叠面板（右上角） -->
        <div class="relative z-20 self-end">
          <button
            class="flex items-center gap-1.5 rounded-full border border-[var(--border-color)] bg-[var(--bg-card)]/80 px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] shadow-sm backdrop-blur transition-colors hover:text-[#6845f5]"
            @click="briefVisible = !briefVisible"
          >
            <svg class="h-3.5 w-3.5 stroke-current stroke-2 text-[#7c5cff]" fill="none" viewBox="0 0 24 24">
              <path d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"></path>
            </svg>
            比赛简介
            <svg class="h-3 w-3 text-[var(--text-muted)] transition-transform" :class="briefVisible ? 'rotate-180' : ''" fill="none" viewBox="0 0 24 24">
              <path d="M6 9l6 6 6-6" class="stroke-current stroke-2"></path>
            </svg>
          </button>

          <transition name="fade">
            <div v-if="briefVisible" class="absolute right-0 mt-2 w-80 rounded-2xl border border-[var(--border-color)] bg-[var(--bg-card)]/95 p-4 shadow-2xl backdrop-blur">
              <div class="mb-2 flex items-center justify-between border-b border-[var(--border-color)] pb-2">
                <span class="text-xs font-semibold text-[var(--text-primary)]">比赛须知与指南</span>
                <span class="rounded bg-[#f5f3ff] px-1.5 py-0.5 font-mono text-[10px] text-[#6845f5]">ACM / ICPC</span>
              </div>
              <div class="prose prose-sm max-w-none text-xs leading-relaxed text-[var(--text-secondary)]" v-html="renderedBrief || '<p class=\'text-[var(--text-muted)]\'>暂无比赛简介</p>'"></div>
            </div>
          </transition>
        </div>

        <!-- 比赛标题 -->
        <div class="relative z-10 mt-6 max-w-lg">
          <div class="mb-4 inline-flex items-center gap-2 rounded-full border border-[var(--border-color)] bg-[var(--bg-card)]/80 px-3 py-1 text-xs font-semibold text-[#6845f5] shadow-sm backdrop-blur">
            <span class="h-1.5 w-1.5 animate-pulse rounded-full bg-[#7c5cff]"></span>
            <span>{{ phaseLabel }}</span>
          </div>
          <h2 class="text-3xl font-extrabold leading-tight tracking-tight text-[var(--text-primary)] lg:text-4xl">
            {{ brief?.title ?? 'Hinina 竞赛' }}
          </h2>
          <p class="mt-2 max-w-sm text-xs tracking-wide text-[var(--text-secondary)]">
            {{ phaseDesc }}
          </p>
        </div>

        <!-- 倒计时 -->
        <div class="relative z-10 mb-2">
          <div class="mb-6 flex max-w-md items-center justify-between border-b border-[var(--border-color)]/60 pb-3">
            <span class="text-[11px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">距离比赛正式开始 COUNTDOWN</span>
            <div class="flex items-center gap-1 rounded-full border border-[var(--border-color)] bg-[var(--bg-card)] px-2 py-0.5 shadow-sm">
              <span class="h-2 w-2 rounded-full bg-[#3b82f6]"></span>
              <span class="h-2 w-2 rounded-full bg-[#f59e0b]"></span>
              <span class="h-2 w-2 rounded-full bg-[#ef4444]"></span>
            </div>
          </div>

          <div class="flex max-w-md items-baseline gap-6 font-mono tabular-nums lg:gap-8">
            <template v-for="(seg, i) in countdownSegs" :key="i">
              <div class="flex flex-col">
                <span :class="['text-4xl font-extrabold tracking-tight lg:text-5xl', segColor(seg.value)]">{{ String(seg.value).padStart(2, '0') }}</span>
                <span :class="['mt-1.5 text-[11px] tracking-wider', segColor(seg.value)]">{{ seg.label }}</span>
              </div>
              <span v-if="i < 3" class="-mt-4 text-2xl font-light text-[var(--text-muted)]">:</span>
            </template>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
