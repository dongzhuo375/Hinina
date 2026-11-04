<script setup lang="ts">
/**
 * 无边框窗口的最小化/最大化/关闭按钮（TopBar 与 `--settings` 极简壳共用）。
 *
 * 窗口 `decorations: false`，这些按钮是唯一的 UI 出口（Alt+F4/任务栏是系统
 * 兜底）。注意父容器要保留 `data-tauri-drag-region`：按钮点击不应触发拖动。
 */
import { useSystemStore } from '@/stores/systemStore'

// 分层：View → Store → Service → Bridge（2026-11-04 统一为严格口径）——
// 视图只调 store，窗口 API 由 bridge/window.bridge 封装（唯一接触
// @tauri-apps/api 的地方）
const systemStore = useSystemStore()

function minimize(): void {
  void systemStore.minimizeWindow()
}

function toggleMaximize(): void {
  void systemStore.toggleMaximizeWindow()
}

function closeAppWindow(): void {
  void systemStore.closeWindow()
}
</script>

<template>
  <div class="window-controls">
    <button type="button" aria-label="最小化" @click="minimize">
      <svg viewBox="0 0 16 16"><path d="M3 8h10" /></svg>
    </button>
    <button type="button" aria-label="最大化" @click="toggleMaximize">
      <svg viewBox="0 0 16 16"><rect x="3.5" y="3.5" width="9" height="9" rx="1" /></svg>
    </button>
    <button type="button" aria-label="关闭窗口" class="window-close" @click="closeAppWindow">
      <svg viewBox="0 0 16 16"><path d="m4 4 8 8m0-8-8 8" /></svg>
    </button>
  </div>
</template>

<style scoped>
.window-controls { display: flex; align-items: center; flex-shrink: 0; gap: 5px; margin-left: 10px; }
.window-controls button { width: 32px; height: 30px; display: grid; place-items: center; color: var(--text-secondary); }
.window-controls button:hover { background: var(--bg-sidebar); }
.window-controls .window-close:hover { background: #a33b2a; color: white; }
.window-controls svg { width: 13px; height: 13px; fill: none; stroke: currentColor; stroke-width: 1.5; }
</style>
