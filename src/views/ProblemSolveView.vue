<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import ProblemTabStrip from '@/components/problem/ProblemTabStrip.vue'
import ProblemStatement from '@/components/problem/ProblemStatement.vue'
import CodeEditor from '@/components/editor/CodeEditor.vue'
import EditorConsoleBar from '@/components/editor/EditorConsoleBar.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { useWorkspaceStore } from '@/stores/workspaceStore'
import { useSubmissionStore } from '@/stores/submissionStore'
import { configService } from '@/services/config.service'
import type { EditorPrefs } from '@/services/config.service'
import { resolveAllowedLanguage } from '@/utils/language'
import { DEFAULT_EDITOR_TAB_SIZE } from '@/utils/editor'
import { errorMessage } from '@/utils/error'
import { createLogger } from '@/utils/logger'

const log = createLogger('ProblemSolveView')

/// 解题工作台：左题面（48%）/ 右代码编辑器（52%），中间 1px 拖拽条可调。
const route = useRoute()
const router = useRouter()
const contestStore = useContestStore()
const problemStore = useProblemStore()
const workspaceStore = useWorkspaceStore()
const submissionStore = useSubmissionStore()

const displayId = computed(() => String(route.params.displayId ?? ''))

// ── 加载编排：contest → workspace（按 pid）→ problem 详情（按 displayId） ──

const localError = ref<string | null>(null)
const viewError = computed(() => localError.value ?? problemStore.error)

/// 并发防护：displayId 快速切换时只认最后一次加载的结果
let loadToken = 0
/// 加载进行中标记：?focus=1 的 query 监听据此让位给 load 完成后的统一消费
let isLoadingPage = false

async function ensureContestId(): Promise<string> {
  if (!contestStore.contest) {
    // 直接进入本页（刷新/深链）时外壳可能尚未加载完比赛数据；
    // whenLoaded 复用在途请求，避免重复 IPC（P59 统一入口）
    await contestStore.whenLoaded()
  }
  const id = contestStore.contest?.id
  if (!id) throw new Error(contestStore.error ?? '加载比赛失败')
  return id
}

async function load(id: string) {
  const token = ++loadToken
  localError.value = null
  isLoadingPage = true
  try {
    // 切题前先落盘未保存代码 —— 代码保留是工作区的核心承诺。
    // saveWorkspace 内部会先把在途的防抖改动推送到后端内存再落盘，因此这里
    // 同时覆盖「刚敲完就切题」：不先推送，落盘写的会是上一次同步的旧内容
    if (workspaceStore.isDirty) {
      await workspaceStore.saveWorkspace().catch((e) => {
        log.error('切题前保存工作区失败:', e)
      })
    }
    const contestId = await ensureContestId()
    if (token !== loadToken) return

    const cp = contestStore.problems.find((p) => p.displayId === id)
    if (!cp) {
      localError.value = `未在当前比赛中找到题目 ${id}`
      return
    }

    // 工作区按题目真实 ID（pid）隔离，题面详情按比赛内展示题号查询
    await workspaceStore.loadWorkspace(contestId, cp.problemId)
    if (token !== loadToken) return
    await problemStore.openProblem(contestId, id)
    if (token !== loadToken) return

    // 允许语言归位（与 QuickSubmitDialog 同款语义）：工作区/配置默认存的是规范名
    //（"C++"），服务端按题列表可能是部署变体（"C++17 (GCC 13.2)"）或不含当前语言族。
    // 提交参数必须用服务端认得的写法，故加载后统一归位；changeLanguage 同名短路、
    // 归位幂等，写回服务端原名反而让下次加载直接命中
    const allowed = problemStore.currentProblem?.languages ?? []
    if (allowed.length > 0) {
      const target = resolveAllowedLanguage(workspaceStore.language, allowed) ?? allowed[0]
      if (target !== workspaceStore.language) workspaceStore.changeLanguage(target)
    }

    // limits 与我的状态供题面限制/Tab 状态点使用；失败不抛出（store 内部已兜底），
    // 从题目总览进入时通常已缓存，此处补齐直接进入本页的场景
    void problemStore.loadLimits(contestId, [id])
    void problemStore.loadMyStatus(
      contestId,
      contestStore.problems.map((p) => p.problemId),
    )
  } catch (e) {
    if (token !== loadToken) return
    localError.value = errorMessage(e, '加载题目失败')
  } finally {
    // 只有最新一次加载负责收尾；被取代的加载直接退出，由新加载统一消费 focus
    if (token === loadToken) {
      isLoadingPage = false
      await consumeFocusQuery()
    }
  }
}

