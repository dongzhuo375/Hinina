<script setup lang="ts">
import SettingsSectionFrame from './SettingsSectionFrame.vue'
import AboutInfoRow from './AboutInfoRow.vue'
import { useAboutSettings } from '@/composables/useAboutSettings'

const props = defineProps<{ model: ReturnType<typeof useAboutSettings> }>()
const { storage, storageFailed, copiedKey, copyText } = props.model

/// 项目仓库入口：模板内唯一的模板级外链锚点。点击经 document 级外链拦截器
/// （`utils/external-links`）转交系统浏览器打开；锚点带 `target="_blank"` +
/// `rel="noopener noreferrer"` 兜底——拦截器失效时 wry 拒新窗口，点击无
/// 动作而非整页导航走（与 markdown 渲染面的消毒钩子同款取舍）。
const REPO_SLUG = 'dongzhuo375/Hinina'
const REPO_URL = `https://github.com/${REPO_SLUG}`
</script>

<template>
  <SettingsSectionFrame id="settings-6" title="关于">
    <template #icon>
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v5m0-8h.01" stroke-linecap="round" />
    </template>
    <dl class="divide-y divide-[var(--border-color)] px-5">
      <!-- 行序是测试契约：版本行必须是最早出现的含按钮行
           （SettingsView.behavior.spec 取 About 区第一个 button 断言复制反馈） -->
      <template v-if="storage">
        <AboutInfoRow
          label="客户端版本"
          copy-key="version"
          :value="storage.version"
          :copied-key="copiedKey"
          :copy-text="copyText"
        />
        <AboutInfoRow
          label="存储目录"
          copy-key="baseDir"
          :value="storage.baseDir"
          :title="storage.baseDir"
          :copied-key="copiedKey"
          :copy-text="copyText"
        />
        <AboutInfoRow
          label="日志文件"
          copy-key="logPath"
          :value="storage.logPath"
          :title="storage.logPath"
          :copied-key="copiedKey"
          :copy-text="copyText"
        />
      </template>
      <!-- 读取中/失败回退行：无复制按钮，与 AboutInfoRow 形态不同，保持手写 -->
      <div v-else class="flex items-center gap-3 py-3">
        <dt class="w-24 shrink-0 text-xs font-medium text-[var(--text-secondary)]">
          存储信息
        </dt>
        <dd
          class="flex-1 text-xs"
          :class="storageFailed ? 'text-[var(--color-error)]' : 'text-[var(--text-muted)]'"
        >
          {{ storageFailed ? '获取失败' : '正在获取…' }}
        </dd>
      </div>
      <div class="flex items-center gap-3 py-3">
        <dt class="w-24 shrink-0 text-xs font-medium text-[var(--text-secondary)]">项目仓库</dt>
        <dd class="flex min-w-0 flex-1 items-center">
          <a
            :href="REPO_URL"
            target="_blank"
            rel="noopener noreferrer"
            class="inline-flex items-center gap-1.5 text-xs text-[var(--color-primary)] transition-colors hover:text-[var(--color-primary-hover)] hover:underline"
          >
            <svg class="h-4 w-4 shrink-0" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8z"/></svg>
            <span class="font-mono">{{ REPO_SLUG }}</span>
          </a>
        </dd>
      </div>
    </dl>
  </SettingsSectionFrame>
</template>
