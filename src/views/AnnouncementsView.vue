<script setup lang="ts">
import {
  computed,
  nextTick,
  onMounted,
  onUnmounted,
  ref,
  watch,
  type ComponentPublicInstance,
} from 'vue'
import ErrorMessage from '@/components/common/ErrorMessage.vue'
import LoadingSpinner from '@/components/common/LoadingSpinner.vue'
import { configService } from '@/services/config.service'
import { useAnnouncementStore } from '@/stores/announcementStore'
import { useContestStore } from '@/stores/contestStore'
import { renderMarkdown } from '@/utils/markdown'

const contestStore = useContestStore()
const announcementStore = useAnnouncementStore()

/// OJ 基址：把公告正文中的相对图片 URL 改写为绝对地址（读取失败回退空串）
const baseUrl = ref('')

/// 组件已卸载标记：异步引导链路可能在卸载后才落地，此时不得再触碰 store
let alive = true

const contest = computed(() => contestStore.contest)
const headerTitle = computed(() =>
  contest.value?.title ? `${contest.value.title} 赛事公告` : '赛事公告',
)

/// 比赛加载彻底失败（无数据可展示）：整页换成错误提示 + 重试
const isContestFailed = computed(() => contestStore.error !== null && !contest.value)
/// 首次加载：比赛或公告任一在途且没有任何可展示数据
const isBootstrapping = computed(
  () =>
    !isContestFailed.value &&
    ((!contest.value && contestStore.isLoading) ||
      (announcementStore.isLoading && !announcementStore.hasData)),
)
/// 公告加载失败且无旧数据（有旧数据时保留列表，错误交给下一轮轮询自愈）
const isAnnouncementFailed = computed(
  () => !isContestFailed.value && announcementStore.error !== null && !announcementStore.hasData,
)
const isEmpty = computed(
  () =>
    !isContestFailed.value &&
    !isBootstrapping.value &&
    !isAnnouncementFailed.value &&
    announcementStore.announcements.length === 0,
)

/// 公告正文 HTML 缓存：内容或基址变化才重渲染，避免模板里反复调 renderMarkdown
const renderedContents = computed(() => {
  const map = new Map<string, string>()
  for (const a of announcementStore.announcements) {
    map.set(a.id, renderMarkdown(a.content, baseUrl.value))
  }
  return map
})

/// createdAt 为 epoch 秒，与 Contest.startTime 同口径
function formatTime(epochSecs: number): string {
  if (!epochSecs || epochSecs <= 0) return '—'
  return new Date(epochSecs * 1000).toLocaleString('zh-CN', { hour12: false })
}

// ── 长文折叠：仅对实际溢出（scrollHeight > clientHeight）的卡片显示「展开全部」 ──

const bodyEls = new Map<string, HTMLElement>()
/// 已判定内容溢出的公告 ID 集合（展开后不再复测，标记保留）
const overflowIds = ref(new Set<string>())
/// 用户手动展开的公告 ID 集合
const expandedIds = ref(new Set<string>())

function setBodyRef(id: string) {
  return (el: Element | ComponentPublicInstance | null) => {
    if (el instanceof HTMLElement) bodyEls.set(id, el)
    else bodyEls.delete(id)
  }
}

function measureOverflow() {
  const next = new Set<string>()
  // 沿用仍然在列表中的既有标记（含已展开、无法复测的卡片）
  for (const id of overflowIds.value) {
    if (bodyEls.has(id)) next.add(id)
  }
  for (const [id, el] of bodyEls) {
    // 展开状态下 scrollHeight === clientHeight，跳过复测
    if (expandedIds.value.has(id)) continue
    if (el.scrollHeight > el.clientHeight + 2) next.add(id)
  }
  overflowIds.value = next
}

