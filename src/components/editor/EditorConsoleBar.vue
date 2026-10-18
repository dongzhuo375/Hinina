<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { submissionService } from '@/services/submission.service'
import { useContestStore } from '@/stores/contestStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import type { JudgementStatus, SubmissionRecord } from '@/types/submission'
import { findFirstFailedCase, formatMsToSeconds, isTerminalStatus, statusAbbr, statusTone } from '@/utils/submission'
import type { StatusTone } from '@/utils/submission'
import { DEFAULT_EDITOR_TAB_SIZE } from '@/utils/editor'
import { createLogger } from '@/utils/logger'

const log = createLogger('EditorConsoleBar')

/// 编辑器底部控制台条：最新评测记录 pill + 提交入口 + 光标/编码状态行。
const props = withDefaults(
  defineProps<{
    /// Monaco 光标位置（由 CodeEditor 经父级转发；null 表示尚未产生光标事件）
    cursor: { line: number; column: number } | null
    /// 当前题目 pid，用于从会话提交列表中筛出「本题最新一条」
    problemId: string | null
    /// 编辑器当前 Tab 宽度（缩进状态行展示；由 CodeEditor 经父级转发，
    /// 解题页弹层可改，故不能按常量写死）
    tabSize?: number
    pendingSubmissionId?: string | null
  }>(),
  { tabSize: DEFAULT_EDITOR_TAB_SIZE, pendingSubmissionId: null },
)

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

/// 本地提交的失败原因首行（CE 编译错误 / SE / SF）。
///
/// 轮询把错误信息一并带回（`JudgementResult.errorMessage`），此处只取首行做条内提示
/// —— 完整多行编译错误在提交详情页展示。没有它，选手在解题页只看到「Compile Error」
/// 四个字，必须点进详情页才知道错在哪。
const failureDetail = computed(() => {
  const raw = latest.value?.errorMessage
  if (!raw) return null
  const firstLine = raw.split('\n').find((line) => line.trim() !== '') ?? raw
  return firstLine.trim() || null
})

// ── 服务端题目提交摘要（最新记录 pill + 提交记录计数） ──

const summary = ref<{ latest: SubmissionRecord | null; total: number } | null>(null)
/// 失败记录的测试点提示（如 "Test 4 · 2.01s"）；每条提交只拉一次测试点明细
const failedCaseHint = ref<string | null>(null)
let hintSubmitId: string | null = null
let summaryVersion = 0

