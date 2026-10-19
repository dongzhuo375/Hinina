<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { getContestPhase } from '@/utils/contest'
import { renderMarkdown } from '@/utils/markdown'
import ActivityBar from '@/components/layout/ActivityBar.vue'
import { vFocusScope } from '@/utils/focus-scope'
import { configService } from '@/services/config.service'

const router = useRouter()
const auth = useAuthStore()
const contestStore = useContestStore()
const appWindow = getCurrentWindow()
defineProps<{ navigationDisabled?: boolean }>()

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

/// 状态徽章：进行中=绿色脉冲点 / 未开始=amber / 已结束=灰
const phaseBadge = computed(() => {
  switch (phase.value) {
    case 'running':
      return {
        label: '进行中',
        pill: 'border-[var(--color-ac-border)] bg-[var(--color-ac-bg)] text-[var(--color-success)]',
        dot: 'bg-[var(--color-success)]',
      }
    case 'upcoming':
      return {
        label: '未开始',
        pill: 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-warning)]',
        dot: 'bg-[var(--color-warning)]',
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
  <header class="contest-header">
    <div class="window-bar" data-tauri-drag-region>
      <div class="window-brand" data-tauri-drag-region>
        <svg class="brand-monogram" viewBox="0 0 64 64" aria-hidden="true" data-tauri-drag-region>
          <rect width="64" height="64" fill="#2E3033" />
          <rect x="16" y="16" width="9" height="32" fill="#F7F6F1" />
          <polygon points="39,16 42,16 48,22 48,48 39,48" fill="#F7F6F1" />
          <rect x="25" y="29" width="14" height="6" fill="#8C6A43" />
        </svg>
        <span class="brand-word" data-tauri-drag-region>Hinina</span>
        <span class="brand-divider" aria-hidden="true"></span>
        <span v-if="contest" class="window-event" :title="contest.title" data-tauri-drag-region>{{ contest.title }}</span>
      </div>
      <div class="window-actions">
        <button class="action-quiet" :aria-expanded="briefOpen" @click="toggleBrief">比赛简介</button>
        <div class="relative">
          <button class="account-button" :aria-expanded="userMenuOpen" @click="userMenuOpen = !userMenuOpen">
            <span class="account-initial">{{ userInitial }}</span>
            <span class="max-w-[120px] truncate">{{ auth.username }}</span><span aria-hidden="true">⌄</span>
          </button>
          <template v-if="userMenuOpen">
            <div class="fixed inset-0 z-40" @click="userMenuOpen = false"></div>
            <div v-focus-scope="() => { userMenuOpen = false }" role="dialog" aria-label="账号操作"
              class="surface-overlay absolute right-0 top-full z-50 mt-2 w-44 p-2">
              <button class="action-quiet w-full text-left" :disabled="isLoggingOut" @click="handleLogout">{{ isLoggingOut ? '登出中…' : '登出' }}</button>
            </div>
          </template>
        </div>
        <div class="window-controls">
          <button aria-label="最小化" @click="minimize"><svg viewBox="0 0 16 16"><path d="M3 8h10" /></svg></button>
          <button aria-label="最大化" @click="toggleMaximize"><svg viewBox="0 0 16 16"><rect x="3.5" y="3.5" width="9" height="9" rx="1" /></svg></button>
          <button aria-label="关闭窗口" class="window-close" @click="close"><svg viewBox="0 0 16 16"><path d="m4 4 8 8m0-8-8 8" /></svg></button>
        </div>
      </div>
    </div>
    <div class="navigation-deck" :inert="navigationDisabled">
      <ActivityBar />
      <div class="race-instrument" aria-label="比赛计时">
        <span class="phase-label"><span class="h-1 w-1 rounded-full" :class="phaseBadge.dot"></span>{{ phaseBadge.label }}</span>
        <span class="clock-caption">剩余</span>
        <span class="clock-value">{{ timing ? formatHMS(timing.remaining) : '--:--:--' }}</span>
        <span class="clock-total">/ {{ timing ? formatHMS(timing.total) : '--:--:--' }}</span>
        <div class="clock-track" aria-hidden="true"><div :style="{ width: `${progressPercent}%` }"></div></div>
      </div>
    </div>
    <template v-if="briefOpen">
      <div class="fixed inset-0 z-40 bg-[#2e3033]/15" @click="briefOpen = false"></div>
      <section v-focus-scope="() => { briefOpen = false }" role="dialog" aria-modal="true" aria-labelledby="brief-title"
        class="surface-overlay absolute right-6 top-12 z-50 max-h-[75vh] w-[480px] max-w-[calc(100vw-48px)] overflow-y-auto p-8">
        <div class="mb-6 flex items-start justify-between gap-4">
          <div><p class="eyebrow mb-3">CONTEST / INFORMATION</p><h2 id="brief-title" class="text-xl font-semibold">比赛简介</h2></div>
          <button class="action-quiet" aria-label="关闭简介" @click="briefOpen = false">关闭 ×</button>
        </div>
        <div v-if="briefHtml" class="reading-body" v-html="briefHtml"></div>
        <p v-else class="text-sm text-[var(--text-muted)]">暂无比赛简介</p>
      </section>
    </template>
  </header>
</template>
<style scoped>
.contest-header { position: relative; z-index: 50; flex-shrink: 0; background: var(--bg-body); }
.window-bar { height: 40px; display: flex; align-items: center; justify-content: space-between; gap: 20px; padding: 0 16px 0 24px; user-select: none; }
.window-brand { display: flex; align-items: center; gap: 10px; min-width: 0; }
.brand-monogram { width: 23px; height: 23px; flex-shrink: 0; }
.brand-word { font-size: 12px; letter-spacing: .12em; font-weight: 600; }
.brand-divider { height: 12px; width: 1px; background: var(--border-color); margin: 0 8px; flex-shrink: 0; }
.window-event { font-size: var(--text-xs); color: var(--text-secondary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.window-actions, .window-controls { display: flex; align-items: center; flex-shrink: 0; gap: 5px; }
.window-controls { margin-left: 10px; }
.window-controls button { width: 32px; height: 30px; display: grid; place-items: center; color: var(--text-secondary); }
.window-controls button:hover { background: var(--bg-sidebar); }
.window-controls .window-close:hover { background: #a33b2a; color: white; }
.window-controls svg { width: 13px; height: 13px; fill: none; stroke: currentColor; stroke-width: 1.5; }
.account-button { display: flex; align-items: center; gap: 8px; font-size: var(--text-xs); padding: 4px 10px; }
.account-initial { font-family: var(--font-mono); color: #8c6a43; }
.navigation-deck { display: flex; align-items: center; justify-content: space-between; gap: 24px; padding: 0 var(--page-gutter); border-bottom: 1px solid var(--border-color); }
.race-instrument { position: relative; display: flex; align-items: center; gap: 12px; height: calc(48px + var(--ui-step) * 4); flex-shrink: 0; }
.phase-label { display: flex; align-items: center; gap: 6px; font-size: var(--text-xs); color: var(--text-secondary); margin-right: 12px; }
.clock-caption { font-size: var(--text-xs); color: var(--text-muted); }
.clock-value { font: calc(22px + var(--ui-step) * 3) var(--font-mono); font-variant-numeric: tabular-nums; letter-spacing: -.05em; }
.clock-total { font: var(--text-xs) var(--font-mono); color: var(--text-muted); }
.clock-track { position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; background: var(--border-color); }
.clock-track div { height: 100%; background: #8c6a43; }
@media (max-width: 1100px) { .navigation-deck { padding: 0 24px; gap: 16px; } .clock-total { display: none; } .phase-label { margin-right: 0; } }
@media (max-width: 850px) { .clock-caption, .brand-divider, .window-event { display: none; } .navigation-deck { gap: 12px; } .race-instrument { gap: 8px; } }
</style>
