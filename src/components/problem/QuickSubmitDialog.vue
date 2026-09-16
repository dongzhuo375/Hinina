<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import CodeEditor from '@/components/editor/CodeEditor.vue'
import { useContestStore } from '@/stores/contestStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import { useWorkspaceStore } from '@/stores/workspaceStore'
import { problemService } from '@/services/problem.service'
import type { ContestProblem } from '@/types/contest'
import {
  DEFAULT_LANGUAGE,
  DEFAULT_LANGUAGES,
  hojLanguageOfFileName,
  resolveAllowedLanguage,
  SOURCE_FILE_EXTENSIONS,
} from '@/utils/language'
import {
  formatMemoryKb,
  isTerminalStatus,
  statusLabel,
  statusTone,
} from '@/utils/submission'
import type { StatusTone } from '@/utils/submission'

/// 题目总览「快捷提交」弹窗：不进入解题页即可完成一次提交并跟踪评测结果。
const props = defineProps<{ problem: ContestProblem }>()
const emit = defineEmits<{ close: [] }>()

const router = useRouter()
const contestStore = useContestStore()
const submissionStore = useSubmissionStore()
const workspaceStore = useWorkspaceStore()

/// 本题允许的提交语言（HOJ 显示名）：挂载时从题目详情拉取，失败回退内置默认列表
const allowedLanguages = ref<string[]>([...DEFAULT_LANGUAGES])

const code = ref('')
/// 默认语言沿用工作区当前选择（与解题页习惯一致），兜底 C++
const language = ref(workspaceStore.language || DEFAULT_LANGUAGE)

async function loadAllowedLanguages() {
  const contestId = contestStore.contest?.id
  if (!contestId) return
  try {
    const detail = await problemService.getProblem(contestId, props.problem.displayId)
    if (detail.languages.length > 0) {
      allowedLanguages.value = detail.languages
      // 当前选择按语言族解析为列表中的服务端原名（"C++"→"C++17" 等变体归位）；
      // 无法解析（本题不允许该语言族）时切到列表首项，避免提交被服务端拒绝
      const resolved = resolveAllowedLanguage(language.value, allowedLanguages.value)
      language.value = resolved ?? allowedLanguages.value[0]
    }
  } catch (e) {
    // 语言列表不可得只影响下拉候选（回退默认列表），不阻断快捷提交
    console.warn('[QuickSubmitDialog] 获取题目允许语言失败，使用默认列表:', e)
  }
}

const submitError = ref<string | null>(null)
const fileError = ref<string | null>(null)

/// tone → pill 样式类（与评测页/详情页同一套语义色）
const TONE_CLASSES: Record<StatusTone, string> = {
  ac: 'border-emerald-200 bg-emerald-50 text-emerald-600',
  wa: 'border-rose-200 bg-rose-50 text-rose-600',
  tle: 'border-amber-200 bg-amber-50 text-amber-600',
  pending: 'border-cyan-200 bg-cyan-50 text-cyan-600',
  system: 'border-slate-200 bg-slate-100 text-slate-600',
  neutral: 'border-slate-200 bg-slate-100 text-slate-500',
}

// ── Esc 关闭 ──

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('close')
}
onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  void loadAllowedLanguages()
})
onUnmounted(() => window.removeEventListener('keydown', onKeydown))

// ── 文件导入（拖放 + 选择文件） ──

/// 与 CodeEditor 上传口径一致，另加 256KB 大小护栏（快捷提交场景无需超大文件）
const MAX_FILE_BYTES = 256 * 1024
/// 可导入扩展名白名单：从 utils/language 的识别面唯一来源派生（防多处清单漂移），另加 .txt
const ACCEPTED_EXTENSIONS: readonly string[] = [...SOURCE_FILE_EXTENSIONS, '.txt']
/// 文件选择器 accept 属性（与白名单同源）
const FILE_ACCEPT = ACCEPTED_EXTENSIONS.join(',')

const fileInput = ref<HTMLInputElement>()

function pickFile() {
  fileInput.value?.click()
}

async function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  // 立即重置，允许连续选择同一文件
  input.value = ''
  if (file) await ingestFile(file)
}

function extOf(name: string): string {
  const idx = name.lastIndexOf('.')
  return idx >= 0 ? name.slice(idx).toLowerCase() : ''
}

async function ingestFile(file: File) {
  const ext = extOf(file.name)
  if (!ACCEPTED_EXTENSIONS.includes(ext)) {
    fileError.value = `不支持的文件类型「${file.name}」，仅接受源代码文件或 .txt`
    return
  }
  if (file.size > MAX_FILE_BYTES) {
    fileError.value = `文件过大（${(file.size / 1024).toFixed(1)} KB），上限 256 KB`
    return
  }
  try {
    code.value = await file.text()
  } catch (e) {
    console.error('[QuickSubmitDialog] 读取文件失败:', e)
    fileError.value = `读取「${file.name}」失败`
    return
  }
  fileError.value = null
  // 按扩展名自动识别语言（.txt 保持当前选择）；经语言族解析为本题允许列表中的
  // 服务端原名（"Python3" 等变体也能命中）。无命中保持原选择 ——
  // 拖入 .py 但本题只允许 C++ 时，静默切过去只会换来一次提交失败
  const detected = hojLanguageOfFileName(file.name)
  if (detected) {
    const target = resolveAllowedLanguage(detected, allowedLanguages.value)
    if (target) language.value = target
  }
}

