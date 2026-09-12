<template>
  <n-config-provider :theme="null" :theme-overrides="themeOverrides">
    <n-dialog-provider>
      <router-view />
    </n-dialog-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { NConfigProvider, NDialogProvider } from 'naive-ui'
import type { GlobalThemeOverrides } from 'naive-ui'

/// Naive UI 主题覆盖（CSS 变量驱动的暗色适配）
const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#7C5CFF',
    primaryColorHover: '#6845F5',
    primaryColorPressed: '#5631E0',
    primaryColorSuppl: '#7C5CFF',
    infoColor: '#2563EB',
    successColor: '#22C55E',
    warningColor: '#F59E0B',
    errorColor: '#EF4444',
  },
}
</script>
