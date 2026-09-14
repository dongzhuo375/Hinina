<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()

/// 当前高亮项：/contest/problems 与 /contest/problem/:displayId 均归属「题目」
const activeKey = computed(() => {
  const p = route.path
  if (p.startsWith('/contest/problem')) return 'problems'
  if (p.startsWith('/contest/rank')) return 'rank'
  if (p.startsWith('/contest/submissions')) return 'submissions'
  if (p.startsWith('/contest/announcements')) return 'announcements'
  if (p.startsWith('/contest/settings')) return 'settings'
  return ''
})

/// 导航项样式：激活=浅紫底 + 紫色文字，悬停=中性浅底
function itemClass(key: string): string {
  return activeKey.value === key
    ? 'relative flex h-12 w-12 flex-col items-center justify-center rounded-xl border border-[#ede9fe] bg-[#f5f3ff] text-[#6845f5] shadow-sm transition'
    : 'relative flex h-12 w-12 flex-col items-center justify-center rounded-xl text-[var(--text-secondary)] transition hover:bg-[var(--bg-sidebar)] hover:text-[var(--text-primary)]'
}
</script>

<template>
  <nav
    class="flex w-16 shrink-0 flex-col items-center justify-between border-r border-[var(--border-color)] bg-[var(--bg-card)] py-3"
  >
    <!-- 主导航：题目 / 榜单 / 评测 / 公告 -->
    <div class="flex w-full flex-col items-center gap-3">
      <router-link :to="{ name: 'ProblemSet' }" :class="itemClass('problems')" title="题目列表 (Problemset)">
        <svg class="mb-0.5 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
          ></path>
        </svg>
        <span class="scale-90 text-[10px] font-semibold">题目</span>
        <!-- 激活指示条：贴活动栏左缘的 3px 紫条 -->
        <span
          v-if="activeKey === 'problems'"
          class="absolute -left-2 bottom-3 top-3 w-[3px] rounded-r bg-[var(--color-primary)]"
        ></span>
      </router-link>

      <router-link :to="{ name: 'Rank' }" :class="itemClass('rank')" title="实时榜单 (Scoreboard)">
        <svg class="mb-0.5 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
          ></path>
        </svg>
        <span class="scale-90 text-[10px] font-semibold">榜单</span>
        <span
          v-if="activeKey === 'rank'"
          class="absolute -left-2 bottom-3 top-3 w-[3px] rounded-r bg-[var(--color-primary)]"
        ></span>
      </router-link>

      <router-link
        :to="{ name: 'Submissions' }"
        :class="itemClass('submissions')"
        title="我的提交 (Submissions)"
      >
        <svg class="mb-0.5 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
          ></path>
        </svg>
        <span class="scale-90 text-[10px] font-semibold">评测</span>
        <span
          v-if="activeKey === 'submissions'"
          class="absolute -left-2 bottom-3 top-3 w-[3px] rounded-r bg-[var(--color-primary)]"
        ></span>
      </router-link>

      <!-- 公告：红点徽标待公告接口接入后再显示，本轮不画假徽标 -->
      <router-link
        :to="{ name: 'Announcements' }"
        :class="itemClass('announcements')"
        title="比赛公告 (Announcements)"
      >
        <svg class="mb-0.5 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
          ></path>
        </svg>
        <span class="scale-90 text-[10px] font-semibold">公告</span>
        <span
          v-if="activeKey === 'announcements'"
          class="absolute -left-2 bottom-3 top-3 w-[3px] rounded-r bg-[var(--color-primary)]"
        ></span>
      </router-link>
    </div>

    <!-- 底部：设置 -->
    <div class="flex w-full flex-col items-center">
      <router-link
        :to="{ name: 'Settings' }"
        class="flex h-10 w-10 items-center justify-center rounded-lg transition"
        :class="
          activeKey === 'settings'
            ? 'bg-[#f5f3ff] text-[#6845f5]'
            : 'text-[var(--text-muted)] hover:bg-[var(--bg-sidebar)] hover:text-[var(--text-primary)]'
        "
        title="环境设置"
      >
        <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
          ></path>
          <path
            d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
          ></path>
        </svg>
      </router-link>
    </div>
  </nav>
</template>
