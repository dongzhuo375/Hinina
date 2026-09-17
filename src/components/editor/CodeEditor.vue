<script setup lang="ts">
import { ref, reactive, onMounted, onUnmounted, watch, nextTick, shallowRef, computed } from 'vue'
import * as monaco from 'monaco-editor'
import EditorSettingsPopover from '@/components/editor/EditorSettingsPopover.vue'
import { configService } from '@/services/config.service'
import type { EditorPrefs } from '@/services/config.service'
import { DEFAULT_LANGUAGES, hojLanguageOfFileName, monacoIdOf, resolveAllowedLanguage, SOURCE_FILE_EXTENSIONS } from '@/utils/language'
import { DEFAULT_EDITOR_FONT_SIZE, DEFAULT_EDITOR_TAB_SIZE, DEFAULT_EDITOR_THEME } from '@/utils/editor'
import { createLogger } from '@/utils/logger'

// ── Monaco Editor Workers（手动配置，避免 worker 打包问题） ──
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker'
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker'
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker'
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker'

const log = createLogger('CodeEditor')

self.MonacoEnvironment = {
  getWorker(_: string, label: string) {
    if (label === 'json') return new jsonWorker()
    if (label === 'css' || label === 'scss' || label === 'less') return new cssWorker()
    if (label === 'html' || label === 'handlebars' || label === 'razor') return new htmlWorker()
    if (label === 'typescript' || label === 'javascript') return new tsWorker()
    return new editorWorker()
  },
}

const props = defineProps<{
  modelValue: string
  /// 当前语言 —— HOJ 显示名（"C++" 等，权威值域见 utils/language）
  language: string
  /// 工作区脏状态，驱动工具条右侧的自动备份指示
  isDirty: boolean
  /// 只读模式（提交详情页代码查看）：隐藏工具条、禁用编辑与提交快捷键
  readonly?: boolean
  /// 本题允许的提交语言（来自题目详情 languages）；空/缺省回退内置默认列表
  languages?: string[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'update:language': [language: string]
  submit: []
  /// 光标位置（供底部状态行显示 Ln/Col）。选择 emit 而非 provide/inject：
  /// 保持单向数据流，CodeEditor 不感知消费方，父级显式转发给 EditorConsoleBar
  cursor: [pos: { line: number; column: number }]
  /// 编辑器偏好（挂载读取配置后 + 每次弹层改动后上报）：父级据此同步状态行
  /// 的缩进宽度等派生展示，无需自己再读一次配置
  'prefs-change': [prefs: EditorPrefs]
}>()

const editorContainer = ref<HTMLDivElement>()
const editor = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null)

/// 语言下拉候选：以**题目详情返回的允许列表**为准（HOJ 比赛按题限制语言）。
/// 服务端列表存在时**原样呈现、不补入当前语言** —— 列表外语言（如工作区遗留的
/// 不允许语言）可见可选只会换来一次被拒的提交；归位由 ProblemSolveView 在题目
/// 加载后统一做（resolveAllowedLanguage → changeLanguage）。
/// 仅当列表未提供（未加载完成/服务端未返回）时回退内置默认并补入当前语言保持可见。
const availableLanguages = computed<string[]>(() => {
  if (props.languages?.length) return [...props.languages]
  const list = [...DEFAULT_LANGUAGES]
  if (props.language && !resolveAllowedLanguage(props.language, list)) list.unshift(props.language)
  return list
})

/// 文件选择器 accept 属性（从识别面唯一来源派生，另加 .txt 纯文本）
const fileAccept = computed(() => [...SOURCE_FILE_EXTENSIONS, '.txt'].join(','))

/// Ctrl+Enter 提交
function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault()
    emit('submit')
  }
}

/// 外部改写代码（切题加载 / 清空 / 上传文件）时同步进编辑器；
/// 抑制 setValue 触发的 change 回流，避免把程序化写入误标为「用户编辑（dirty）」
let suppressChangeEmit = false

watch(
  () => props.modelValue,
  (val) => {
    const ed = editor.value
    if (ed && val !== ed.getValue()) {
      suppressChangeEmit = true
      ed.setValue(val)
      suppressChangeEmit = false
    }
  },
)

