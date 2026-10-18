<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useAnnouncementStore } from '@/stores/announcementStore'
import { settingsUnlocked } from '@/utils/settings-access'

const route = useRoute()
const announcementStore = useAnnouncementStore()

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

/// 公告未读红点：公告页处于激活态时不显示 —— 进入该页即全部标记已读，
/// 避免已读回写往返期间红点残留造成误导
const showAnnouncementDot = computed(
  () => announcementStore.unreadCount > 0 && activeKey.value !== 'announcements',
)

const links = [
  { key: 'problems', name: 'ProblemSet', label: '题目', number: '01' },
  { key: 'rank', name: 'Rank', label: '榜单', number: '02' },
  { key: 'submissions', name: 'Submissions', label: '评测', number: '03' },
  { key: 'announcements', name: 'Announcements', label: '公告', number: '04' },
]
</script>
<template>
  <nav class="contest-navigation" aria-label="赛场导航">
    <router-link v-for="link in links" :key="link.key" :to="{ name: link.name }"
      :class="['navigation-link', { active: activeKey === link.key }]"
      :aria-current="activeKey === link.key ? 'page' : undefined">
      <span class="navigation-number" aria-hidden="true">{{ link.number }}</span>{{ link.label }}
      <span v-if="link.key === 'announcements' && showAnnouncementDot" class="unread-mark" aria-label="有未读公告"></span>
    </router-link>
    <router-link v-if="settingsUnlocked" :to="{ name: 'Settings' }"
      :class="['navigation-link', { active: activeKey === 'settings' }]"
      :aria-current="activeKey === 'settings' ? 'page' : undefined">环境设置</router-link>
  </nav>
</template>
<style scoped>
.contest-navigation { display: flex; align-items: stretch; gap: 30px; min-width: 0; overflow-x: auto; }
.navigation-link { position: relative; display: flex; align-items: center; gap: 9px; min-height: calc(48px + var(--ui-step) * 4); white-space: nowrap; font-size: calc(13px + var(--ui-step)); color: var(--text-secondary); border-bottom: 2px solid transparent; }
.navigation-number { font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-muted); }
.navigation-link:hover { color: #8c6a43; }
.navigation-link.active { color: var(--text-primary); border-bottom-color: var(--text-primary); font-weight: 600; }
.unread-mark { width: 5px; height: 5px; background: var(--color-error); border-radius: 50%; }
@media (max-width: 1100px) { .contest-navigation { gap: 22px; } }
</style>
