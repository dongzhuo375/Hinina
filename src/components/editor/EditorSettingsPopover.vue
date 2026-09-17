<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  EDITOR_FONT_SIZE_MAX,
  EDITOR_FONT_SIZE_MIN,
  EDITOR_TAB_SIZES,
  EDITOR_THEMES,
} from '@/utils/editor'

/// 解题页编辑器设置弹层：字号 / Tab 宽度 / 编辑器主题。
///
/// 只负责呈现与收集意图 —— **即时应用与落盘由 CodeEditor 承担**：
/// 组件不持有 Monaco 实例、也不直接写配置（单向数据流，与 cursor 上报同款约定），
/// 故可被任何持有编辑器实例的容器复用。
const props = defineProps<{
  fontSize: number
  tabSize: number
  editorTheme: string
  /// 落盘失败提示（父级写入；为空时不占位）
  error?: string | null
}>()

const emit = defineEmits<{
  /// 偏好增量变更：父级应用到 Monaco 实例并落盘
  change: [patch: { fontSize?: number; tabSize?: number; editorTheme?: string }]
  /// 恢复默认值：父级以默认值走同一条变更链路（同样即时生效 + 落盘）
  reset: []
}>()

const open = ref(false)

/// Tab 宽度候选：**始终包含当前值**。存储域（1–8）比候选集宽，`config.json` 可被
/// 手改为 6 这类非候选值 —— 此时若只渲染候选档位，会出现「一个都不选中」的假象，
/// 用户会以为设置丢了。故非候选值也作为一枚档位呈现（升序插入）。
const tabSizeOptions = computed(() => {
  const list = [...EDITOR_TAB_SIZES]
  if (!list.includes(props.tabSize)) list.push(props.tabSize)
  return list.sort((a, b) => a - b)
})

/// 字号滑杆：`input[type=range]` 的 value 是字符串，统一转数字后再上抛
function onFontSizeInput(e: Event) {
  const value = Number((e.target as HTMLInputElement).value)
  if (Number.isFinite(value) && value !== props.fontSize) emit('change', { fontSize: value })
}
</script>

<template>
  <div class="relative">
    <button
      type="button"
      title="编辑器设置"
      aria-haspopup="dialog"
      :aria-expanded="open"
      class="flex h-7 w-7 items-center justify-center rounded border border-slate-200 bg-white text-slate-500 shadow-sm transition-colors duration-150 hover:border-[var(--color-primary)] hover:bg-slate-50 hover:text-[var(--color-primary)]"
      @click="open = !open"
    >
      <svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
        <path d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </button>

    <template v-if="open">
      <!-- 点击面板外关闭（与语言下拉/清空确认同款遮罩） -->
      <div class="fixed inset-0 z-40" @click="open = false"></div>
      <div
        role="dialog"
        aria-label="编辑器设置"
        class="absolute top-full right-0 z-50 mt-1.5 max-h-[calc(100vh-7rem)] w-64 overflow-y-auto rounded-lg border border-slate-200 bg-white p-3.5 shadow-xl"
      >
        <div class="flex items-center justify-between">
          <span class="text-xs font-semibold text-slate-800">编辑器设置</span>
          <button
            type="button"
            title="关闭"
            class="flex h-5 w-5 items-center justify-center rounded text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-600"
            @click="open = false"
          >
            <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
              <path d="M6 18L18 6M6 6l12 12" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
        </div>

        <!-- 字号 -->
        <div class="mt-3">
          <div class="flex items-baseline justify-between">
            <span class="text-xs font-medium text-slate-700">字号</span>
            <span class="font-mono text-[11px] text-slate-400">{{ fontSize }} px</span>
          </div>
          <input
            type="range"
            :min="EDITOR_FONT_SIZE_MIN"
            :max="EDITOR_FONT_SIZE_MAX"
            step="1"
            :value="fontSize"
            aria-label="字号"
            class="mt-2 w-full accent-[var(--color-primary)]"
            @input="onFontSizeInput"
          />
        </div>

        <!-- Tab 宽度 -->
        <div class="mt-3.5">
          <span class="text-xs font-medium text-slate-700">Tab 宽度</span>
          <div class="mt-2 flex gap-1.5">
            <button
              v-for="size in tabSizeOptions"
              :key="size"
              type="button"
              class="flex-1 rounded border py-1 font-mono text-[11px] transition-colors"
              :class="
                size === tabSize
                  ? 'border-[var(--color-primary)] bg-[#f5f3ff] font-semibold text-[#6845f5]'
                  : 'border-slate-200 bg-white text-slate-600 hover:bg-slate-50'
              "
              @click="emit('change', { tabSize: size })"
            >
              {{ size }}
            </button>
          </div>
        </div>

        <!-- 编辑器主题（仅编辑器区域；客户端界面仍为浅色） -->
        <div class="mt-3.5">
          <span class="text-xs font-medium text-slate-700">编辑器主题</span>
          <div class="mt-2 flex gap-1.5">
            <button
              v-for="theme in EDITOR_THEMES"
              :key="theme.id"
              type="button"
              :title="theme.hint"
              class="flex-1 rounded border py-1 text-[11px] transition-colors"
              :class="
                theme.id === editorTheme
                  ? 'border-[var(--color-primary)] bg-[#f5f3ff] font-semibold text-[#6845f5]'
                  : 'border-slate-200 bg-white text-slate-600 hover:bg-slate-50'
              "
              @click="emit('change', { editorTheme: theme.id })"
            >
              {{ theme.label }}
            </button>
          </div>
        </div>

        <div class="mt-3.5 flex items-center gap-2 border-t border-slate-100 pt-2.5">
          <!-- 落盘失败才提示；成功路径无需打扰（改动已即时可见） -->
          <span v-if="error" class="text-[10px] leading-tight text-rose-600">{{ error }}</span>
          <button
            type="button"
            class="ml-auto shrink-0 rounded border border-slate-200 bg-slate-50 px-2 py-1 text-[10px] font-medium whitespace-nowrap text-slate-600 transition hover:bg-slate-100"
            @click="emit('reset')"
          >
            恢复默认
          </button>
        </div>
      </div>
    </template>
  </div>
</template>