onMounted(async () => {
  await nextTick()
  if (!editorContainer.value) return

  // 编辑器偏好来自应用配置（设置页 / 编辑器设置弹层可调）；读取失败时服务内部已回退兜底值。
  // 工具栏先于编辑器实例渲染：逐字段并入，已触碰字段保持用户当前值，
  // 未触碰字段采纳磁盘真值（整块覆盖会把存量的字号/主题抹成默认值）
  applyLoadedPrefs(await configService.getEditorPrefs())
  emit('prefs-change', { ...prefs })

  const ed = monaco.editor.create(editorContainer.value, {
    value: props.modelValue,
    language: monacoIdOf(props.language),
    theme: prefs.editorTheme,
    fontSize: prefs.fontSize,
    fontFamily: 'JetBrains Mono, Cascadia Code, Consolas, monospace',
    minimap: { enabled: false },
    lineNumbers: 'on',
    scrollBeyondLastLine: false,
    automaticLayout: true,
    tabSize: prefs.tabSize,
    wordWrap: 'on',
    padding: { top: 12, bottom: 12 },
    readOnly: props.readonly === true,
    // 只读查看无需行内建议/高亮干扰
    renderLineHighlight: props.readonly ? 'none' : 'line',
  })

  ed.onDidChangeModelContent(() => {
    if (suppressChangeEmit) return
    emit('update:modelValue', ed.getValue())
  })

  ed.onDidChangeCursorPosition((e) => {
    emit('cursor', { line: e.position.lineNumber, column: e.position.column })
  })

  editor.value = ed

  // Ctrl+Enter 提交快捷键（只读模式无提交语义，不注册）
  if (!props.readonly) window.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  editor.value?.dispose()
  window.removeEventListener('keydown', handleKeydown)
  // 关闭前把在途的偏好改动补一次落盘（debounce 窗口内离页不该丢设置）
  if (persistTimer) {
    clearTimeout(persistTimer)
    persistTimer = null
    void persistPrefs()
  }
})

watch(
  () => props.language,
  (lang) => {
    const model = editor.value?.getModel()
    if (model) {
      monaco.editor.setModelLanguage(model, monacoIdOf(lang))
    }
  },
)

// ── 语言下拉 ──

const langMenuOpen = ref(false)

function selectLanguage(value: string) {
  langMenuOpen.value = false
  if (value !== props.language) emit('update:language', value)
}

// ── 清空代码（二次确认，避免误删） ──

const confirmClearOpen = ref(false)

function confirmClear() {
  confirmClearOpen.value = false
  emit('update:modelValue', '')
  editor.value?.focus()
}

// ── 上传代码（原生 input + FileReader，不引入 Tauri dialog 插件） ──

const fileInput = ref<HTMLInputElement>()

function pickFile() {
  fileInput.value?.click()
}

async function handleFileChange(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  // 立即重置，允许连续选择同一文件
  input.value = ''
  if (!file) return
  try {
    const text = await file.text()
    emit('update:modelValue', text)
    // 按扩展名自动识别语言（上传 .java 就该切到 Java）；经语言族解析为
    // 本题允许列表中的服务端原名（"Python3" 等变体也能命中），
    // 不在允许列表内则不切换 —— 切到不允许的语言只会换来一次提交失败
    const detected = hojLanguageOfFileName(file.name)
    if (detected) {
      const target = resolveAllowedLanguage(detected, availableLanguages.value)
      if (target && target !== props.language) emit('update:language', target)
    }
    editor.value?.focus()
  } catch (err) {
    log.error('读取上传文件失败:', err)
  }
}

// ── 编辑器设置（字号 / Tab 宽度 / 主题） ──
//
// 偏好是**应用级配置**（与设置页「编辑器」分组同一份数据），因此改动走
// configService.updateEditorPrefs 落盘，而非仅作用于本实例。落盘前先即时应用到
// Monaco —— 赛场调字号是「边看边调」，等 IPC 回来才生效是不可接受的交互延迟。

/// 当前偏好：挂载时读配置，弹层改动就地更新（不重读配置，避免与在途写入打架）
const prefs = reactive<EditorPrefs>({
  fontSize: DEFAULT_EDITOR_FONT_SIZE,
  tabSize: DEFAULT_EDITOR_TAB_SIZE,
  editorTheme: DEFAULT_EDITOR_THEME,
})

const prefsError = ref<string | null>(null)

/// 编辑器容器底色跟随主题：Monaco 实例创建前与尺寸重算的瞬间不露白底
/// （背景本身由 Monaco 主题绘制，此处只是同色兜底）
const editorSurfaceClass = computed(() =>
  prefs.editorTheme === 'vs-dark' ? 'bg-[#1e1e1e]' : 'bg-white',
)

