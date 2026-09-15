<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'

/// 题目快速切换条：横向 chips（A/B/C…），当前题高亮，chip 带我的提交状态点。
/// 数据直接取自 contestStore / problemStore（View → Store 分层，组件不触 Service/Bridge）。
const route = useRoute()
const router = useRouter()
const contestStore = useContestStore()
const problemStore = useProblemStore()

const problems = computed(() => contestStore.problems)
const currentDisplayId = computed(() => String(route.params.displayId ?? ''))

/// 状态点颜色：1=已AC（绿）/ 2=尝试过（amber）/ 0=未提交（不显示点）
function statusDotClass(status: number): string {
  if (status === 1) return 'bg-[var(--color-success)]'
  if (status === 2) return 'bg-[var(--color-warning)]'
  return ''
}

function go(displayId: string) {
  if (displayId === currentDisplayId.value) return
  router.push({ name: 'ProblemSolve', params: { displayId } })
}
</script>

<template>
  <div
    class="no-scrollbar flex h-10 shrink-0 items-center gap-1 overflow-x-auto border-b border-slate-200 bg-white px-3 select-none"
  >
    <button
      v-for="p in problems"
      :key="p.displayId"
      type="button"
      :title="`${p.displayId}. ${p.displayTitle}`"
      class="relative flex shrink-0 items-center gap-1.5 rounded-md px-3 py-1.5 font-mono text-xs font-semibold transition-colors"
      :class="
        p.displayId === currentDisplayId
          ? 'bg-[#f5f3ff] text-[#6845f5]'
          : 'text-slate-500 hover:bg-slate-100 hover:text-slate-900'
      "
      @click="go(p.displayId)"
    >
      <span
        v-if="problemStore.statusOf(p.problemId) !== 0"
        class="h-1.5 w-1.5 rounded-full"
        :class="statusDotClass(problemStore.statusOf(p.problemId))"
      ></span>
      {{ p.displayId }}
      <!-- 当前题下划线指示条 -->
      <span
        v-if="p.displayId === currentDisplayId"
        class="absolute inset-x-2 -bottom-[5px] h-0.5 rounded-full bg-[var(--color-primary)]"
      ></span>
    </button>
    <span v-if="problems.length === 0" class="px-2 text-xs text-slate-400">题目列表未加载</span>
  </div>
</template>

<style scoped>
/* 横向滚动但隐藏滚动条（设计稿的 tab 条无可见滚动条） */
.no-scrollbar {
  scrollbar-width: none;
}
.no-scrollbar::-webkit-scrollbar {
  display: none;
}
</style>
