<script setup lang="ts">
import { ref, onMounted } from 'vue'
import AppHeader from '@/components/layout/AppHeader.vue'
import ProblemSidebar from '@/components/problem/ProblemSidebar.vue'
import ProblemStatement from '@/components/problem/ProblemStatement.vue'
import CodeEditor from '@/components/editor/CodeEditor.vue'
import SubmissionPanel from '@/components/submission/SubmissionPanel.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import { useWorkspaceStore } from '@/stores/workspaceStore'

const contest = useContestStore()
const problem = useProblemStore()
const submission = useSubmissionStore()
const workspace = useWorkspaceStore()

const rightRatio = ref(0.5)

/// 分栏拖拽
function startDrag(e: MouseEvent) {
  const startX = e.clientX
  const startRatio = rightRatio.value
  const containerWidth = (e.currentTarget as HTMLElement).parentElement!.clientWidth

  function onMove(ev: MouseEvent) {
    const dx = startX - ev.clientX
    rightRatio.value = Math.min(0.7, Math.max(0.3, startRatio + dx / containerWidth))
  }
  function onUp() {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
  }

  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onUp)
}

/// 加载比赛
async function loadContest() {
  try {
    await contest.loadContest()
    // 自动打开第一道题
    const first = contest.problems[0]
    if (first) await openProblem(first.displayId, first.problemId)
  } catch {
    // error set by store
  }
}

/// 打开题目（displayId for API, problemId for workspace）
async function openProblem(displayId: string, problemId: string) {
  if (!contest.contest) return
  // 保存当前工作区
  if (workspace.isDirty) await workspace.saveWorkspace()
  // load_workspace uses pid; get_problem uses displayId
  await workspace.loadWorkspace(contest.contest.id, problemId)
  await problem.openProblem(contest.contest.id, displayId)
}

/// 提交代码（评测轮询由 submissionStore 编排，View 只表达提交意图）
async function handleSubmit() {
  if (!contest.contest || !problem.currentProblem) return
  try {
    await submission.submitCode(
      contest.contest.id,
      problem.currentProblem.id,
      workspace.language,
      workspace.code,
    )
  } catch {
    // 失败原因已由 submissionStore 写入 error
  }
}

/// 页面加载：会话有效性已由路由守卫（meta.requiresAuth）保证，此处只负责加载比赛数据
onMounted(() => {
  void loadContest()
})
</script>

<template>
  <div class="flex h-full flex-col bg-[var(--bg-body)]">
    <!-- 顶部栏 -->
    <AppHeader />

    <!-- 主内容区：三栏布局 -->
    <div v-if="contest.isLoading" class="flex-1 flex items-center justify-center">
        <LoadingSpinner message="正在加载比赛..." />
      </div>

      <ErrorMessage
        v-else-if="contest.error"
        :message="contest.error"
        :retry="loadContest"
        class="flex-1"
      />

      <div v-else class="flex flex-1 overflow-hidden">
        <!-- 左侧：题目列表 -->
        <ProblemSidebar
          :problems="contest.problems"
          :current-id="problem.currentProblem?.id ?? null"
          @select="openProblem"
        />

        <!-- 中右分栏 -->
        <div class="flex flex-1 overflow-hidden">
          <!-- 中间：题面 -->
          <div class="flex flex-col overflow-hidden border-r border-[var(--border-color)]" :style="{ flex: 1 - rightRatio }">
            <LoadingSpinner
              v-if="problem.isLoading"
              message="正在加载题目..."
              class="flex-1"
            />
            <ErrorMessage
              v-else-if="problem.error"
              :message="problem.error"
              class="flex-1"
            />
            <ProblemStatement
              v-else-if="problem.currentProblem"
              :problem="problem.currentProblem"
            />
            <div v-else class="flex-1 flex items-center justify-center text-sm text-[var(--text-secondary)]">
              请从左侧选择题目
            </div>
          </div>

          <!-- 拖拽分割条 -->
          <div
            class="w-1 cursor-col-resize bg-[var(--border-color)] hover:bg-[var(--color-primary)] active:bg-[var(--color-primary)] transition-colors shrink-0"
            @mousedown="startDrag"
          />

          <!-- 右侧：编辑器 + 提交面板 -->
          <div class="flex flex-col overflow-hidden" :style="{ flex: rightRatio }">
            <CodeEditor
              :model-value="workspace.code"
              :language="workspace.language"
              @update:model-value="workspace.updateCode"
              @update:language="(lang: string) => workspace.changeLanguage(lang)"
              @submit="handleSubmit"
            />

            <SubmissionPanel
              :submissions="submission.submissions"
              class="h-[200px] shrink-0"
            />
          </div>
        </div>
      </div>
    </div>
</template>
