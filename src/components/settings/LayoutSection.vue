<script setup lang="ts">
import SettingsSectionFrame from './SettingsSectionFrame.vue'
import { computed } from 'vue'
import { clampRatio } from '@/composables/useSettingsForm'
import type { SettingsFormPatch } from '@/composables/useSettingsForm'

const props = defineProps<{ splitRatio: number }>()
const emit = defineEmits<{ (event: 'update:form', patch: SettingsFormPatch): void }>()
const splitPercent = computed(() => Math.round(props.splitRatio * 100))

function onSplitInput(event: Event): void {
  const value = Number((event.target as HTMLInputElement).value)
  if (Number.isFinite(value)) emit('update:form', { splitRatio: clampRatio(value) })
}
</script>

<template>
  <SettingsSectionFrame id="settings-2" title="布局">
    <template #icon>
      <rect x="3" y="4" width="18" height="16" rx="1.5" />
      <path d="M10 4v16" />
    </template>
              <div class="px-5 py-4">
                <div class="mb-2 flex items-center justify-between">
                  <span class="text-xs font-medium text-[var(--text-secondary)]">
                    解题页分栏比例
                  </span>
                  <span class="font-mono text-xs font-semibold text-[var(--color-primary)]">
                    左栏 {{ splitPercent }}%
                  </span>
                </div>
                <input
                  type="range"
                  min="0.3"
                  max="0.7"
                  step="0.01"
                  :value="splitRatio"
                  class="w-full accent-[var(--color-primary)]"
                  @input="onSplitInput"
                />
                <div
                  class="mt-1 flex justify-between font-mono text-xs text-[var(--text-muted)]"
                >
                  <span>30%</span>
                  <span>70%</span>
                </div>
                <p class="mt-2 text-xs text-[var(--text-muted)]">
                  拖拽解题页分栏条也会自动保存该比例
                </p>
              </div>
  </SettingsSectionFrame>
</template>
