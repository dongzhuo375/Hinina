<script setup lang="ts">
defineProps<{ id: string }>()
</script>

<template>
  <!-- 真实 section 保留 scroll-spy 的 id/focus 契约；组件边界不出现在 DOM 中 -->
  <section :id="id" tabindex="-1" class="settings-section">
    <slot />
  </section>
</template>

<style scoped>
.settings-section { scroll-margin-top: 24px; border-top: 1px solid var(--border-color); }
:deep(.settings-section > div:first-child) { border: 0; padding: 20px 0 8px; }
:deep(.settings-section > div:first-child svg) { display: none; }
:deep(.settings-section h2) { font-size: 18px; font-weight: 600; }
:deep(.settings-section > div:not(:first-child)) { padding-left: 0; padding-right: 0; }
:deep(.settings-section :is(input:not([type=range]):not([type=checkbox]), select)) {
  border-radius: 0;
  border-width: 0 0 1px;
  background: transparent;
  padding-left: 0;
}
:deep(.settings-section input[inputmode="numeric"]) {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}
:deep(.settings-switch) { border-radius: 2px; }
:deep(.settings-switch-knob) { box-shadow: 0 1px 3px #2e303326; }
</style>
