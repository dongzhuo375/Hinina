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
    primaryColor: '#18a058',
    primaryColorHover: '#36ad6a',
    primaryColorPressed: '#0c7a43',
  },
}
</script>
