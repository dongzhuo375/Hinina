<script setup lang="ts">
import { computed } from 'vue'
import SpinnerIcon from './SpinnerIcon.vue'

/// 「提交代码」三态按钮（快速提交对话框 / 解题页控制台条共用）：
/// 常态（纸飞机图标）→ 提交中（spinner）→ 已提交待评测（spinner，遮蔽再提交）。
/// 三态文案与 aria-busy 判据单点维护；底色经 tone 区分对话框（success）与控制台条（primary）。
const props = defineProps<{
  /** 提交请求进行中 */
  busy: boolean
  /** 已提交、等待评测结果（防重复提交遮蔽） */
  pending: boolean
  tone: 'success' | 'primary'
  /** 控制台条版本的文字加宽（tracking-wide），对话框版本不加 */
  tracking?: boolean
}>()
const emit = defineEmits<{ (event: 'submit'): void }>()

const working = computed(() => props.busy || props.pending)
</script>

<template>
  <button
    type="button"
    class="flex items-center gap-2 rounded-sm px-5 py-1.5 text-[13px] font-semibold whitespace-nowrap text-white shadow-none transition-all duration-150 hover:shadow disabled:cursor-not-allowed disabled:opacity-60"
    :class="
      tone === 'success'
        ? 'bg-[var(--color-success)] hover:bg-[var(--color-success)]'
        : 'bg-[var(--color-primary)] hover:bg-[var(--color-primary-hover)]'
    "
    :disabled="working"
    :aria-busy="working"
    @click="emit('submit')"
  >
    <SpinnerIcon v-if="working" class="h-4 w-4" />
    <span :class="tracking ? 'tracking-wide' : undefined">
      {{ busy ? '提交中…' : pending ? '评测中…' : '提交代码' }}
    </span>
    <svg
      v-if="!working"
      class="h-4 w-4"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      viewBox="0 0 24 24"
      aria-hidden="true"
    >
      <path d="M6 12L3.269 3.125A59.769 59.769 0 0121.485 12 59.768 59.768 0 013.27 20.875L5.999 12zm0 0h7.5" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>
</template>
