<script setup lang="ts">
/// 区块头并入 Frame 后统一图标容器/标题/右侧提示的形态；
/// 头部 DOM 形态（section > div:first-child > svg + h2 [+ span]）是本组件
/// :deep 样式的目标（隐藏图标、重排首块 padding），不得改变层级。
defineProps<{
  id: string
  title: string
  /** 区块头右侧提示（如维护者锁定说明）；不传则不渲染 */
  hint?: string
}>()
</script>

<template>
  <!-- 真实 section 保留 scroll-spy 的 id/focus 契约；组件边界不出现在 DOM 中 -->
  <section :id="id" tabindex="-1" class="settings-section">
    <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
      <svg
        class="h-4 w-4 text-[var(--color-primary)]"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        viewBox="0 0 24 24"
        aria-hidden="true"
      >
        <slot name="icon" />
      </svg>
      <h2 class="text-sm font-semibold text-[var(--text-primary)]">{{ title }}</h2>
      <span v-if="hint" class="ml-auto text-xs text-[var(--text-muted)]">{{ hint }}</span>
    </div>
    <slot />
  </section>
</template>

<style scoped>
.settings-section { scroll-margin-top: 24px; border-top: 1px solid var(--border-color); }
.settings-section :deep(> div:first-child) { border: 0; padding: 20px 0 8px; }
.settings-section :deep(> div:first-child svg) { display: none; }
.settings-section :deep(h2) { font-size: 18px; font-weight: 600; }
.settings-section :deep(> div:not(:first-child)) { padding-left: 0; padding-right: 0; }
.settings-section :deep(:is(input:not([type=range]):not([type=checkbox]), select)) {
  border-radius: 0;
  border-width: 0 0 1px;
  background: transparent;
  padding-left: 0;
}
.settings-section :deep(input[inputmode="numeric"]) {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}
.settings-section :deep(.settings-switch) { border-radius: 2px; }
.settings-section :deep(.settings-switch-knob) { box-shadow: 0 1px 3px #2e303326; }
</style>