/// 用户已改动过的偏好字段。挂载读配置到达前动过的字段以用户值为准，落盘也只写这些
/// 字段 —— 未触碰字段必须保持磁盘原值，整块覆盖会把存量的字号/主题抹成默认值
const touched = new Set<keyof EditorPrefs>()

/// 并入配置读到的偏好：逐字段跳过已触碰项（配置读取失败时 service 已返回兜底值，
/// 同样只并入未触碰字段）
function applyLoadedPrefs(loaded: EditorPrefs) {
  if (!touched.has('fontSize')) prefs.fontSize = loaded.fontSize
  if (!touched.has('tabSize')) prefs.tabSize = loaded.tabSize
  if (!touched.has('editorTheme')) prefs.editorTheme = loaded.editorTheme
}

/// 落盘防抖：拖字号滑杆会连发多次变更，逐次写配置既浪费 IPC 也可能撞上写竞态
const PERSIST_DEBOUNCE_MS = 400
let persistTimer: ReturnType<typeof setTimeout> | null = null

function handlePrefsChange(patch: Partial<EditorPrefs>) {
  for (const key of Object.keys(patch) as (keyof EditorPrefs)[]) touched.add(key)
  Object.assign(prefs, patch)
  applyPrefs()
  emit('prefs-change', { ...prefs })
  if (persistTimer) clearTimeout(persistTimer)
  persistTimer = setTimeout(() => {
    persistTimer = null
    void persistPrefs()
  }, PERSIST_DEBOUNCE_MS)
}

/// 只取用户真正改过的字段：`updateEditorPrefs` 是部分写语义，未触碰字段保持磁盘原值
function touchedPatch(): Partial<EditorPrefs> {
  const patch: Partial<EditorPrefs> = {}
  if (touched.has('fontSize')) patch.fontSize = prefs.fontSize
  if (touched.has('tabSize')) patch.tabSize = prefs.tabSize
  if (touched.has('editorTheme')) patch.editorTheme = prefs.editorTheme
  return patch
}

/// 即时应用到 Monaco 实例。字号/Tab 是实例选项；**主题是全局选项**
/// （Monaco 无按实例主题），故同页其它编辑器（快捷提交对话框等）一并跟随。
function applyPrefs() {
  if (!editor.value) return
  editor.value.updateOptions({ fontSize: prefs.fontSize, tabSize: prefs.tabSize })
  monaco.editor.setTheme(prefs.editorTheme)
}

/// 恢复默认值：与手动改动同一条链路（即时生效 + 落盘），不是只改本地内存
function resetPrefs() {
  handlePrefsChange({
    fontSize: DEFAULT_EDITOR_FONT_SIZE,
    tabSize: DEFAULT_EDITOR_TAB_SIZE,
    editorTheme: DEFAULT_EDITOR_THEME,
  })
}

async function persistPrefs() {
  const patch = touchedPatch()
  // 无改动可写（只读实例等）：不发起无意义的配置写
  if (Object.keys(patch).length === 0) return
  try {
    await configService.updateEditorPrefs(patch)
    prefsError.value = null
  } catch (e) {
    // 已应用到编辑器，仅落盘失败：提示「仅本次会话生效」而不是回滚 ——
    // 把用户刚调好的字号弹回去比不持久化更糟
    log.error('保存编辑器设置失败:', e)
    prefsError.value = '保存失败，设置仅本次会话生效'
  }
}

/// 供父级程序化聚焦（题目总览「快捷提交」跳转 ?focus=1）；
/// Monaco 实例尚未就绪时静默降级为 no-op，不抛错、不阻塞调用方
function focus() {
  editor.value?.focus()
}

