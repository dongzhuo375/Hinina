<template>
  <n-config-provider :theme="null" :theme-overrides="themeOverrides">
    <n-dialog-provider>
      <div class="flex h-screen w-screen flex-col overflow-hidden rounded-xl border border-[var(--border-color)] bg-[var(--bg-body)] shadow-2xl">
        <!-- 兜底窗口控制条：比赛外壳自带 TopBar（含窗口控制），
             外壳之外的路由（当前只有登录页）由此处提供拖拽区与窗口按钮 -->
        <header
          v-if="showGlobalTitleBar"
          data-tauri-drag-region
          class="flex h-10 shrink-0 select-none items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-card)] px-4"
        >
          <div data-tauri-drag-region class="flex items-center gap-2.5">
            <div
              data-tauri-drag-region
              class="flex h-6 w-6 items-center justify-center rounded-md bg-gradient-to-tr from-[#5631e0] to-[#7c5cff] text-xs font-bold text-white"
            >
              H
            </div>
            <span data-tauri-drag-region class="text-sm font-bold tracking-tight text-[var(--text-primary)]">Hinina</span>
          </div>
          <div class="flex items-center gap-1">
            <button
              aria-label="最小化"
              class="flex h-8 w-8 items-center justify-center rounded-lg text-[var(--text-muted)] transition-colors hover:bg-[var(--bg-sidebar)] hover:text-[var(--text-primary)]"
              @click="minimize"
            >
              <svg class="h-3.5 w-3.5 stroke-current stroke-2" fill="none" viewBox="0 0 16 16">
                <path d="M3 8H13"></path>
              </svg>
            </button>
            <button
              aria-label="最大化"
              class="flex h-8 w-8 items-center justify-center rounded-lg text-[var(--text-muted)] transition-colors hover:bg-[var(--bg-sidebar)] hover:text-[var(--text-primary)]"
              @click="toggleMaximize"
            >
              <svg class="h-3.5 w-3.5 stroke-current stroke-2" fill="none" viewBox="0 0 16 16">
                <rect height="9" rx="1.5" width="9" x="3.5" y="3.5"></rect>
              </svg>
            </button>
            <button
              aria-label="关闭窗口"
              class="flex h-8 w-8 items-center justify-center rounded-lg text-[var(--text-muted)] transition-colors hover:bg-[#e81123] hover:text-white"
              @click="close"
            >
              <svg class="h-3.5 w-3.5 stroke-current stroke-2" fill="none" viewBox="0 0 16 16">
                <path d="M4 4L12 12M12 4L4 12"></path>
              </svg>
            </button>
          </div>
        </header>
        <div class="flex-1 overflow-hidden">
          <router-view />
        </div>
      </div>
    </n-dialog-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { NConfigProvider, NDialogProvider } from 'naive-ui'
import type { GlobalThemeOverrides } from 'naive-ui'
import { getCurrentWindow } from '@tauri-apps/api/window'

const route = useRoute()
const appWindow = getCurrentWindow()

/// 比赛外壳（ContestLayout，name: 'Contest'）的 TopBar 已含窗口控制，
/// 外壳之外的路由（当前只有登录页）由 App 渲染精简窗口条，避免窗口无法关闭/最小化
const showGlobalTitleBar = computed(() => !route.matched.some((r) => r.name === 'Contest'))

function minimize() {
  appWindow.minimize()
}
function toggleMaximize() {
  appWindow.toggleMaximize()
}
function close() {
  appWindow.close()
}

/// Naive UI 主题覆盖（CSS 变量驱动的暗色适配）
const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#7C5CFF',
    primaryColorHover: '#6845F5',
    primaryColorPressed: '#5631E0',
    primaryColorSuppl: '#7C5CFF',
    infoColor: '#2563EB',
    successColor: '#22C55E',
    warningColor: '#F59E0B',
    errorColor: '#EF4444',
  },
}
</script>
