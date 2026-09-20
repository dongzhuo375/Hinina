<script setup lang="ts">
import { computed, onUnmounted, ref } from 'vue'
import { useContestStore } from '@/stores/contestStore'
import { tapVersion } from '@/utils/settings-access'
import { createLogger } from '@/utils/logger'

const log = createLogger('StatusBar')

const contestStore = useContestStore()

/// 连接状态判据：比赛数据已加载且无错误（复用 store 状态，不额外发探测请求）
const connected = computed(() => contestStore.contest !== null && !contestStore.error)

/// 版本号构建期注入（vite define ← package.json），写死会与发布版本漂移
const version = __APP_VERSION__

/// 解锁瞬间的提示：连点 5 下本身没有任何视觉反馈，若设置项不在当前视野内，
/// 用户无法判断「到底生效没有」。给一次 3 秒提示，之后不再打扰。
const unlockedHint = ref(false)
let hintTimer: ReturnType<typeof setTimeout> | null = null

function onVersionClick() {
  if (!tapVersion()) return

  log.info('设置入口已解锁（连点版本号）')
  unlockedHint.value = true
  if (hintTimer) clearTimeout(hintTimer)
  hintTimer = setTimeout(() => {
    hintTimer = null
    unlockedHint.value = false
  }, 3_000)
}

onUnmounted(() => {
  if (hintTimer) clearTimeout(hintTimer)
})
</script>

<template>
  <footer
    class="flex shrink-0 select-none items-center justify-between border-t border-[var(--border-color)] bg-[var(--bg-card)] px-6 py-2"
  >
    <div class="flex items-center gap-1.5 text-xs">
      <span
        class="h-2 w-2 rounded-full"
        :class="connected ? 'animate-pulse bg-[#22c55e]' : 'bg-[var(--color-error)]'"
      ></span>
      <span
        class="font-medium"
        :class="connected ? 'text-[var(--text-secondary)]' : 'text-[var(--color-error)]'"
      >
        {{ connected ? '已连接比赛服务器' : '连接异常' }}
      </span>
    </div>

    <div class="flex items-center gap-2">
      <span v-if="unlockedHint" class="font-mono text-[10px] text-[var(--color-primary)]">
        设置已解锁
      </span>
      <!--
        版本号即隐藏入口：连点 5 下解锁左侧活动栏的设置项（见 utils/settings-access）。
        刻意**不给任何可点击的视觉暗示**（无 title、光标保持默认、hover 无变化）——
        它要防的是误触，不是引导用户去点。
      -->
      <button
        type="button"
        class="cursor-default font-mono text-[10px] text-[var(--text-muted)]"
        @click="onVersionClick"
      >
        Hinina v{{ version }}
      </button>
    </div>
  </footer>
</template>
