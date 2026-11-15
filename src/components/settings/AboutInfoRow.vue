<script setup lang="ts">
/// About 区信息行：dt 标签 + 等宽截断值 + 逐项复制按钮。
/// 复制反馈（「已复制」2 秒回弹）经 copiedKey/copyText 由 useAboutSettings 模型透传，
/// 行自身不持有状态。
defineProps<{
  label: string
  value: string
  /** 复制反馈的去重键（同时只有一个 key 显示「已复制」） */
  copyKey: string
  /** 悬停完整值：截断路径类字段（目录/日志路径）传入；版本号等短值省略 */
  title?: string
  copiedKey: string | null
  copyText: (key: string, text: string) => Promise<void>
}>()
</script>

<template>
  <div class="flex items-center gap-3 py-3">
    <dt class="w-24 shrink-0 text-xs font-medium text-[var(--text-secondary)]">
      {{ label }}
    </dt>
    <dd
      class="min-w-0 flex-1 truncate font-mono text-xs text-[var(--text-primary)]"
      :title="title"
    >
      {{ value }}
    </dd>
    <button
      type="button"
      class="shrink-0 rounded-md border border-[var(--border-color)] px-2 py-0.5 text-xs text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
      @click="copyText(copyKey, value)"
    >
      {{ copiedKey === copyKey ? '已复制' : '复制' }}
    </button>
  </div>
</template>
