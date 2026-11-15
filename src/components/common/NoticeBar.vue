<script setup lang="ts">
/// 页面顶部的状态横幅（榜单/评测页共用）：不打断阅读、不清空已有数据。
///
/// tone 决定配色（wa = 刷新失败保留旧数据 / pending = 封榜等信息态）；
/// 消息内容经默认 slot 自由组合（截断单段或双段强调）；重试动作沿用
/// ErrorMessage 的 `retry?: () => void` 惯例，传入即渲染「立即重试」按钮。
defineProps<{
  tone: 'wa' | 'pending'
  retry?: () => void
}>()
</script>

<template>
  <div
    class="flex shrink-0 items-center gap-2 rounded-sm border px-3 py-2 text-xs"
    :class="
      tone === 'wa'
        ? 'border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] text-[var(--color-error)]'
        : 'border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] text-[var(--color-warning)]'
    "
  >
    <slot name="icon">
      <svg
        v-if="tone === 'wa'"
        class="h-3.5 w-3.5 shrink-0"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        viewBox="0 0 24 24"
        aria-hidden="true"
      >
        <path
          d="M12 9v4m0 4h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </slot>
    <slot />
    <button
      v-if="retry"
      type="button"
      class="shrink-0 rounded-md border border-[var(--color-wa-border)] bg-[var(--bg-card)] px-2 py-0.5 font-medium transition-colors hover:bg-[var(--color-wa-bg)]"
      @click="retry"
    >
      立即重试
    </button>
  </div>
</template>
