<script setup lang="ts">
/// 通用加载动画组件。
defineProps<{ message?: string }>()
</script>

<template>
  <div role="status" class="flex flex-col items-center justify-center gap-4 py-12">
    <div aria-hidden="true" class="h-6 w-6 animate-spin rounded-full border border-[var(--border-color)] border-t-[var(--color-primary)]" />
    <span v-if="message" class="text-sm text-[var(--text-secondary)]">{{ message }}</span>
  </div>
</template>
