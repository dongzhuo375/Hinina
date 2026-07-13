<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { NMessageProvider } from 'naive-ui'
import AppHeader from '@/components/layout/AppHeader.vue'
import ProblemSidebar from '@/components/problem/ProblemSidebar.vue'
import ProblemStatement from '@/components/problem/ProblemStatement.vue'
import CodeEditor from '@/components/editor/CodeEditor.vue'
import SubmissionPanel from '@/components/submission/SubmissionPanel.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import { useWorkspaceStore } from '@/stores/workspaceStore'

const router = useRouter()
const auth = useAuthStore()
const contest = useContestStore()
const problem = useProblemStore()
const submission = useSubmissionStore()
const workspace = useWorkspaceStore()

const leftWidth = ref(220)
const rightRatio = ref(0.5)
let isDragging = false

/// 分栏拖拽
function startDrag(e: MouseEvent) {
  isDragging = true
  const startX = e.clientX
  const startRatio = rightRatio.value
  const containerWidth = (e.currentTarget as HTMLElement).parentElement!.clientWidth

  function onMove(ev: MouseEvent) {
    const dx = startX - ev.clientX
    rightRatio.value = Math.min(0.7, Math.max(0.3, startRatio + dx / containerWidth))
  }
  function onUp() {
    isDragging = false
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
    if (first) await openProblem(first.problemId)
  } catch {
    // error set by store
  }
}

/// 打开题目
async function openProblem(problemId: string) {
  if (!contest.contest) return
  // 保存当前工作区
  if (workspace.isDirty) await workspace.saveWorkspace()
  // 加载新题目的工作区
  await workspace.loadWorkspace(contest.contest.id, problemId)
  // 获取题目详情
  await problem.openProblem(contest.contest.id, problemId)
}

/// 提交代码
async function handleSubmit() {
  if (!contest.contest || !problem.currentProblem) return
  await submission.submitCode(
    contest.contest.id,
    problem.currentProblem.id,
    workspace.language,
    workspace.code,
  )
}

/// 页面加载
onMounted(async () => {
  // 检查登录状态
  const hasSession = await auth.checkSession()
  if (!hasSession) {
    router.replace('/login')
    return
  }
  await loadContest()
})
</script>

<template>
  <n-message-provider>
    <div class="flex h-screen flex-col bg-[var(--bg-body)]">
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
  </n-message-provider>
</template>
