<script setup lang="ts">
/// Settings 表单字段脚手架：label 文本 + 控件（默认 slot）+ 错误/提示行。
///
/// `<label>` 包裹控件的 DOM 形态是测试契约（SettingsView.behavior.spec 的
/// field() 按「label 首个直接子 span 文本 → label 内 input/select」定位控件），
/// 不得改为平铺结构。控件本身（v-model/校验态 class/disabled）留在调用方。
defineProps<{
  label: string
  /** 锁定态：label 降为 muted 色（维护者钥匙解锁语义），控件禁用由调用方处理 */
  locked?: boolean
  /** 校验错误：存在时替代提示行展示 */
  error?: string | null
  /** 常态提示文案 */
  hint?: string
}>()
</script>

<template>
  <label class="block">
    <span
      class="mb-1 block text-xs font-medium"
      :class="locked ? 'text-[var(--text-muted)]' : 'text-[var(--text-secondary)]'"
    >
      {{ label }}
    </span>
    <slot />
    <span v-if="error" class="mt-1 block text-xs text-[var(--color-error)]">{{ error }}</span>
    <span v-else-if="hint" class="mt-1 block text-xs text-[var(--text-muted)]">{{ hint }}</span>
  </label>
</template>
