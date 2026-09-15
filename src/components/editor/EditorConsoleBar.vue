<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useSubmissionStore } from '@/stores/submissionStore'
import type { JudgementStatus } from '@/types/submission'

/// 编辑器底部控制台条：最新评测记录 pill + 提交入口 + 光标/编码状态行。
const props = defineProps<{
  /// Monaco 光标位置（由 CodeEditor 经父级转发；null 表示尚未产生光标事件）
  cursor: { line: number; column: number } | null
  /// 当前题目 pid，用于从会话提交列表中筛出「本题最新一条」
  problemId: string | null
}>()

defineEmits<{
  submit: []
}>()

const router = useRouter()
const submissionStore = useSubmissionStore()

/// 本次会话内当前题目的最新一条提交（store 只存本会话提交，取末尾匹配项）；
/// 无本地提交时为 null —— 该区域留空，不显示假数据
const latest = computed(() => {
  if (!props.problemId) return null
  for (let i = submissionStore.submissions.length - 1; i >= 0; i--) {
    const s = submissionStore.submissions[i]
    if (s.problemId === props.problemId) return s
  }
  return null
})

/// 状态 → 语义色类别（映射 global.css 的 --color-success/error/warning/pending）
type Tone = 'success' | 'error' | 'warning' | 'pending' | 'muted'

function toneOf(status: JudgementStatus): Tone {
  switch (status) {
    case 'Accepted':
      return 'success'
    case 'TimeLimitExceeded':
    case 'MemoryLimitExceeded':
      return 'warning'
    case 'Pending':
    case 'Compiling':
    case 'Running':
      return 'pending'
    case 'Unknown':
      return 'muted'
    default:
      // WrongAnswer / RuntimeError / CompilationError
      return 'error'
  }
}

const TONE_STYLES: Record<Tone, { fg: string; bg: string; border: string }> = {
  success: { fg: 'var(--color-success)', bg: '#f0fdf4', border: '#bbf7d0' },
  error: { fg: 'var(--color-error)', bg: '#fff1f2', border: '#fecdd3' },
  warning: { fg: 'var(--color-warning)', bg: '#fffbeb', border: '#fde68a' },
  pending: { fg: 'var(--color-pending)', bg: '#ecfeff', border: '#a5f3fc' },
  muted: { fg: 'var(--text-secondary)', bg: 'var(--bg-sidebar)', border: 'var(--border-color)' },
}

const toneStyle = computed(() => TONE_STYLES[latest.value ? toneOf(latest.value.status) : 'muted'])

/// 状态文案用后端 JudgementStatus 原词，仅把驼峰拆成空格分隔（WrongAnswer → Wrong Answer），
/// 不缩写成 AC/WA —— 与判题语义一一对应，避免歧义
function statusLabel(status: JudgementStatus): string {
  return status.replace(/([a-z])([A-Z])/g, '$1 $2')
}

/// 运行耗时（轮询回填的 timeMs）；未到达终态时无值
const timeLabel = computed(() => {
  const t = latest.value?.time
  return typeof t === 'number' && t > 0 ? `${t} ms` : ''
})

function goSubmissions() {
  router.push({ name: 'Submissions' })
}
</script>

<template>
  <div
    class="z-30 flex shrink-0 select-none flex-col gap-3 border-t border-slate-200 bg-white px-5 py-3 shadow-sm"
  >
    <div class="flex items-center justify-between gap-4">
      <!-- 左：最新评测记录 pill（无本地提交时留空） -->
      <div class="flex min-w-0 items-center gap-3">
        <div
          v-if="latest"
          class="flex items-center gap-2 rounded-full border px-3 py-1 font-mono text-[11px] font-semibold whitespace-nowrap transition-colors"
          :style="{ color: toneStyle.fg, backgroundColor: toneStyle.bg, borderColor: toneStyle.border }"
        >
          <!-- 状态图标：终态勾/叉/警告，非终态旋转圈 -->
          <svg
            v-if="toneOf(latest.status) === 'success'"
            class="h-3.5 w-3.5 shrink-0"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <svg
            v-else-if="toneOf(latest.status) === 'error'"
            class="h-3.5 w-3.5 shrink-0"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <svg
            v-else-if="toneOf(latest.status) === 'warning'"
            class="h-3.5 w-3.5 shrink-0"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <svg v-else class="h-3.5 w-3.5 shrink-0 animate-spin" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
            <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
          </svg>
          <span>最新记录: #{{ latest.id }} {{ statusLabel(latest.status) }}</span>
          <span v-if="timeLabel" class="font-mono opacity-70">({{ timeLabel }})</span>
        </div>
        <!-- 提交失败信息（store.error 已由 submitCode/轮询写入） -->
        <span
          v-if="submissionStore.error"
          class="truncate text-xs text-[var(--color-error)]"
          :title="submissionStore.error"
        >
          {{ submissionStore.error }}
        </span>
      </div>

      <!-- 右：提交记录 + 提交代码 -->
      <div class="flex shrink-0 items-center gap-3">
        <button
          type="button"
          title="查看提交记录"
          class="flex items-center gap-1.5 rounded-lg border border-slate-200 bg-slate-50 px-3.5 py-1.5 text-xs font-medium whitespace-nowrap text-slate-700 shadow-sm transition-all duration-150 hover:bg-slate-100 active:scale-[0.98]"
          @click="goSubmissions"
        >
          <svg class="h-4 w-4 text-slate-500" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>提交记录</span>
        </button>
        <button
          type="button"
          class="flex items-center gap-2 rounded-lg bg-emerald-600 px-5 py-1.5 text-[13px] font-semibold whitespace-nowrap text-white shadow-sm transition-all duration-150 hover:bg-emerald-700 hover:shadow active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="submissionStore.isSubmitting"
          @click="$emit('submit')"
        >
          <svg
            v-if="submissionStore.isSubmitting"
            class="h-4 w-4 animate-spin"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
            <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
          </svg>
          <span class="tracking-wide">{{ submissionStore.isSubmitting ? '提交中…' : '提交代码' }}</span>
          <svg v-if="!submissionStore.isSubmitting" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M6 12L3.269 3.125A59.769 59.769 0 0121.485 12 59.768 59.768 0 013.27 20.875L5.999 12zm0 0h7.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>
    </div>

    <!-- 状态行：光标位置 / 编码 / 缩进 + 快捷键提示。
         UTF-8：工作区文件由 Rust 后端以 UTF-8 落盘；Spaces: 4：Monaco 建编辑器时
         tabSize=4 且未开启 insertSpaces=false，二者均为固定事实，故按常量展示 -->
    <div
      class="flex items-center justify-between border-t border-slate-100 pt-2 font-mono text-[11px] text-slate-400 select-none"
    >
      <div class="flex items-center gap-2.5 whitespace-nowrap">
        <span>Ln {{ cursor?.line ?? 1 }}, Col {{ cursor?.column ?? 1 }}</span>
        <span class="text-slate-300">|</span>
        <span>UTF-8</span>
        <span class="text-slate-300">|</span>
        <span>Spaces: 4</span>
      </div>
      <div class="flex items-center gap-1 whitespace-nowrap">
        <span>Ctrl + Enter 快捷提交</span>
      </div>
    </div>
  </div>
</template>
