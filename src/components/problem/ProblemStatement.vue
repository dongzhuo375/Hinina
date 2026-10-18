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
import { createLogger } from '@/utils/logger'

const log = createLogger('ProblemStatement')

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
/// 语言权威值已是 HOJ 显示名（"C++"），直接展示；历史 Monaco id 经映射表兜底
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
    log.error('复制样例输入失败:', e)
  }
}

onUnmounted(() => {
  if (copyTimer) clearTimeout(copyTimer)
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <!-- 元信息头部：题号微标 + 标题 + 仪器数据行；右侧巨型题号水印作视觉锚点 -->
    <div class="relative shrink-0 overflow-hidden border-b border-[var(--border-color)] px-6 py-5">
      <span
        class="pointer-events-none absolute -top-3 right-5 select-none font-mono text-[104px] font-bold leading-none text-[#2e3033]/[0.06]"
        aria-hidden="true"
      >
        {{ displayId }}
      </span>
      <div class="relative">
        <p class="font-mono text-[10px] tracking-[0.35em] text-[#8c6a43]">PROBLEM {{ displayId }}</p>
        <h1 class="mt-1.5 text-lg font-semibold tracking-wide text-[var(--text-primary)]">
          {{ problem.title }}
        </h1>
      <div class="mt-3 flex flex-wrap items-center gap-x-5 gap-y-1.5 font-mono text-xs text-[var(--text-secondary)]">
        <span class="flex items-baseline gap-1.5">
          <span class="text-[9px] tracking-[0.25em] text-[var(--text-muted)]">TIME</span>
          <span class="font-medium tabular-nums text-[var(--text-primary)]">{{ timeText }}</span>
        </span>
        <span class="h-3 w-px bg-[var(--border-color)]"></span>
        <span class="flex items-baseline gap-1.5">
          <span class="text-[9px] tracking-[0.25em] text-[var(--text-muted)]">MEMORY</span>
          <span class="font-medium tabular-nums text-[var(--text-primary)]">{{ memoryText }}</span>
        </span>
        <span class="h-3 w-px bg-[var(--border-color)]"></span>
        <span class="flex items-baseline gap-1.5">
          <span class="text-[9px] tracking-[0.25em] text-[var(--text-muted)]">SOLVED</span>
          <span class="font-medium tabular-nums text-[var(--text-primary)]">{{ acText }}</span>
        </span>
      </div>
      <!-- 语言倍率说明：limits 是 C/C++ 基准，当前语言 ×2 时明确标注（HOJ-Problem-Limits-API.md §5） -->
      <p v-if="isDouble && shownLimits" class="mt-2 text-[11px] text-[var(--text-muted)]">
        题面限制为 C/C++ 基准（{{ baseTimeText }} / {{ baseMemoryText }}），当前语言
        {{ languageLabel }} 判题时时间与内存 ×2
      </p>
      </div>
    </div>

    <!-- 分节滚动正文：编号章节 + 发丝线，正文限宽保证阅读度量 -->
    <div class="flex-1 space-y-10 overflow-y-auto px-6 py-6 text-[var(--text-primary)]">
      <!-- 题目描述 -->
      <section>
        <div class="flex items-baseline gap-3">
          <span class="font-mono text-[11px] font-semibold tracking-[0.2em] text-[#8c6a43]">01</span>
          <h2 class="text-[15px] font-semibold text-[var(--text-primary)]">题目描述</h2>
          <span class="font-mono text-[9px] tracking-[0.3em] text-[var(--text-muted)]">DESCRIPTION</span>
          <span class="h-px flex-1 self-center bg-[var(--border-color)]"></span>
        </div>
        <div class="prose mt-3 max-w-[68ch]" v-html="renderedDescription"></div>
      </section>

      <!-- 输入格式 -->
      <section>
        <div class="flex items-baseline gap-3">
          <span class="font-mono text-[11px] font-semibold tracking-[0.2em] text-[#8c6a43]">02</span>
          <h2 class="text-[15px] font-semibold text-[var(--text-primary)]">输入格式</h2>
          <span class="font-mono text-[9px] tracking-[0.3em] text-[var(--text-muted)]">INPUT</span>
          <span class="h-px flex-1 self-center bg-[var(--border-color)]"></span>
        </div>
        <div class="prose mt-3 max-w-[68ch]" v-html="renderedInput"></div>
      </section>

      <!-- 输出格式 -->
      <section>
        <div class="flex items-baseline gap-3">
          <span class="font-mono text-[11px] font-semibold tracking-[0.2em] text-[#8c6a43]">03</span>
          <h2 class="text-[15px] font-semibold text-[var(--text-primary)]">输出格式</h2>
          <span class="font-mono text-[9px] tracking-[0.3em] text-[var(--text-muted)]">OUTPUT</span>
          <span class="h-px flex-1 self-center bg-[var(--border-color)]"></span>
        </div>
        <div class="prose mt-3 max-w-[68ch]" v-html="renderedOutput"></div>
      </section>

      <!-- 样例数据 -->
      <section v-if="samples.length > 0">
        <div class="flex items-baseline gap-3">
          <span class="font-mono text-[11px] font-semibold tracking-[0.2em] text-[#8c6a43]">04</span>
          <h2 class="text-[15px] font-semibold text-[var(--text-primary)]">样例数据</h2>
          <span class="font-mono text-[9px] tracking-[0.3em] text-[var(--text-muted)]">SAMPLES</span>
          <span class="h-px flex-1 self-center bg-[var(--border-color)]"></span>
        </div>
        <div class="mt-4 space-y-4">
          <div
            v-for="(sample, i) in samples"
            :key="i"
            class="overflow-hidden border border-[var(--border-color)] bg-[var(--bg-card)]"
          >
            <div class="flex items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-body)] px-3.5 py-2">
              <span class="font-mono text-[11px] font-semibold tracking-[0.1em] text-[var(--text-primary)]">
                SAMPLE #{{ i + 1 }}
              </span>
              <button
                type="button"
                class="flex items-center gap-1 font-mono text-[11px] transition-colors duration-200 ease-out"
                :class="copiedIndex === i ? 'text-[#5f7a6a]' : 'text-[var(--text-secondary)] hover:text-[#8c6a43]'"
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
            <div class="grid grid-cols-1 divide-y divide-[var(--border-color)] md:grid-cols-2 md:divide-x md:divide-y-0">
              <div class="bg-[var(--bg-body)] p-3.5">
                <div class="mb-1.5 font-mono text-[9px] tracking-[0.25em] text-[var(--text-muted)]">INPUT</div>
                <pre class="sample-pre sample-input">{{ sample.input }}</pre>
              </div>
              <div class="bg-[var(--bg-card)] p-3.5">
                <div class="mb-1.5 font-mono text-[9px] tracking-[0.25em] text-[var(--text-muted)]">OUTPUT</div>
                <pre class="sample-pre sample-output">{{ sample.output }}</pre>
              </div>
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
  color: var(--text-secondary);
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
  color: var(--text-primary);
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
  background: #efede5;
  border: 1px solid #e2e0d6;
  border-radius: 3px;
  padding: 0.1rem 0.35rem;
  color: #2e3033;
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
  border: 1px solid #e2e0d6;
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
  color: #2e3033;
}
.sample-input::selection {
  background: #ece4d4;
}
.sample-output {
  color: #5f7a6a;
  font-weight: 600;
}
.sample-output::selection {
  background: #dfe7e0;
}
</style>
