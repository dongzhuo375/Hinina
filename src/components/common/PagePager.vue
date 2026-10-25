<script setup lang="ts">
/**
 * 极简分页器（评测页 / 榜单页共用）。
 *
 * 设计语言：靠留白与线条分割，不画框 —— 没有任何边框与底色；
 * 「上一页 / 下一页」纯文字，深灰 → 悬停转黑；
 * 页码纯文本等宽数字，当前页加粗并带一根极细的木色下划线
 * （#8C6A43，仪表盘指针的隐喻，与页头「03 评测」的编号气质一致）。
 */

defineProps<{
  /// 当前页（1 起）
  current: number
  /// 总页数
  pages: number
  /// 页码窗口（首尾页 + 当前页 ±1，间隙为 '…'），由调用方计算
  items: readonly (number | '…')[]
  /// 翻页请求在途：禁用全部按钮，防止连点并发
  loading?: boolean
}>()

const emit = defineEmits<{ go: [page: number] }>()
</script>

<template>
  <nav class="page-pager flex items-center gap-1 text-xs" aria-label="分页">
    <button
      type="button"
      class="pager-nav"
      :disabled="current <= 1 || loading"
      @click="emit('go', current - 1)"
    >
      上一页
    </button>
    <template v-for="(item, index) in items" :key="item === '…' ? `gap-${index}` : item">
      <span v-if="item === '…'" class="px-1 text-[var(--text-muted)]">…</span>
      <button
        v-else
        type="button"
        class="pager-page font-mono"
        :class="{ current: item === current }"
        :aria-current="item === current ? 'page' : undefined"
        :disabled="loading"
        @click="emit('go', item)"
      >
        {{ item }}
      </button>
    </template>
    <button
      type="button"
      class="pager-nav"
      :disabled="current >= pages || loading"
      @click="emit('go', current + 1)"
    >
      下一页
    </button>
  </nav>
</template>

<style scoped>
.pager-nav {
  padding: 4px 6px;
  color: var(--text-secondary);
  transition: color 150ms ease-out;
}
.pager-nav:hover:not(:disabled) { color: var(--text-primary); }
.pager-nav:disabled { color: var(--text-muted); cursor: not-allowed; }

.pager-page {
  min-width: 26px;
  padding: 4px 2px 3px;
  color: var(--text-secondary);
  border-bottom: 1px solid transparent;
  transition: color 150ms ease-out, border-color 150ms ease-out;
}
.pager-page:hover:not(:disabled):not(.current) { color: var(--text-primary); }
.pager-page.current {
  color: var(--text-primary);
  font-weight: 700;
  border-bottom-color: #8c6a43;
}
.pager-page:disabled { cursor: default; }

.pager-nav:focus-visible,
.pager-page:focus-visible {
  outline: 1px solid #8c6a43;
  outline-offset: 2px;
}
</style>
