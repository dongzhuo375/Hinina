<script setup lang="ts">
/// 统一页首版式：英文 micro-label（木色、大间距）+ 中文主标题 + 可选元信息行，
/// 底部发丝线分隔。右侧 slot 放置统计/操作区。
///
/// 各视图页首一律走本组件，不再自排标题行 —— 保证「编辑版式页首 + 仪器密度
/// 数据区」的整体节奏一致。
defineProps<{
  /// 英文 micro-label（大写字距拉开），如 'PROBLEMSET'
  en: string
  /// 中文主标题
  title: string
  /// 元信息行（如「共 12 题 · ACM 赛制」），缺省不占位
  meta?: string
}>()
</script>

<template>
  <header class="shrink-0 border-b border-[var(--border-color)] bg-[var(--bg-card)] px-6 pb-4 pt-5">
    <div class="flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
      <div class="min-w-0">
        <p class="font-mono text-[10px] tracking-[0.35em] text-[#8c6a43]">{{ en }}</p>
        <h1 class="mt-1.5 truncate text-xl font-semibold tracking-wide text-[var(--text-primary)]">
          {{ title }}
        </h1>
        <p v-if="meta" class="mt-1.5 text-xs text-[var(--text-secondary)]">{{ meta }}</p>
      </div>
      <div v-if="$slots.default" class="flex shrink-0 items-center gap-3">
        <slot />
      </div>
    </div>
  </header>
</template>
