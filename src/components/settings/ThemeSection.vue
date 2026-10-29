<script setup lang="ts">
import SettingsSectionFrame from './SettingsSectionFrame.vue'
import { SETTINGS_INPUT as INPUT } from '@/composables/useSettingsForm'
</script>

<template>
  <SettingsSectionFrame id="settings-3">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <circle cx="12" cy="12" r="4" />
                  <path
                    d="M12 2v2m0 16v2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4M2 12h2m16 0h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"
                    stroke-linecap="round"
                  />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">主题</h2>
              </div>
              <div class="grid grid-cols-1 gap-x-4 gap-y-4 px-5 py-4 sm:grid-cols-2">
                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    界面主题
                  </span>
                  <select disabled :class="INPUT">
                    <option>浅色</option>
                  </select>
                </label>
                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    编辑器主题
                  </span>
                  <select disabled :class="INPUT">
                    <option>由解题页「编辑器设置」控制</option>
                  </select>
                </label>
                <p class="text-xs text-[var(--text-muted)] sm:col-span-2">
                  界面暗色主题即将上线，当前版本固定浅色；编辑器主题在解题页「编辑器设置」中即时切换
                </p>
              </div>
  </SettingsSectionFrame>
</template>