function toggleExpand(id: string) {
  const next = new Set(expandedIds.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  expandedIds.value = next
}

/// 列表刷新（轮询落新数据）或基址就绪后正文高度变化，需要重新测量
watch(
  () => [announcementStore.announcements, baseUrl.value] as const,
  async () => {
    await nextTick()
    measureOverflow()
  },
)

async function bootstrap(): Promise<void> {
  try {
    await contestStore.whenLoaded()
  } catch {
    // 失败原因已写入 contestStore.error，模板据此展示重试入口
    return
  }
  if (!alive) return
  const contestId = contest.value?.id
  if (!contestId) return
  try {
    await announcementStore.load(contestId)
  } catch {
    // 失败原因已写入 announcementStore.error
    return
  }
  if (!alive) return
  // 产品决策：进入公告页即把当前列表全部标记已读，ActivityBar 红点随之消失
  void announcementStore.markAllRead()
}

async function retryAnnouncements(): Promise<void> {
  const contestId = announcementStore.contestId || contest.value?.id
  if (!contestId) {
    await bootstrap()
    return
  }
  try {
    await announcementStore.load(contestId)
  } catch {
    // 同上，错误已写入 store
  }
}

function onRefresh() {
  void announcementStore.refresh()
}

onMounted(async () => {
  baseUrl.value = await configService.getOjBaseUrl()
  if (!alive) return
  await bootstrap()
  if (!alive) return
  await nextTick()
  measureOverflow()
})

// 轮询由外壳 ContestLayout 统一持有，本视图卸载时只标记失效、不停止轮询
onUnmounted(() => {
  alive = false
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden bg-[var(--bg-body)]">
    <!-- 头部：标题 + 统计 + 刷新 -->
    <div class="shrink-0 border-b border-[var(--border-color)] bg-[var(--bg-card)] px-6 py-4">
      <div class="mx-auto flex max-w-5xl flex-wrap items-center justify-between gap-3">
        <div class="min-w-0">
          <div class="flex min-w-0 items-center space-x-3">
            <h1 class="truncate text-lg font-bold tracking-tight text-[var(--text-primary)]">
              {{ headerTitle }}
            </h1>
            <span class="shrink-0 font-mono text-xs text-[var(--text-muted)]">/ Announcements</span>
          </div>
          <p class="mt-1 text-xs text-[var(--text-secondary)]">
            共 {{ announcementStore.total }} 条公告
            <span v-if="announcementStore.unreadCount > 0" class="text-[var(--color-primary)]">
              · 未读 {{ announcementStore.unreadCount }} 条
            </span>
          </p>
        </div>
        <button
          type="button"
          class="flex shrink-0 items-center gap-1.5 rounded-lg border border-[var(--border-color)] bg-white px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] shadow-sm transition-colors hover:bg-slate-50 hover:text-[var(--text-primary)] disabled:cursor-not-allowed disabled:opacity-50"
          :disabled="announcementStore.isLoading"
          title="刷新公告"
          @click="onRefresh"
        >
          <svg
            class="h-4 w-4"
            :class="{ 'animate-spin': announcementStore.isLoading }"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            viewBox="0 0 24 24"
            aria-hidden="true"
          >
            <path
              d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          <span>刷新</span>
        </button>
      </div>
    </div>

    <!-- 公告卡片流 -->
    <div class="flex min-h-0 flex-1 flex-col overflow-y-auto px-6 py-5">
      <div class="mx-auto w-full max-w-5xl">
        <ErrorMessage
          v-if="isContestFailed && contestStore.error"
          :message="contestStore.error"
          :retry="() => bootstrap()"
          class="m-auto"
        />

        <LoadingSpinner v-else-if="isBootstrapping" message="正在加载公告…" class="m-auto" />

        <ErrorMessage
          v-else-if="isAnnouncementFailed && announcementStore.error"
          :message="announcementStore.error"
          :retry="retryAnnouncements"
          class="m-auto"
        />

        <!-- 空列表 -->
        <div
          v-else-if="isEmpty"
          class="flex flex-col items-center justify-center gap-3 py-24 text-center"
        >
          <svg
            class="h-12 w-12 text-slate-300"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            viewBox="0 0 24 24"
            aria-hidden="true"
          >
            <path
              d="M11 5.882V19.24a1.76 1.76 0 01-3.417.592l-2.147-6.15M18 13a3 3 0 100-6M5.436 13.683A4.001 4.001 0 017 6h1.832c4.1 0 7.625-1.234 9.168-3v14c-1.543-1.766-5.069-3-9.168-3H7a3.988 3.988 0 01-1.564-.317z"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          <p class="text-sm font-medium text-[var(--text-secondary)]">暂无公告</p>
          <p class="text-xs text-[var(--text-muted)]">裁判组发布新公告后会自动出现在这里</p>
        </div>

        <!-- 卡片列表 -->
        <div v-else class="space-y-4 pb-8">
          <article
            v-for="a in announcementStore.announcements"
            :key="a.id"
            class="rounded-xl border bg-white p-5 shadow-sm transition-colors hover:border-slate-300"
            :class="
              announcementStore.isUnread(a.id)
                ? 'border-[var(--color-primary)]/35 ring-1 ring-[var(--color-primary)]/15'
                : 'border-[var(--border-color)]'
            "
          >
            <!-- 标题行：未读圆点 + 标题 -->
            <div class="flex items-start gap-2">
              <span
                v-if="announcementStore.isUnread(a.id)"
                class="mt-1.5 h-2 w-2 shrink-0 rounded-full bg-[var(--color-primary)]"
                title="未读"
              ></span>
              <h2 class="min-w-0 flex-1 text-[15px] font-bold leading-6 text-slate-900">
                {{ a.title }}
              </h2>
            </div>

            <!-- 元信息行 -->
            <p
              class="mt-1.5 text-xs text-slate-500"
              :class="{ 'pl-4': announcementStore.isUnread(a.id) }"
            >
              发布者 {{ a.author }} · {{ formatTime(a.createdAt) }}
            </p>

            <!-- 正文：折叠 + 渐变遮罩 + 展开切换 -->
            <div class="relative mt-3">
              <div
                :ref="setBodyRef(a.id)"
                class="prose overflow-hidden transition-[max-height] duration-300"
                :class="expandedIds.has(a.id) || !overflowIds.has(a.id) ? '' : 'max-h-36'"
                v-html="renderedContents.get(a.id)"
              ></div>
              <div
                v-if="overflowIds.has(a.id) && !expandedIds.has(a.id)"
                class="pointer-events-none absolute inset-x-0 bottom-0 h-14 bg-gradient-to-t from-white to-transparent"
              ></div>
            </div>
            <div v-if="overflowIds.has(a.id)" class="mt-1 flex justify-center">
              <button
                type="button"
                class="inline-flex items-center gap-1 rounded px-3 py-1 text-xs font-medium text-slate-500 transition-colors hover:bg-slate-50 hover:text-[var(--color-primary)]"
                @click="toggleExpand(a.id)"
              >
                <span>{{ expandedIds.has(a.id) ? '收起' : '展开全部' }}</span>
                <svg
                  class="h-3.5 w-3.5 transition-transform"
                  :class="{ 'rotate-180': expandedIds.has(a.id) }"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <path d="M19 9l-7 7-7-7" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </button>
            </div>
          </article>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 公告正文排版：与题面 .prose 同风格（内容来自 renderMarkdown 出口，已消毒） */
.prose {
  font-size: 13px;
  line-height: 1.7;
  color: #475569;
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
  color: #0f172a;
  font-weight: 600;
}
.prose :deep(h1),
.prose :deep(h2),
.prose :deep(h3),
.prose :deep(h4) {
  color: #0f172a;
  font-weight: 600;
  margin: 0.75rem 0 0.5rem;
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
  background: #f1f5f9;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  padding: 0.1rem 0.35rem;
  color: #334155;
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
  border: 1px solid #e2e8f0;
  padding: 0.25rem 0.5rem;
}
.prose :deep(blockquote) {
  border-left: 3px solid #e2e8f0;
  padding-left: 0.75rem;
  color: #64748b;
  margin: 0 0 0.75rem;
}
</style>