// ── 快捷提交联动（题目总览跳转携带 ?focus=1） ──

const codeEditor = ref<InstanceType<typeof CodeEditor> | null>(null)

/// 消费 ?focus=1：题目与工作区加载完成后把焦点移入 Monaco，随后清掉 query
/// （保留 params），避免切题回来或刷新后反复抢焦点。
/// 编辑器未就绪时静默降级（不聚焦），但 query 仍然清除，保证行为幂等、不阻塞加载
async function consumeFocusQuery() {
  if (route.query.focus !== '1') return
  await nextTick()
  codeEditor.value?.focus()
  await router.replace({ query: {} })
}

// 立即执行一次覆盖挂载场景；路由参数变化（Tab 切题）时重新加载。
// 必须先于 query.focus 监听注册：两者同批触发时按注册顺序执行，
// load() 同步置位 isLoadingPage 后，focus 监听才会让位给 load 完成后的统一消费
watch(displayId, (id) => { if (id) load(id) }, { immediate: true })

// query 单独变化（已在本题、仅追加 focus=1）时直接消费；
// 与 displayId 同时变化时 isLoadingPage 已置位，由 load 的 finally 统一处理
watch(
  () => route.query.focus,
  (v) => {
    if (v === '1' && !isLoadingPage) void consumeFocusQuery()
  },
)

// ── 落盘时机：失焦 / 页面隐藏 / 离开解题页 ──
//
// 后端 auto-save 周期最长 30 秒（配置上限 300 秒），而「内存 → 磁盘」之间只有
// 后端内存副本：这三处是内存可能被替换（切题/离开）或进程可能退出（关窗由
// main.ts 的关窗握手负责）的时刻，必须主动落盘。

/// 落盘进行中标记：失焦与页面隐藏可能同时触发，避免重复落盘
let flushingToDisk = false

async function flushToDisk(reason: string): Promise<void> {
  if (flushingToDisk) return
  flushingToDisk = true
  try {
    await workspaceStore.saveWorkspace()
  } catch (e) {
    // 落盘失败不打断使用：内容仍在编辑器与后端内存，下次时机或 auto-save 会重试
    log.error(`${reason}落盘工作区失败:`, e)
  } finally {
    flushingToDisk = false
  }
}

function onVisibilityChange() {
  if (document.visibilityState === 'hidden') void flushToDisk('页面隐藏时')
}

function onWindowBlur() {
  void flushToDisk('窗口失焦时')
}

onMounted(() => {
  document.addEventListener('visibilitychange', onVisibilityChange)
  window.addEventListener('blur', onWindowBlur)
})

onBeforeUnmount(() => {
  document.removeEventListener('visibilitychange', onVisibilityChange)
  window.removeEventListener('blur', onWindowBlur)
  // 离开解题页（切到评测/榜单/设置）：在途改动与未落盘内容一并落地
  void flushToDisk('离开解题页时')
})

// ── 左右分栏拖拽（0.3 – 0.7） ──

const splitRatio = ref(0.48)
const splitContainer = ref<HTMLElement>()
/// 用户是否已手动拖拽 —— 配置异步到达时不得覆盖用户刚调好的比例
let userAdjustedSplit = false

// 初始分栏比例读取配置（P55：layout.splitRatio 消费落地）；失败回退设计稿 0.48
void configService.getSplitRatio().then((ratio) => {
  if (!userAdjustedSplit) splitRatio.value = ratio
})

/// 拖拽结束后把比例写回配置（下次进入解题页生效）；失败只记录，不打断使用
function persistSplitRatio() {
  const ratio = splitRatio.value
  configService
    .updateConfig((draft) => {
      draft.layout.splitRatio = ratio
    })
    .catch((e) => {
      log.error('分栏比例持久化失败:', e)
    })
}