defineExpose({ focus })
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <!-- 工具条（只读模式隐藏：详情页代码查看无语言切换/提交语义） -->
    <div
      v-if="!readonly"
      class="flex h-10 shrink-0 items-center justify-between border-b border-slate-200 bg-slate-100 px-4 select-none"
    >
      <!-- 左：语言选择 -->
      <div class="relative">
        <button
          type="button"
          class="flex items-center gap-1.5 rounded border border-slate-300 bg-white px-2.5 py-1 font-mono text-[11px] font-semibold text-slate-800 shadow-sm transition-colors hover:border-[var(--color-primary)]"
          @click="langMenuOpen = !langMenuOpen"
        >
          <span class="font-bold text-[var(--color-primary)]">{{ language }}</span>
          <svg
            class="h-3.5 w-3.5 text-slate-400 transition-transform"
            :class="{ 'rotate-180': langMenuOpen }"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
          >
            <path d="M6 9l6 6 6-6" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
        <template v-if="langMenuOpen">
          <div class="fixed inset-0 z-40" @click="langMenuOpen = false"></div>
          <div
            class="absolute left-0 top-full z-50 mt-1 w-32 overflow-hidden rounded-md border border-slate-200 bg-white py-1 shadow-lg"
          >
            <button
              v-for="lang in availableLanguages"
              :key="lang"
              type="button"
              class="flex w-full items-center justify-between px-3 py-1.5 text-left font-mono text-[11px] transition-colors"
              :class="
                lang === language
                  ? 'bg-[#f5f3ff] font-semibold text-[#6845f5]'
                  : 'text-slate-700 hover:bg-slate-50'
              "
              @click="selectLanguage(lang)"
            >
              {{ lang }}
              <svg
                v-if="lang === language"
                class="h-3 w-3"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                viewBox="0 0 24 24"
              >
                <path d="M5 13l4 4L19 7" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </button>
          </div>
        </template>
      </div>

      <!-- 右：清空 / 上传 / 设置 + 备份指示 -->
      <div class="flex items-center gap-2">
        <div class="relative">
          <button
            type="button"
            title="清空代码"
            class="flex h-7 w-7 items-center justify-center rounded border border-slate-200 bg-white text-slate-500 shadow-sm transition-colors duration-150 hover:border-rose-200 hover:bg-rose-50 hover:text-rose-600"
            @click="confirmClearOpen = !confirmClearOpen"
          >
            <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
          <!-- 清空二次确认气泡 -->
          <template v-if="confirmClearOpen">
            <div class="fixed inset-0 z-40" @click="confirmClearOpen = false"></div>
            <div
              class="absolute right-0 top-full z-50 mt-1.5 w-52 rounded-lg border border-slate-200 bg-white p-3 shadow-xl"
            >
              <p class="text-xs font-medium text-slate-700">清空编辑器内全部代码？</p>
              <p class="mt-0.5 text-[11px] text-slate-400">此操作会覆盖当前工作区代码</p>
              <div class="mt-2.5 flex justify-end gap-2">
                <button
                  type="button"
                  class="rounded border border-slate-200 bg-slate-50 px-2.5 py-1 text-[11px] font-medium text-slate-600 transition hover:bg-slate-100"
                  @click="confirmClearOpen = false"
                >
                  取消
                </button>
                <button
                  type="button"
                  class="rounded bg-rose-600 px-2.5 py-1 text-[11px] font-semibold text-white transition hover:bg-rose-700"
                  @click="confirmClear"
                >
                  确认清空
                </button>
              </div>
            </div>
          </template>
        </div>

        <button
          type="button"
          title="上传代码"
          class="flex h-7 w-7 items-center justify-center rounded border border-slate-200 bg-white text-slate-500 shadow-sm transition-colors duration-150 hover:border-[var(--color-primary)] hover:bg-slate-50 hover:text-[var(--color-primary)]"
          @click="pickFile"
        >
          <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
        <input
          ref="fileInput"
          type="file"
          :accept="fileAccept"
          class="hidden"
          @change="handleFileChange"
        />

        <EditorSettingsPopover
          :font-size="prefs.fontSize"
          :tab-size="prefs.tabSize"
          :editor-theme="prefs.editorTheme"
          :error="prefsError"
          @change="handlePrefsChange"
          @reset="resetPrefs"
        />

        <div class="h-3.5 w-px bg-slate-300"></div>

        <!-- 自动备份指示：dirty=编辑中（灰）/ 已落盘=已自动备份（绿） -->
        <span
          class="flex items-center gap-1.5 pl-1 font-mono text-[11px] font-medium"
          :class="isDirty ? 'text-slate-400' : 'text-emerald-600'"
        >
          <svg v-if="isDirty" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <svg v-else class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
            <path d="M2.25 15a4.5 4.5 0 004.5 4.5H18a3.75 3.75 0 001.332-7.257 3 3 0 00-3.758-3.848 5.25 5.25 0 00-10.233 2.33A4.502 4.502 0 002.25 15z" stroke-linecap="round" stroke-linejoin="round" />
            <path d="M9 13l2 2 4-4" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>{{ isDirty ? '编辑中…' : '已自动备份' }}</span>
        </span>
      </div>
    </div>

    <!-- Monaco Editor（底色跟随主题，见 editorSurfaceClass） -->
    <div ref="editorContainer" class="min-h-0 flex-1" :class="editorSurfaceClass" />
  </div>
</template>
