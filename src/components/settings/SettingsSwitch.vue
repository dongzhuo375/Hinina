<script setup lang="ts">
/// Settings 开关行：role=switch 按钮圆钮 + 「已开启/已关闭」状态文本。
/// 按钮内只有圆钮无可见文本，可访问名经 aria-label=label 提供（状态由 aria-checked 表达）。
/// `settings-switch(-knob)` class 由 SettingsSectionFrame 的 :deep 样式消费，不得改名。
defineProps<{
  /** 开关的可访问名（设置项名称，如「题面缓存」） */
  label: string
  checked: boolean
  disabled?: boolean
}>()
const emit = defineEmits<{ (event: 'update', checked: boolean): void }>()
</script>

<template>
  <div class="flex items-center gap-3 py-1">
    <button
      type="button"
      role="switch"
      :aria-label="label"
      :aria-checked="checked"
      :disabled="disabled"
      class="settings-switch relative h-6 w-11 shrink-0 transition-colors disabled:cursor-not-allowed disabled:opacity-60"
      :class="checked ? 'bg-[var(--color-primary)]' : 'bg-[var(--bg-sidebar)]'"
      @click="emit('update', !checked)"
    >
      <span
        class="settings-switch-knob absolute top-0.5 left-0.5 h-5 w-5 rounded-sm bg-[var(--bg-card)] transition-transform"
        :class="checked ? 'translate-x-5' : ''"
      ></span>
    </button>
    <span class="text-xs text-[var(--text-secondary)]">
      {{ checked ? '已开启' : '已关闭' }}
    </span>
  </div>
</template>
