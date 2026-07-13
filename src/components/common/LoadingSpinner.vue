<script setup lang="ts">
/// 通用加载动画组件。
defineProps<{ message?: string }>()
</script>

<template>
  <div class="flex flex-col items-center justify-center gap-3 py-12">
    <div class="h-8 w-8 animate-spin rounded-full border-3 border-[var(--color-primary)] border-t-transparent" />
    <span v-if="message" class="text-sm text-[var(--text-secondary)]">{{ message }}</span>
  </div>
</template>
