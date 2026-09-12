<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

const appWindow = getCurrentWindow()

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
    class="flex h-10 shrink-0 select-none items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-card)] px-4"
  >
    <!-- 左侧：Logo -->
    <div data-tauri-drag-region class="flex items-center gap-2.5">
      <div class="flex h-6 w-6 items-center justify-center rounded-md bg-gradient-to-tr from-[#5631e0] to-[#7c5cff] text-white">
        <svg class="h-3.5 w-3.5" fill="currentColor" viewBox="0 0 24 24">
          <path d="M12 2C9.5 5 7 7.5 7 11.5C7 14.54 9.46 17 12.5 17C10.5 15.5 10 13.5 10.5 11.8C11.5 14 13.5 14.5 14.5 13.2C15.8 11.5 15.5 9 14 7C16.8 8.8 18 11.4 18 14C18 17.87 14.87 21 11 21C6.03 21 2 16.97 2 12C2 7.8 5.5 4 8.5 2.5C9.5 2 10.8 1.5 12 2Z"></path>
        </svg>
      </div>
      <span class="text-sm font-bold tracking-tight text-[var(--text-primary)]">Hinina</span>
    </div>

    <!-- 右侧：窗口控制 -->
    <div class="flex items-center space-x-1">
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
</template>
