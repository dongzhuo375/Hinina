<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { Problem } from '@/types/problem'
import { NCard, NTag, NScrollbar, NTabs, NTabPane } from 'naive-ui'
import { renderMarkdown } from '@/utils/markdown'
import { getConfig } from '@/bridge/config.bridge'

const props = defineProps<{
  problem: Problem
}>()

/// HOJ 服务端地址（用于把题目描述中的相对图片 URL 改写为绝对地址）
const baseUrl = ref('')

onMounted(async () => {
  try {
    const config = await getConfig()
    baseUrl.value = config.oj.hojUrl
  } catch {
    // 配置获取失败时保持空，相对路径将按原样输出
  }
})

/// 将 Markdown 题面渲染为 HTML
const renderedDescription = computed(() => renderMarkdown(props.problem.description, baseUrl.value))
const renderedInput = computed(() => renderMarkdown(props.problem.inputDescription, baseUrl.value))
const renderedOutput = computed(() => renderMarkdown(props.problem.outputDescription, baseUrl.value))
</script>

<template>
  <n-scrollbar class="flex-1">
    <div class="p-5">
      <!-- 题目标题与限制 -->
      <div class="mb-5">
        <h2 class="text-xl font-bold text-[var(--text-primary)]">{{ problem.title }}</h2>
        <div class="mt-2 flex gap-2">
          <n-tag size="small" :bordered="false" type="info">
            时间限制: {{ problem.timeLimit }}ms
          </n-tag>
          <n-tag size="small" :bordered="false" type="info">
            内存限制: {{ problem.memoryLimit }}MB
          </n-tag>
        </div>
      </div>

      <!-- 题面 -->
      <n-tabs type="line" size="small" animated>
        <n-tab-pane name="desc" tab="题目描述">
          <div class="prose prose-sm max-w-none pt-3 text-[var(--text-primary)]" v-html="renderedDescription" />
        </n-tab-pane>

        <n-tab-pane name="input" tab="输入说明">
          <div class="prose prose-sm max-w-none pt-3 text-[var(--text-primary)]" v-html="renderedInput" />
        </n-tab-pane>

        <n-tab-pane name="output" tab="输出说明">
          <div class="prose prose-sm max-w-none pt-3 text-[var(--text-primary)]" v-html="renderedOutput" />
        </n-tab-pane>

        <n-tab-pane name="samples" tab="样例">
          <div class="pt-3 space-y-4">
            <div
              v-for="(sample, i) in problem.samples"
              :key="i"
              class="rounded-lg border border-[var(--border-color)] overflow-hidden"
            >
              <div class="px-3 py-1.5 text-xs font-semibold text-[var(--text-secondary)] bg-[var(--bg-sidebar)]">
                样例 {{ i + 1 }}
              </div>
              <div class="grid grid-cols-2 divide-x divide-[var(--border-color)]">
                <div class="p-3">
                  <div class="mb-1 text-xs text-[var(--text-secondary)]">输入</div>
                  <pre class="text-sm font-mono whitespace-pre-wrap text-[var(--text-primary)]">{{ sample.input }}</pre>
                </div>
                <div class="p-3">
                  <div class="mb-1 text-xs text-[var(--text-secondary)]">输出</div>
                  <pre class="text-sm font-mono whitespace-pre-wrap text-[var(--text-primary)]">{{ sample.output }}</pre>
                </div>
              </div>
            </div>
          </div>
        </n-tab-pane>
      </n-tabs>
    </div>
  </n-scrollbar>
</template>

<style scoped>
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
}
.prose :deep(img) {
  max-width: 100%;
  height: auto;
}
</style>
