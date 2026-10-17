<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { getContestPhase } from '@/utils/contest'
import { renderMarkdown } from '@/utils/markdown'
import { configService } from '@/services/config.service'

const router = useRouter()
const auth = useAuthStore()
const contestStore = useContestStore()
const appWindow = getCurrentWindow()

/// 当前时间戳（秒），每秒刷新驱动倒计时与比赛阶段跃迁
const now = ref(Math.floor(Date.now() / 1000))
let timer: ReturnType<typeof setInterval> | null = null

onMounted(() => {
  timer = setInterval(() => {
    now.value = Math.floor(Date.now() / 1000)
  }, 1000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})

const contest = computed(() => contestStore.contest)

/// 比赛阶段（判据集中在 utils/contest，与登录页共用）
const phase = computed(() => getContestPhase(contest.value, now.value))

/// 状态徽章：进行中=竹青脉冲点 / 未开始=木色 / 已结束=灰
const phaseBadge = computed(() => {
  switch (phase.value) {
    case 'running':
      return {
        label: '进行中',
        pill: 'border-[#c2cfc4] bg-[#edf1ec] text-[#4a6153]',
        dot: 'bg-[#5f7a6a] animate-pulse',
      }
    case 'upcoming':
      return {
        label: '未开始',
        pill: 'border-[#e0d2b4] bg-[#f5efe3] text-[#8c6a43]',
        dot: 'bg-[#8c6a43]',
      }
    case 'ended':
      return {
        label: '已结束',
        pill: 'border-[var(--border-color)] bg-[var(--bg-sidebar)] text-[var(--text-secondary)]',
        dot: 'bg-[var(--text-muted)]',
      }
    default:
      return {
        label: '未加载',
        pill: 'border-[var(--border-color)] bg-[var(--bg-sidebar)] text-[var(--text-secondary)]',
        dot: 'bg-[var(--text-muted)]',
      }
  }
})

/// 比赛计时：总时长 = endTime - startTime；未开始时已用为 0，结束后封顶
const timing = computed(() => {
  const c = contest.value
  if (!c) return null
  const total = Math.max(1, c.endTime - c.startTime)
  const elapsed = Math.min(total, Math.max(0, now.value - c.startTime))
  return { total, elapsed, remaining: total - elapsed }
})

function formatHMS(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  const s = secs % 60
  return [h, m, s].map((v) => String(v).padStart(2, '0')).join(':')
}

/// 已用时间占比（驱动 amber 进度条）
const progressPercent = computed(() =>
  timing.value ? (timing.value.elapsed / timing.value.total) * 100 : 0,
)

// ── 比赛简介弹层 ──

const briefOpen = ref(false)
const briefHtml = ref('')

/// OJ 基址须在打开时异步读取（用于把简介中的相对图片地址改写为绝对地址）
async function toggleBrief() {
  briefOpen.value = !briefOpen.value
  if (!briefOpen.value) return
  const baseUrl = await configService.getOjBaseUrl()
  briefHtml.value = renderMarkdown(contest.value?.description ?? '', baseUrl)
}

// ── 用户菜单与登出 ──

const userMenuOpen = ref(false)
/// 登出请求进行中标记，防止重复点击
const isLoggingOut = ref(false)

const userInitial = computed(() => (auth.username ?? '?').charAt(0).toUpperCase())

/**
 * 登出并返回登录页。
 *
 * `authStore.logout()` 不会 reject（后端失败也会清理本地会话与会话级领域状态），
 * 因此 `finally` 中的跳转必然执行，不会把用户滞留在比赛页面。
 */
async function handleLogout() {
  if (isLoggingOut.value) return
  isLoggingOut.value = true
  try {
    await auth.logout()
  } finally {
    isLoggingOut.value = false
    userMenuOpen.value = false
    await router.replace({ name: 'Login' })
  }
}

// ── 窗口控制 ──

function minimize() {
  appWindow.minimize()
}
function toggleMaximize() {
  appWindow.toggleMaximize()
}
function close() {
  appWindow.close()
}
</script>

<template>
  <header
    data-tauri-drag-region
    class="relative z-50 flex h-14 shrink-0 select-none items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-card)] px-5"
  >
    <!-- 左侧：Logo + 比赛状态徽章 + 标题 -->
    <div data-tauri-drag-region class="flex min-w-0 items-center gap-3">
      <div
        data-tauri-drag-region
        class="flex h-7 w-7 shrink-0 items-center justify-center bg-[#2e3033] text-xs font-semibold text-[#f7f6f1]"
      >
        H
      </div>
      <span data-tauri-drag-region class="shrink-0 text-sm font-semibold tracking-[0.12em] text-[var(--text-primary)]">
        Hinina
      </span>
      <div class="mx-1 h-4 w-px shrink-0 bg-[var(--border-color)]"></div>
      <span
        class="inline-flex shrink-0 items-center gap-1.5 rounded-[3px] border px-2 py-0.5 text-[10px] font-medium tracking-[0.15em]"
        :class="phaseBadge.pill"
      >
        <span class="h-1.5 w-1.5 rounded-full" :class="phaseBadge.dot"></span>
        {{ phaseBadge.label }}
      </span>
      <h1
        v-if="contest"
        class="truncate text-sm font-medium text-[var(--text-primary)]"
        :title="contest.title"
      >
        {{ contest.title }}
      </h1>
    </div>

    <!-- 中间：剩余时间（精密仪表）+ 细进度轨 -->
    <div data-tauri-drag-region class="hidden flex-col items-center justify-center px-4 lg:flex">
      <div data-tauri-drag-region class="flex items-baseline gap-2 font-mono">
        <span class="text-[9px] tracking-[0.25em] text-[var(--text-muted)]">REMAINING</span>
        <span class="tabular-nums text-base font-semibold tracking-wider text-[var(--text-primary)]">
          {{ timing ? formatHMS(timing.remaining) : '--:--:--' }}
        </span>
        <span class="tabular-nums text-[10px] text-[var(--text-muted)]">
          / {{ timing ? formatHMS(timing.total) : '--:--:--' }}
        </span>
      </div>
      <div
        data-tauri-drag-region
        class="mt-1.5 h-px w-56 bg-[var(--border-color)]"
      >
        <div class="h-full bg-[#2e3033] transition-[width] duration-500 ease-out" :style="{ width: `${progressPercent}%` }"></div>
      </div>
    </div>

    <!-- 右侧：比赛简介 + 用户 + 窗口控制（可交互元素一律不带 drag region） -->
    <div class="flex shrink-0 items-center gap-2.5">
      <button
        type="button"
        class="flex items-center gap-1.5 rounded-[3px] border border-[var(--border-color)] bg-[var(--bg-card)] px-2.5 py-1 text-xs font-medium text-[var(--text-secondary)] transition-colors duration-200 ease-out hover:border-[#8c6a43] hover:text-[#8c6a43]"
        @click="toggleBrief"
      >
        <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
          ></path>
        </svg>
        比赛简介
      </button>

      <div class="relative">
        <button
          type="button"
          class="flex items-center gap-2 rounded-[3px] border border-[var(--border-color)] bg-[var(--bg-card)] px-2.5 py-1 text-xs transition-colors duration-200 ease-out hover:border-[#8c6a43]"
          @click="userMenuOpen = !userMenuOpen"
        >
          <span
            class="flex h-5 w-5 items-center justify-center bg-[#2e3033] text-[10px] font-semibold text-[#f7f6f1]"
          >
            {{ userInitial }}
          </span>
          <span class="max-w-[120px] truncate font-mono font-medium text-[var(--text-primary)]">
            {{ auth.username }}
          </span>
          <svg
            class="h-3 w-3 text-[var(--text-muted)] transition-transform duration-200 ease-out"
            :class="{ 'rotate-180': userMenuOpen }"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M6 9l6 6 6-6" stroke-linecap="round" stroke-linejoin="round"></path>
          </svg>
        </button>

        <!-- 用户下拉菜单 -->
        <template v-if="userMenuOpen">
          <div class="fixed inset-0 z-40" @click="userMenuOpen = false"></div>
          <div
            class="absolute right-0 top-full z-50 mt-2 w-40 overflow-hidden rounded-[3px] border border-[var(--border-color)] bg-[var(--bg-card)] py-1 shadow-[var(--shadow-float)]"
          >
            <button
              type="button"
              class="flex w-full items-center gap-2 px-3 py-2 text-left text-xs font-medium transition-colors duration-200 ease-out hover:bg-[var(--bg-sidebar)] disabled:cursor-not-allowed disabled:opacity-60"
              :class="isLoggingOut ? 'text-[var(--text-muted)]' : 'text-[var(--color-error)]'"
              :disabled="isLoggingOut"
              @click="handleLogout"
            >
              <svg class="h-3.5 w-3.5 shrink-0" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" stroke-linecap="round" stroke-linejoin="round"></path>
                <path d="M16 17l5-5-5-5" stroke-linecap="round" stroke-linejoin="round"></path>
                <path d="M21 12H9" stroke-linecap="round" stroke-linejoin="round"></path>
              </svg>
              {{ isLoggingOut ? '登出中…' : '登出' }}
            </button>
          </div>
        </template>
      </div>

      <!-- 窗口控制 -->
      <div class="flex items-center gap-1 border-l border-[var(--border-color)] pl-2.5">
        <button
          type="button"
          aria-label="最小化"
          title="最小化"
          class="flex h-7 w-7 items-center justify-center rounded-[3px] text-[var(--text-muted)] transition-colors duration-200 ease-out hover:bg-[var(--bg-sidebar)] hover:text-[var(--text-primary)]"
          @click="minimize"
        >
          <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="M20 12H4" stroke-linecap="round" stroke-linejoin="round" stroke-width="2"></path>
          </svg>
        </button>
        <button
          type="button"
          aria-label="最大化"
          title="最大化"
          class="flex h-7 w-7 items-center justify-center rounded-[3px] text-[var(--text-muted)] transition-colors duration-200 ease-out hover:bg-[var(--bg-sidebar)] hover:text-[var(--text-primary)]"
          @click="toggleMaximize"
        >
          <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <rect height="16" rx="1.5" stroke-width="2" width="16" x="4" y="4"></rect>
          </svg>
        </button>
        <button
          type="button"
          aria-label="关闭窗口"
          title="关闭"
          class="flex h-7 w-7 items-center justify-center rounded-[3px] text-[var(--text-muted)] transition-colors duration-200 ease-out hover:bg-[#a33b2a] hover:text-white"
          @click="close"
        >
          <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round" stroke-width="2"></path>
          </svg>
        </button>
      </div>
    </div>

    <!-- 比赛简介弹层 -->
    <template v-if="briefOpen">
      <div class="fixed inset-0 z-40" @click="briefOpen = false"></div>
      <div
        class="absolute right-5 top-full z-50 mt-2 max-h-[70vh] w-[440px] overflow-y-auto rounded-[4px] border border-[var(--border-color)] bg-[var(--bg-card)] p-6 shadow-[var(--shadow-float)]"
      >
        <div class="mb-4 flex items-center justify-between border-b border-[var(--border-color)] pb-3">
          <h2 class="text-[10px] font-medium tracking-[0.25em] text-[var(--text-muted)]">比赛简介</h2>
          <button
            type="button"
            aria-label="关闭简介"
            class="flex h-6 w-6 items-center justify-center rounded-[3px] text-[var(--text-muted)] transition-colors duration-200 ease-out hover:bg-[var(--bg-sidebar)] hover:text-[var(--text-primary)]"
            @click="briefOpen = false"
          >
            <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round"></path>
            </svg>
          </button>
        </div>
        <div
          v-if="briefHtml"
          class="prose prose-sm max-w-none text-xs leading-relaxed text-[var(--text-secondary)]"
          v-html="briefHtml"
        ></div>
        <p v-else class="text-xs text-[var(--text-muted)]">暂无比赛简介</p>
      </div>
    </template>
  </header>
</template>