async function refreshSummary() {
  const version = ++summaryVersion
  const contestId = contestStore.contest?.id
  if (!contestId || !displayId.value) return
  try {
    const result = await submissionStore.fetchProblemSummary(contestId, displayId.value)
    if (version === summaryVersion) summary.value = result
  } catch (e) {
    // 摘要失败不影响解题：回退本地会话记录 pill，仅记录日志
    log.warn('获取题目提交摘要失败:', e)
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
      if (first && hintSubmitId === record.submitId) failedCaseHint.value = `Test ${first.seq} · ${formatMsToSeconds(first.timeMs)}`
    } catch (e) {
      // 测试点不可得时回退记录自身耗时（下方 serverPill 兜底）
      log.warn('获取测试点明细失败:', e)
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
  success: { fg: 'var(--color-success)', bg: 'var(--color-ac-bg)', border: 'var(--color-ac-border)' },
  error: { fg: 'var(--color-error)', bg: 'var(--color-wa-bg)', border: 'var(--color-wa-border)' },
  warning: { fg: 'var(--color-warning)', bg: 'var(--color-pending-bg)', border: 'var(--color-pending-border)' },
  pending: { fg: 'var(--color-pending)', bg: 'var(--color-pending-bg)', border: 'var(--color-pending-border)' },
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

/// 服务端最新记录 pill（本地新提交尚未同步到摘要时，优先展示本地反馈）：
/// `#123 WA (Test 4 · 2.01s)`；测试点不可得时回退 `#123 WA (1.20s)`
const serverPill = computed(() => {
  const record = summary.value?.latest
  if (!record) return null
  // 本次提交立即接管反馈；旧摘要或同一提交的旧状态不能遮住本地轮询结果。
  const local = latest.value
  if (local && (!isTerminalStatus(local.status)
    || (record.submitId !== local.id && Date.parse(local.submittedAt) >= record.submitTime * 1000)
    || (record.submitId === local.id && !isTerminalStatus(record.status)))) return null
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
    class="editor-console z-30 flex shrink-0 select-none flex-col gap-3 border-t border-[var(--border-color)] bg-[var(--bg-card)] px-5 py-3 shadow-none"
  >
    <p v-if="submissionStore.isSubmitting || pendingSubmissionId" role="status" aria-live="polite" class="text-xs text-[var(--text-secondary)]">
      <template v-if="submissionStore.isSubmitting">正在提交代码，请稍候…</template>
      <template v-else>已提交 <span class="font-mono">#{{ pendingSubmissionId }}</span>，评测结果将自动更新。</template>
    </p>
    <div class="flex flex-wrap items-center justify-between gap-3">
      <!-- 左：最新评测记录（本次提交即时显示，摘要追上后补充测试点信息） -->
      <div class="flex min-w-0 items-center gap-3">
        <!-- 服务端最新记录：#123 WA (Test 4 · 2.01s)，点击进提交详情 -->
        <div
          v-if="serverPill"
          class="flex cursor-pointer items-center gap-2 rounded-sm border px-3 py-1 font-mono text-xs font-semibold whitespace-nowrap transition-colors hover:brightness-[0.97]"
          :style="{ color: serverToneStyle.fg, backgroundColor: serverToneStyle.bg, borderColor: serverToneStyle.border }"
          title="点击查看提交详情"
          role="button" tabindex="0" @keydown.enter="goDetail(serverPill.submitId)" @keydown.space.prevent="goDetail(serverPill.submitId)"
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
          <span v-if="serverPill.detail" class="font-mono">({{ serverPill.detail }})</span>
        </div>
        <!-- 本会话提交（服务端摘要未到达时的回退） -->
        <div
          v-else-if="latest"
          class="flex cursor-pointer items-center gap-2 rounded-sm border px-3 py-1 font-mono text-xs font-semibold whitespace-nowrap transition-colors hover:brightness-[0.97]"
          :style="{ color: toneStyle.fg, backgroundColor: toneStyle.bg, borderColor: toneStyle.border }"
          title="点击查看提交详情"
          role="button" tabindex="0" @keydown.enter="goDetail(latest.id)" @keydown.space.prevent="goDetail(latest.id)"
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
          <span v-if="timeLabel" class="font-mono">({{ timeLabel }})</span>
        </div>
        <!-- 本地提交的失败原因（CE 编译错误等，轮询回填；完整内容在详情页） -->
        <span
          v-if="failureDetail"
          class="max-w-[420px] truncate font-mono text-xs text-[var(--color-error)]"
          :title="latest?.errorMessage ?? ''"
        >
          {{ failureDetail }}
        </span>
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
          class="flex items-center gap-1.5 rounded-sm border border-[var(--border-color)] bg-[var(--bg-sidebar)] px-3.5 py-1.5 text-xs font-medium whitespace-nowrap text-[var(--text-primary)] shadow-none transition-all duration-150 hover:bg-[var(--bg-sidebar)] "
          @click="goSubmissions"
        >
          <svg class="h-4 w-4 text-[var(--text-secondary)]" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>提交记录<span v-if="totalCount > 0" class="font-mono"> ({{ totalCount }})</span></span>
        </button>
        <button
          type="button"
          class="flex items-center gap-2 rounded-sm bg-[var(--color-primary)] px-5 py-1.5 text-[13px] font-semibold whitespace-nowrap text-white shadow-none transition-all duration-150 hover:bg-[var(--color-primary-hover)] hover:shadow  disabled:cursor-not-allowed disabled:opacity-60"
          :disabled="submissionStore.isSubmitting || !!pendingSubmissionId"
          :aria-busy="submissionStore.isSubmitting || !!pendingSubmissionId"
          @click="$emit('submit')"
        >
          <svg
            v-if="submissionStore.isSubmitting || pendingSubmissionId"
            class="h-4 w-4 animate-spin"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
            <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
          </svg>
          <span class="tracking-wide">{{ submissionStore.isSubmitting ? '提交中…' : pendingSubmissionId ? '评测中…' : '提交代码' }}</span>
          <svg v-if="!submissionStore.isSubmitting && !pendingSubmissionId" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M6 12L3.269 3.125A59.769 59.769 0 0121.485 12 59.768 59.768 0 013.27 20.875L5.999 12zm0 0h7.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>
    </div>

    <!-- 状态行：光标位置 / 编码 / 缩进 + 快捷键提示。
         UTF-8：工作区文件由 Rust 后端以 UTF-8 落盘；Spaces：Monaco 建编辑器时
         未关 insertSpaces，缩进恒为空格，宽度取编辑器当前 tabSize（弹层可调，
         经 CodeEditor 上报后由父级转发，不写死） -->
    <div
      class="flex items-center justify-between border-t border-[var(--border-color)] pt-2 font-mono text-xs text-[var(--text-muted)] select-none"
    >
      <div class="flex items-center gap-2.5 whitespace-nowrap">
        <span>Ln {{ cursor?.line ?? 1 }}, Col {{ cursor?.column ?? 1 }}</span>
        <span class="text-[var(--text-muted)]">|</span>
        <span>UTF-8</span>
        <span class="text-[var(--text-muted)]">|</span>
        <span>Spaces: {{ tabSize }}</span>
      </div>
      <div class="flex items-center gap-1 whitespace-nowrap">
        <span>Ctrl + Enter 快捷提交</span>
      </div>
    </div>
  </div>
</template>
