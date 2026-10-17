<script setup lang="ts">
/// 通用错误提示组件。
defineProps<{ message: string; retry?: () => void }>()
</script>

<template>
  <div class="flex flex-col items-center justify-center gap-3 py-12 text-center">
    <div class="text-4xl">⚠</div>
    <p class="text-sm text-[var(--text-secondary)] max-w-md">{{ message }}</p>
    <button
      v-if="retry"
      class="rounded-[3px] bg-[var(--color-primary)] px-4 py-1.5 text-sm text-[#f7f6f1] transition-colors duration-200 ease-out hover:bg-[var(--color-primary-hover)]"
      @click="retry"
    >
      重试
    </button>
  </div>
</template>
