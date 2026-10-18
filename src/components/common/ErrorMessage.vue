<script setup lang="ts">
/// 通用错误提示组件。
defineProps<{ message: string; retry?: () => void }>()
</script>

<template>
  <div role="alert" class="flex flex-col items-center justify-center gap-4 px-6 py-12 text-center">
    <svg aria-hidden="true" class="h-8 w-8 text-[var(--color-error)]" viewBox="0 0 32 32" fill="none" stroke="currentColor"><path d="M8 3h16l5 5v16l-5 5H8l-5-5V8Z"/><path d="M16 9v9m0 4v1" stroke-width="2"/></svg>
    <p class="text-sm text-[var(--text-secondary)] max-w-md break-words">{{ message }}</p>
    <button
      v-if="retry"
      class="rounded-md bg-[var(--color-primary)] px-4 py-1.5 text-sm text-white hover:bg-[var(--color-primary-hover)] transition-colors"
      @click="retry"
    >
      重试
    </button>
  </div>
</template>
