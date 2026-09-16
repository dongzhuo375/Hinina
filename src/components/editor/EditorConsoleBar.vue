<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { submissionService } from '@/services/submission.service'
import { useContestStore } from '@/stores/contestStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import type { JudgementStatus, SubmissionRecord } from '@/types/submission'
import { findFirstFailedCase, formatMsToSeconds, isTerminalStatus, statusAbbr, statusTone } from '@/utils/submission'
import type { StatusTone } from '@/utils/submission'

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

const route = useRoute()
const router = useRouter()
const contestStore = useContestStore()
const submissionStore = useSubmissionStore()

/// 当前路由的比赛内展示题号（解题页 /contest/problem/:displayId）
const displayId = computed(() => String(route.params.displayId ?? ''))

/// 本次会话内当前题目的最新一条提交（store 只存本会话提交，取末尾匹配项）；
/// 无本地提交时为 null —— 此时优先展示服务端最新记录
const latest = computed(() => {
  if (!props.problemId) return null
  for (let i = submissionStore.submissions.length - 1; i >= 0; i--) {
    const s = submissionStore.submissions[i]
    if (s.problemId === props.problemId) return s
  }
  return null
})

// ── 服务端题目提交摘要（最新记录 pill + 提交记录计数） ──

const summary = ref<{ latest: SubmissionRecord | null; total: number } | null>(null)
/// 失败记录的测试点提示（如 "Test 4 · 2.01s"）；每条提交只拉一次测试点明细
const failedCaseHint = ref<string | null>(null)
let hintSubmitId: string | null = null

async function refreshSummary() {
  const contestId = contestStore.contest?.id
  if (!contestId || !displayId.value) return
  try {
    summary.value = await submissionStore.fetchProblemSummary(contestId, displayId.value)
  } catch (e) {
    // 摘要失败不影响解题：回退本地会话记录 pill，仅记录日志
    console.warn('[EditorConsoleBar] 获取题目提交摘要失败:', e)
  }
}

onMounted(() => {
  void refreshSummary()
})

/// 切题（Tab 切换 / 路由变化）时重置摘要并重新拉取
watch([displayId, () => props.problemId], () => {
  summary.value = null
  failedCaseHint.value = null
  hintSubmitId = null
  void refreshSummary()
})

/// 本地提交到达终态（store 轮询回填状态）后刷新服务端摘要，
/// 覆盖「本页提交收敛 → pill 换成服务端最新记录（含测试点提示）」的链路
watch(
  () => latest.value?.status,
  (status) => {
    if (status && isTerminalStatus(status)) void refreshSummary()
  },
)

/// 服务端最新记录为失败判定（WA/TLE 色系）时，拉一次测试点定位首个非 AC 测试点
watch(
  () => summary.value?.latest,
  async (record) => {
    if (!record) {
      failedCaseHint.value = null
      hintSubmitId = null
      return
    }
    if (hintSubmitId === record.submitId) return
    hintSubmitId = record.submitId
    failedCaseHint.value = null
    const tone = statusTone(record.status)
    if (tone !== 'wa' && tone !== 'tle') return
    try {
      const result = await submissionService.getSubmissionCases(record.submitId)
      // 子任务制下平铺 cases 常为空，helper 会展开 subTasks 查找，避免提示静默消失
      const first = findFirstFailedCase(result)
      if (first) failedCaseHint.value = `Test ${first.seq} · ${formatMsToSeconds(first.timeMs)}`
    } catch (e) {
      // 测试点不可得时回退记录自身耗时（下方 serverPill 兜底）
      console.warn('[EditorConsoleBar] 获取测试点明细失败:', e)
    }
  },
)

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