// HTML5 拖放（tauri.conf.json 已关闭 dragDropEnabled，事件才能到达 WebView）；
// 用进入/离开深度计数避免掠过子元素时高亮闪烁
const dragging = ref(false)
let dragDepth = 0

function onDragEnter(e: DragEvent) {
  e.preventDefault()
  dragDepth++
  dragging.value = true
}

function onDragOver(e: DragEvent) {
  e.preventDefault()
}

function onDragLeave(e: DragEvent) {
  e.preventDefault()
  dragDepth = Math.max(0, dragDepth - 1)
  if (dragDepth === 0) dragging.value = false
}

function onDrop(e: DragEvent) {
  e.preventDefault()
  dragDepth = 0
  dragging.value = false
  const file = e.dataTransfer?.files?.[0]
  if (file) void ingestFile(file)
}

// ── 提交与结果跟踪（结果轮询由 submissionStore 内部负责，此处只观察条目状态） ──

const submittedId = ref<string | null>(null)

const entry = computed(() => {
  const id = submittedId.value
  if (!id) return null
  return submissionStore.submissions.find((s) => s.id === id) ?? null
})

const entryTerminal = computed(() => (entry.value ? isTerminalStatus(entry.value.status) : false))

async function doSubmit() {
  if (submissionStore.isSubmitting) return
  submitError.value = null
  if (!code.value.trim()) {
    submitError.value = '代码不能为空'
    return
  }
  let contestId = contestStore.contest?.id
  if (!contestId) {
    try {
      await contestStore.whenLoaded()
      contestId = contestStore.contest?.id
    } catch {
      // 失败原因已写入 contestStore.error
    }
  }
  if (!contestId) {
    submitError.value = contestStore.error ?? '比赛数据未就绪，无法提交'
    return
  }
  try {
    submittedId.value = await submissionStore.submitCode(
      contestId,
      props.problem.problemId,
      language.value,
      code.value,
    )
  } catch {
    submitError.value = submissionStore.error ?? '提交失败'
  }
}

function goDetail() {
  if (!submittedId.value) return
  router.push({ name: 'SubmissionDetail', params: { submitId: submittedId.value } })
}
</script>

