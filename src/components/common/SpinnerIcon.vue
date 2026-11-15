<script setup lang="ts">
/// 旋转加载图标：尺寸/颜色经 class 透传（如 h-3 w-3 / text-[var(--color-warning)]），
/// 细节粗细（状态 pill 用 2.5，按钮用 2）经 strokeWidth 覆盖。
defineProps<{ strokeWidth?: number | string }>()
</script>

<template>
  <svg
    class="animate-spin"
    fill="none"
    stroke="currentColor"
    :stroke-width="strokeWidth ?? 2"
    viewBox="0 0 24 24"
    aria-hidden="true"
  >
    <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
    <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
  </svg>
</template>