function startDrag(e: MouseEvent) {
  e.preventDefault()
  const container = splitContainer.value
  if (!container) return
  const rect = container.getBoundingClientRect()
  if (rect.width <= 0) return

  const onMove = (ev: MouseEvent) => {
    const ratio = (ev.clientX - rect.left) / rect.width
    userAdjustedSplit = true
    splitRatio.value = Math.min(0.7, Math.max(0.3, ratio))
  }
  const onUp = () => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    if (userAdjustedSplit) persistSplitRatio()
  }
  // 拖拽期间全局锁定光标与选区，避免划过题面时选中文字
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

// ── 编辑器状态 ──

/// Monaco 光标位置（CodeEditor emit → 本视图 → EditorConsoleBar prop，单向数据流）
const cursor = ref<{ line: number; column: number } | null>(null)

/// 编辑器偏好（CodeEditor 挂载读配置后 + 每次弹层改动后上报）。本视图只消费
/// `tabSize` 供状态行展示缩进宽度，不重复读配置，避免与弹层写入竞态
const editorPrefs = ref<EditorPrefs | null>(null)

const currentProblemId = computed(() => problemStore.currentProblem?.id ?? null)

async function handleSubmit() {
  const contestId = contestStore.contest?.id
  const problem = problemStore.currentProblem
  if (!contestId || !problem || submissionStore.isSubmitting) return
  // displayId 取路由参数（比赛内题号 "A"）—— HOJ 提交接口认的是它而不是数字 pid
  const displayId = String(route.params.displayId ?? '')
  try {
    // 轮询由 store 在提交成功后自动启动（终态或超时停止），此处不重复实现
    await submissionStore.submitCode(
      contestId,
      problem.id,
      displayId,
      workspaceStore.language,
      workspaceStore.code,
    )
  } catch {
    // 失败原因已由 store 写入 submissionStore.error，EditorConsoleBar 负责展示
  }
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-slate-50">
    <ProblemTabStrip />

    <div ref="splitContainer" class="flex min-h-0 flex-1 overflow-hidden">
      <!-- 左栏：题面（48%，可拖拽调整） -->
      <section
        class="flex min-w-0 flex-col overflow-hidden bg-white"
        :style="{ width: `${splitRatio * 100}%` }"
      >
        <div v-if="viewError" class="flex flex-1 flex-col justify-center">
          <ErrorMessage :message="viewError" :retry="() => load(displayId)" />
        </div>
        <div
          v-else-if="problemStore.isLoading || !problemStore.currentProblem"
          class="flex flex-1 flex-col justify-center"
        >
          <LoadingSpinner message="加载题目中…" />
        </div>
        <ProblemStatement
          v-else
          :problem="problemStore.currentProblem"
          :display-id="displayId"
        />
      </section>

      <!-- 拖拽条 -->
      <div
        class="w-px shrink-0 cursor-col-resize bg-slate-200 transition-colors hover:bg-[var(--color-primary)]"
        @mousedown="startDrag"
      ></div>

      <!-- 右栏：代码编辑器 + 控制台条（52%） -->
      <section class="flex min-w-0 flex-1 flex-col overflow-hidden bg-slate-50">
        <CodeEditor
          ref="codeEditor"
          :model-value="workspaceStore.code"
          :language="workspaceStore.language"
          :languages="problemStore.currentProblem?.languages ?? []"
          :is-dirty="workspaceStore.isDirty"
          @update:model-value="workspaceStore.updateCode"
          @update:language="workspaceStore.changeLanguage"
          @cursor="cursor = $event"
          @submit="handleSubmit"
          @prefs-change="editorPrefs = $event"
        />
        <EditorConsoleBar
          :cursor="cursor"
          :problem-id="currentProblemId"
          :tab-size="editorPrefs?.tabSize ?? DEFAULT_EDITOR_TAB_SIZE"
          @submit="handleSubmit"
        />
      </section>
    </div>
  </div>
</template>