<template>
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4 backdrop-blur-[2px]"
    @click.self="emit('close')"
  >
    <div
      class="flex max-h-[92vh] w-[760px] max-w-full flex-col overflow-hidden rounded-xl border border-[var(--border-color)] bg-white shadow-2xl"
    >
      <!-- 头部 -->
      <div
        class="flex shrink-0 items-center justify-between gap-3 border-b border-[var(--border-color)] px-5 py-3.5"
      >
        <h2 class="min-w-0 truncate text-sm font-bold tracking-tight text-[var(--text-primary)]">
          快捷提交 ·
          <span class="font-mono text-[var(--color-primary)]">{{ problem.displayId }}</span>
          {{ problem.displayTitle || `Problem ${problem.displayId}` }}
        </h2>
        <div class="flex shrink-0 items-center gap-3">
          <!-- 语言选择 -->
          <div class="relative">
            <select
              v-model="language"
              class="cursor-pointer appearance-none rounded-lg border border-[var(--border-color)] bg-white py-1.5 pl-3 pr-8 font-mono text-xs font-medium text-[var(--text-primary)] shadow-xs transition-colors hover:bg-slate-50 focus:border-[var(--color-primary)] focus:outline-none"
            >
              <option v-for="lang in allowedLanguages" :key="lang" :value="lang">
                {{ lang }}
              </option>
            </select>
            <svg
              class="pointer-events-none absolute right-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--text-muted)]"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <path d="M6 9l6 6 6-6" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </div>
          <button
            type="button"
            title="关闭"
            class="flex h-7 w-7 items-center justify-center rounded-md text-[var(--text-muted)] transition-colors hover:bg-slate-100 hover:text-[var(--text-primary)]"
            @click="emit('close')"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
        </div>
      </div>

      <!-- 编辑器 + 拖放区 -->
      <div
        class="relative min-h-0 shrink-0 border-b border-[var(--border-color)]"
        @dragenter="onDragEnter"
        @dragover="onDragOver"
        @dragleave="onDragLeave"
        @drop="onDrop"
      >
        <div class="flex h-[45vh] min-h-[240px] flex-col overflow-hidden">
          <CodeEditor
            v-model="code"
            :language="language"
            :languages="allowedLanguages"
            :is-dirty="false"
            @update:language="language = $event"
            @submit="doSubmit"
          />
        </div>

        <!-- 拖放高亮层：拖拽进入后接管 drop，避免 Monaco 吞掉文件拖放事件 -->
        <div
          v-if="dragging"
          class="pointer-events-auto absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 border-2 border-dashed border-[var(--color-primary)] bg-[#f5f3ff]/90"
          @dragover="onDragOver"
          @drop="onDrop"
        >
          <svg class="h-8 w-8 text-[var(--color-primary)]" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <p class="text-xs font-medium text-[var(--color-primary)]">松开以导入代码文件</p>
          <p class="font-mono text-[11px] text-[var(--text-muted)]">.cpp / .c / .java / .py / .go / .rs 等源代码文件，≤ 256 KB</p>
        </div>
      </div>

      <!-- 底部：文件导入 / 错误 / 评测状态 / 提交 -->
      <div class="flex shrink-0 flex-col gap-2 px-5 py-3">
        <div class="flex items-center justify-between gap-3">
          <button
            type="button"
            class="flex items-center gap-1.5 rounded-lg border border-[var(--border-color)] bg-slate-50 px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] shadow-xs transition-colors hover:bg-slate-100"
            @click="pickFile"
          >
            <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <span>选择代码文件</span>
          </button>
          <span class="text-[11px] text-[var(--text-muted)]">
            也可直接把文件拖入编辑器区域 · Ctrl + Enter 提交
          </span>
          <input
            ref="fileInput"
            type="file"
            :accept="FILE_ACCEPT"
            class="hidden"
            @change="onFileChange"
          />
        </div>

        <p v-if="fileError" class="text-xs font-medium text-rose-600">{{ fileError }}</p>
        <p v-if="submitError" class="text-xs font-medium text-rose-600">{{ submitError }}</p>

        <!-- 提交后的评测状态行 -->
        <div
          v-if="entry"
          class="flex flex-wrap items-center gap-3 rounded-lg border border-[var(--border-color)] bg-slate-50/70 px-3.5 py-2.5"
        >
          <template v-if="!entryTerminal">
            <svg class="h-4 w-4 animate-spin text-cyan-500" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24" aria-hidden="true">
              <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
              <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
            </svg>
            <span class="text-xs font-medium text-[var(--text-secondary)]">
              评测中…（#{{ entry.id }}，结果将自动更新）
            </span>
          </template>
          <template v-else>
            <span
              class="inline-flex items-center rounded-full border px-2.5 py-1 text-xs font-semibold"
              :class="TONE_CLASSES[statusTone(entry.status)]"
            >
              {{ statusLabel(entry.status) }}
            </span>
            <span class="font-mono text-xs tabular-nums text-[var(--text-secondary)]">
              <template v-if="typeof entry.time === 'number' && entry.time > 0">{{ entry.time }} ms</template>
              <template v-else>-</template>
              ·
              <template v-if="typeof entry.memory === 'number' && entry.memory > 0">{{ formatMemoryKb(entry.memory) }}</template>
              <template v-else>-</template>
            </span>
            <button
              type="button"
              class="text-xs font-medium text-[var(--color-primary)] transition-colors hover:text-[var(--color-primary-hover)] hover:underline"
              @click="goDetail"
            >
              查看详情
            </button>
            <button
              type="button"
              class="ml-auto rounded-lg border border-[var(--border-color)] bg-white px-3.5 py-1.5 text-xs font-medium text-[var(--text-secondary)] shadow-xs transition-colors hover:bg-slate-50"
              @click="emit('close')"
            >
              关闭
            </button>
          </template>
        </div>

        <div class="flex items-center justify-end gap-2 pt-0.5">
          <button
            type="button"
            class="rounded-lg border border-[var(--border-color)] bg-white px-4 py-1.5 text-xs font-medium text-[var(--text-secondary)] shadow-xs transition-colors hover:bg-slate-50"
            @click="emit('close')"
          >
            取消
          </button>
          <button
            type="button"
            class="flex items-center gap-2 rounded-lg bg-emerald-600 px-5 py-1.5 text-[13px] font-semibold whitespace-nowrap text-white shadow-sm transition-all duration-150 hover:bg-emerald-700 hover:shadow active:scale-[0.98] disabled:cursor-not-allowed disabled:opacity-60"
            :disabled="submissionStore.isSubmitting"
            @click="doSubmit"
          >
            <svg
              v-if="submissionStore.isSubmitting"
              class="h-4 w-4 animate-spin"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <circle cx="12" cy="12" r="9" stroke-opacity="0.25" />
              <path d="M21 12a9 9 0 00-9-9" stroke-linecap="round" />
            </svg>
            <span>{{ submissionStore.isSubmitting ? '提交中…' : '提交代码' }}</span>
            <svg v-if="!submissionStore.isSubmitting" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true">
              <path d="M6 12L3.269 3.125A59.769 59.769 0 0121.485 12 59.768 59.768 0 013.27 20.875L5.999 12zm0 0h7.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
