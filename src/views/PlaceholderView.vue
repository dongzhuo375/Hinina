<script setup lang="ts">
/// 评测 / 公告 / 设置共用的占位页，文案由路由 `props` 注入
defineProps<{
  title: string
  description: string
}>()
</script>

<template>
  <div class="flex flex-1 flex-col items-center justify-center gap-3 overflow-y-auto p-8 text-center">
    <div
      class="flex h-14 w-14 items-center justify-center rounded-2xl border border-[#ede9fe] bg-[#f5f3ff] text-[var(--color-primary)]"
    >
      <svg class="h-7 w-7" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
        <path
          d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"
          stroke-linecap="round"
          stroke-linejoin="round"
        ></path>
        <path d="m3.3 7 8.7 5 8.7-5" stroke-linecap="round" stroke-linejoin="round"></path>
        <path d="M12 22V12" stroke-linecap="round" stroke-linejoin="round"></path>
      </svg>
    </div>
    <h1 class="text-lg font-bold tracking-tight text-[var(--text-primary)]">{{ title }}</h1>
    <p class="max-w-sm text-sm leading-relaxed text-[var(--text-secondary)]">{{ description }}</p>
    <p class="mt-1 text-xs text-[var(--text-muted)]">该模块尚未接入服务端接口</p>
  </div>
</template>
