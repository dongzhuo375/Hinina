<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { Problem, Sample } from '@/types/problem'
import type { ProblemLimits } from '@/types/rank'
import { renderMarkdown } from '@/utils/markdown'
import { configService } from '@/services/config.service'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { useWorkspaceStore } from '@/stores/workspaceStore'
import {
  effectiveLimits,
  formatMemoryLimit,
  formatTimeLimit,
  isDoubleLimitLanguage,
} from '@/utils/limits'

/// 题面视图：头部元信息（标题 + 限制 + 通过率）+ 分节滚动正文（描述/输入/输出/样例）。
const props = defineProps<{
  problem: Problem
  /// 比赛内展示题号（A/B/C…），用于 limits 查询与标题前缀
  displayId: string
}>()

const contestStore = useContestStore()
const problemStore = useProblemStore()
const workspaceStore = useWorkspaceStore()

/// OJ 服务端地址（用于把题目描述中的相对图片 URL 改写为绝对地址）
const baseUrl = ref('')

onMounted(async () => {
  // 配置服务内部已兜底：读取失败返回空串，相对路径按原样输出
  baseUrl.value = await configService.getOjBaseUrl()
})

// ── 头部元信息 ──

/// 本题在比赛题目表中的摘要（通过率 ac/total 来源）
const contestProblem = computed(() =>
  contestStore.problems.find((p) => p.displayId === props.displayId) ?? null,
)

/// C/C++ 基准 limits：优先取 limits API 的精确值，缺失时回退题目详情自带值；
/// 两者都没有则为 null，展示占位符 `—`（不留假默认值）
const baseLimits = computed<ProblemLimits | null>(() => {
  const fromApi = problemStore.limitsOf(props.displayId)
  if (fromApi) return fromApi
  const p = props.problem
  if (p.timeLimit > 0 || p.memoryLimit > 0) {
    return { displayId: props.displayId, timeLimit: p.timeLimit, memoryLimit: p.memoryLimit }
  }
  return null
})

/// HOJ 判题端对非 C/C++ 语言时间与内存 ×2（HOJ-Problem-Limits-API.md §5），
/// 头部展示的是**当前语言实际生效**的阈值，与判题行为一致
const isDouble = computed(() => isDoubleLimitLanguage(workspaceStore.language))
const shownLimits = computed(() =>
  baseLimits.value ? effectiveLimits(baseLimits.value, workspaceStore.language) : null,
)

const timeText = computed(() =>
  shownLimits.value ? formatTimeLimit(shownLimits.value.timeLimit) : '—',
)
const memoryText = computed(() =>
  shownLimits.value ? formatMemoryLimit(shownLimits.value.memoryLimit) : '—',
)
const baseTimeText = computed(() =>
  baseLimits.value ? formatTimeLimit(baseLimits.value.timeLimit) : '—',
)
const baseMemoryText = computed(() =>
  baseLimits.value ? formatMemoryLimit(baseLimits.value.memoryLimit) : '—',
)

const LANG_LABELS: Record<string, string> = { c: 'C', cpp: 'C++', java: 'Java', python: 'Python' }
const languageLabel = computed(
  () => LANG_LABELS[workspaceStore.language] ?? workspaceStore.language,
)

const acText = computed(() => {
  const cp = contestProblem.value
  return cp ? `${cp.ac} / ${cp.total}` : '—'
})

// ── 正文分节 ──

const renderedDescription = computed(() => renderMarkdown(props.problem.description, baseUrl.value))
const renderedInput = computed(() => renderMarkdown(props.problem.inputDescription, baseUrl.value))
const renderedOutput = computed(() => renderMarkdown(props.problem.outputDescription, baseUrl.value))

const samples = computed<Sample[]>(() => props.problem.samples ?? [])

// ── 样例复制反馈 ──

const copiedIndex = ref(-1)
let copyTimer: ReturnType<typeof setTimeout> | null = null

async function copySampleInput(index: number, text: string) {
  try {
    await navigator.clipboard.writeText(text)
    copiedIndex.value = index
    if (copyTimer) clearTimeout(copyTimer)
    copyTimer = setTimeout(() => (copiedIndex.value = -1), 1500)
  } catch (e) {
    // 剪贴板不可用（权限/环境）：不打断做题，仅记录
    console.error('[ProblemStatement] 复制样例输入失败:', e)
  }
}

