<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick, shallowRef } from 'vue'
import { NButton, NSelect } from 'naive-ui'
import * as monaco from 'monaco-editor'

// ── Monaco Editor Workers（手动配置，避免 worker 打包问题） ──
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker'
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker'
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker'
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker'

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
  language: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'update:language': [language: string]
  submit: []
}>()

const editorContainer = ref<HTMLDivElement>()
const editor = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null)
const isReadonly = ref(false)

/// 支持的语言
const languages = [
  { label: 'C', value: 'c' },
  { label: 'C++', value: 'cpp' },
  { label: 'Java', value: 'java' },
  { label: 'Python', value: 'python' },
]

/// Monaco language ID 映射
const langMap: Record<string, string> = {
  c: 'c',
  cpp: 'cpp',
  java: 'java',
  python: 'python',
}

onMounted(async () => {
  await nextTick()
  if (!editorContainer.value) return

  const ed = monaco.editor.create(editorContainer.value, {
    value: props.modelValue,
    language: langMap[props.language] || 'cpp',
    theme: 'vs-dark',
    fontSize: 14,
    fontFamily: 'JetBrains Mono, Cascadia Code, Consolas, monospace',
    minimap: { enabled: false },
    lineNumbers: 'on',
    scrollBeyondLastLine: false,
    automaticLayout: true,
    tabSize: 4,
    wordWrap: 'on',
    padding: { top: 12, bottom: 12 },
  })

  ed.onDidChangeModelContent(() => {
    emit('update:modelValue', ed.getValue())
  })

  editor.value = ed
})

onUnmounted(() => {
  editor.value?.dispose()
})

watch(() => props.language, (lang) => {
  const model = editor.value?.getModel()
  if (model) {
    monaco.editor.setModelLanguage(model, langMap[lang] || 'cpp')
  }
})

/// Ctrl+Enter 提交
function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault()
    emit('submit')
  }
}

onMounted(() => window.addEventListener('keydown', handleKeydown))
onUnmounted(() => window.removeEventListener('keydown', handleKeydown))
</script>

<template>
  <div class="flex flex-1 flex-col">
    <!-- 工具栏 -->
    <div class="flex items-center justify-between border-b border-[var(--border-color)] bg-[var(--bg-card)] px-3 py-2">
      <div class="flex items-center gap-3">
        <n-select
          :value="language"
          :options="languages"
          size="small"
          style="width: 100px"
          @update:value="(v: string) => emit('update:language', v)"
        />
        <span class="text-xs text-[var(--text-secondary)]">Ctrl+Enter 提交</span>
      </div>
      <n-button type="primary" size="small" @click="emit('submit')">
        提交代码
      </n-button>
    </div>

    <!-- Monaco Editor -->
    <div ref="editorContainer" class="flex-1" />
  </div>
</template>