/// 服务端记录的语义色调 → 控制台条既有配色
const SERVER_TONE_MAP: Record<StatusTone, Tone> = {
  ac: 'success',
  wa: 'error',
  tle: 'warning',
  pending: 'pending',
  system: 'muted',
  neutral: 'muted',
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

/// 服务端最新记录 pill（优先于本地会话记录展示）：
/// `#123 WA (Test 4 · 2.01s)`；测试点不可得时回退 `#123 WA (1.20s)`
const serverPill = computed(() => {
  const record = summary.value?.latest
  if (!record) return null
  const semantic = statusTone(record.status)
  let detail = ''
  if (failedCaseHint.value && hintSubmitId === record.submitId) {
    detail = failedCaseHint.value
  } else if (record.timeMs > 0) {
    detail = semantic === 'wa' || semantic === 'tle'
      ? formatMsToSeconds(record.timeMs)
      : `${record.timeMs} ms`
  }
  return {
    submitId: record.submitId,
    abbr: statusAbbr(record.status),
    tone: SERVER_TONE_MAP[semantic] ?? 'muted',
    detail,
  }
})

const serverToneStyle = computed(() => TONE_STYLES[serverPill.value?.tone ?? 'muted'])

/// 本题提交总数（「提交记录 (n)」）；摘要未到达/为 0 时不显示计数
const totalCount = computed(() => summary.value?.total ?? 0)

function goSubmissions() {
  router.push({
    name: 'Submissions',
    query: displayId.value ? { problem: displayId.value } : undefined,
  })
}

function goDetail(submitId: string) {
  router.push({ name: 'SubmissionDetail', params: { submitId } })
}
</script>

<template>
  <div
    class="z-30 flex shrink-0 select-none flex-col gap-3 border-t border-slate-200 bg-white px-5 py-3 shadow-sm"
  >
    <div class="flex items-center justify-between gap-4">
      <!-- 左：最新评测记录 pill（优先服务端最新记录，其次本会话提交；均无则留空） -->
      <div class="flex min-w-0 items-center gap-3">
        <!-- 服务端最新记录：#123 WA (Test 4 · 2.01s)，点击进提交详情 -->
        <div
          v-if="serverPill"
          class="flex cursor-pointer items-center gap-2 rounded-full border px-3 py-1 font-mono text-[11px] font-semibold whitespace-nowrap transition-colors hover:brightness-[0.97]"
          :style="{ color: serverToneStyle.fg, backgroundColor: serverToneStyle.bg, borderColor: serverToneStyle.border }"
          title="点击查看提交详情"
          @click="goDetail(serverPill.submitId)"
        >
          <svg
            v-if="serverPill.tone === 'success'"
            class="h-3.5 w-3.5 shrink-0"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <svg
            v-else-if="serverPill.tone === 'error'"
            class="h-3.5 w-3.5 shrink-0"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <svg
            v-else-if="serverPill.tone === 'warning'"
            class="h-3.5 w-3.5 shrink-0"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <svg
            v-else-if="serverPill.tone === 'pending'"
            class="h-3.5 w-3.5 shrink-0 animate-spin"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
            <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
          </svg>
          <span>最新记录: #{{ serverPill.submitId }} {{ serverPill.abbr }}</span>
          <span v-if="serverPill.detail" class="font-mono opacity-70">({{ serverPill.detail }})</span>
        </div>
        <!-- 本会话提交（服务端摘要未到达时的回退） -->
        <div
          v-else-if="latest"
          class="flex cursor-pointer items-center gap-2 rounded-full border px-3 py-1 font-mono text-[11px] font-semibold whitespace-nowrap transition-colors hover:brightness-[0.97]"
          :style="{ color: toneStyle.fg, backgroundColor: toneStyle.bg, borderColor: toneStyle.border }"
          title="点击查看提交详情"
          @click="goDetail(latest.id)"
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
          title="查看当前题目的历史提交记录"
          class="flex items-center gap-1.5 rounded-lg border border-slate-200 bg-slate-50 px-3.5 py-1.5 text-xs font-medium whitespace-nowrap text-slate-700 shadow-sm transition-all duration-150 hover:bg-slate-100 active:scale-[0.98]"
          @click="goSubmissions"
        >
          <svg class="h-4 w-4 text-slate-500" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>提交记录{{ totalCount > 0 ? ` (${totalCount})` : '' }}</span>
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
