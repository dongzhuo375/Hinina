<template>
  <div class="pointer-events-none fixed right-4 top-12 z-[9999] flex w-80 flex-col gap-2" aria-live="polite">
    <div v-for="item in store.notifications" :key="item.id"
      class="pointer-events-auto rounded-lg border border-[var(--border-color)] bg-[var(--bg-body)] p-3 text-sm text-[var(--text-primary)] shadow-lg">
      <div class="flex items-center justify-between gap-2">
        <span class="truncate text-xs text-[var(--text-muted)]">{{ item.pluginId }}</span>
        <button aria-label="关闭通知" @click="store.dismiss(item.id)">×</button>
      </div>
      <p class="mt-1 whitespace-pre-wrap break-words">{{ item.message }}</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { usePluginStore } from '@/stores/pluginStore'
const store = usePluginStore()
let unlisten: (() => void) | undefined
let disposed = false
onMounted(() => {
  void store.subscribe().then(stop => { if (disposed) stop(); else unlisten = stop })
    .catch(() => console.error('插件通知订阅失败'))
})
onUnmounted(() => { disposed = true; unlisten?.() })
</script>