onUnmounted(() => {
  if (copyTimer) clearTimeout(copyTimer)
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-white">
    <!-- 元信息头部 -->
    <div class="shrink-0 border-b border-slate-200 bg-slate-50/80 px-5 py-4">
      <h1 class="text-lg font-bold tracking-tight text-slate-900">
        {{ displayId }}. {{ problem.title }}
      </h1>
      <div class="mt-2.5 flex flex-wrap items-center gap-3.5 font-mono text-xs text-slate-500">
        <span class="flex items-center gap-1.5">
          <svg class="h-3.5 w-3.5 text-slate-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          时间限制: <span class="font-medium text-slate-700">{{ timeText }}</span>
        </span>
        <span class="text-slate-300">•</span>
        <span class="flex items-center gap-1.5">
          <svg class="h-3.5 w-3.5 text-slate-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          空间限制: <span class="font-medium text-slate-700">{{ memoryText }}</span>
        </span>
        <span class="text-slate-300">•</span>
        <span class="flex items-center gap-1.5">
          <svg class="h-3.5 w-3.5 text-slate-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          通过率: <span class="font-medium text-slate-700">{{ acText }}</span>
        </span>
      </div>
      <!-- 语言倍率说明：limits 是 C/C++ 基准，当前语言 ×2 时明确标注（HOJ-Problem-Limits-API.md §5） -->
      <p v-if="isDouble && shownLimits" class="mt-1.5 pl-5 text-[11px] text-slate-400">
        题面限制为 C/C++ 基准（{{ baseTimeText }} / {{ baseMemoryText }}），当前语言
        {{ languageLabel }} 判题时时间与内存 ×2
      </p>
    </div>

    <!-- 分节滚动正文 -->
    <div class="flex-1 space-y-6 overflow-y-auto bg-white p-5 text-slate-800">
      <!-- 题目描述 -->
      <section class="space-y-2">
        <div class="flex items-center space-x-2">
          <span class="h-4 w-1.5 rounded-full bg-[var(--color-primary)]"></span>
          <h2 class="text-[15px] font-semibold text-slate-900">题目描述 (Problem Description)</h2>
        </div>
        <div class="prose pl-3.5" v-html="renderedDescription"></div>
      </section>

      <!-- 输入格式 -->
      <section class="space-y-2">
        <div class="flex items-center space-x-2">
          <span class="h-4 w-1.5 rounded-full bg-[var(--color-primary)]"></span>
          <h2 class="text-[15px] font-semibold text-slate-900">输入格式 (Input Format)</h2>
        </div>
        <div class="prose pl-3.5" v-html="renderedInput"></div>
      </section>

      <!-- 输出格式 -->
      <section class="space-y-2">
        <div class="flex items-center space-x-2">
          <span class="h-4 w-1.5 rounded-full bg-[var(--color-primary)]"></span>
          <h2 class="text-[15px] font-semibold text-slate-900">输出格式 (Output Format)</h2>
        </div>
        <div class="prose pl-3.5" v-html="renderedOutput"></div>
      </section>

      <!-- 样例数据 -->
      <section v-if="samples.length > 0" class="space-y-3">
        <div class="flex items-center space-x-2">
          <span class="h-4 w-1.5 rounded-full bg-[var(--color-primary)]"></span>
          <h2 class="text-[15px] font-semibold text-slate-900">样例数据 (Sample Testcases)</h2>
        </div>
        <div
          v-for="(sample, i) in samples"
          :key="i"
          class="overflow-hidden rounded-lg border border-slate-200 bg-white shadow-sm"
        >
          <div class="flex items-center justify-between border-b border-slate-200 bg-slate-50 px-3 py-1.5">
            <span class="font-mono text-[11px] font-semibold text-slate-800">
              样例 {{ i + 1 }} (Sample #{{ i + 1 }})
            </span>
            <button
              type="button"
              class="flex items-center gap-1 font-mono text-[11px] transition-colors"
              :class="copiedIndex === i ? 'text-emerald-600' : 'text-slate-500 hover:text-[var(--color-primary)]'"
              @click="copySampleInput(i, sample.input)"
            >
              <svg v-if="copiedIndex !== i" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <svg v-else class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <span>{{ copiedIndex === i ? '已复制' : '复制输入' }}</span>
            </button>
          </div>
          <div class="grid grid-cols-1 divide-y divide-slate-200 md:grid-cols-2 md:divide-x md:divide-y-0">
            <div class="bg-slate-50/50 p-3">
              <div class="mb-1 font-mono text-[11px] text-slate-400">输入 (Input):</div>
              <pre class="sample-pre sample-input">{{ sample.input }}</pre>
            </div>
            <div class="bg-white p-3">
              <div class="mb-1 font-mono text-[11px] text-slate-400">输出 (Output):</div>
              <pre class="sample-pre sample-output">{{ sample.output }}</pre>
            </div>
          </div>
        </div>
      </section>

      <!-- 提示节：Problem 类型无 hint 字段，服务端未提供数据，按约定跳过 -->
    </div>
  </div>
</template>

<style scoped>
.prose {
  font-size: 13px;
  line-height: 1.7;
  color: #475569;
}
.prose :deep(p) {
  margin: 0 0 0.75rem;
}
.prose :deep(p:last-child) {
  margin-bottom: 0;
}
.prose :deep(ul) {
  list-style: disc;
  padding-left: 1.25rem;
  margin: 0 0 0.75rem;
}
.prose :deep(ol) {
  list-style: decimal;
  padding-left: 1.25rem;
  margin: 0 0 0.75rem;
}
.prose :deep(li) {
  margin-bottom: 0.25rem;
}
.prose :deep(strong) {
  color: #0f172a;
  font-weight: 600;
}
.prose :deep(pre) {
  background: var(--bg-sidebar);
  border-radius: var(--radius);
  padding: 0.75rem 1rem;
  font-size: 0.875rem;
  overflow-x: auto;
}
.prose :deep(code) {
  font-family: var(--font-mono);
  font-size: 0.875em;
  background: #f1f5f9;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  padding: 0.1rem 0.35rem;
  color: #334155;
}
.prose :deep(pre code) {
  background: transparent;
  border: none;
  padding: 0;
}
.prose :deep(img) {
  max-width: 100%;
  height: auto;
}
.prose :deep(table) {
  border-collapse: collapse;
  margin-bottom: 0.75rem;
}
.prose :deep(th),
.prose :deep(td) {
  border: 1px solid #e2e8f0;
  padding: 0.25rem 0.5rem;
}

/* 样例 I/O：等宽小字，横向滚动，选区色区分输入/输出 */
.sample-pre {
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.6;
  overflow-x: auto;
  white-space: pre;
  margin: 0;
}
.sample-input {
  color: #1e293b;
}
.sample-input::selection {
  background: #f3e8ff;
}
.sample-output {
  color: #059669;
  font-weight: 600;
}
.sample-output::selection {
  background: #d1fae5;
}
</style>
