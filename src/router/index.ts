import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { systemService } from '@/services/system.service'
import { getAuthoritativeSchedule, getContestPhase } from '@/utils/contest'

/// 路由元信息类型声明，避免 `to.meta.*` 退化为 any
declare module 'vue-router' {
  interface RouteMeta {
    title?: string
    requiresAuth?: boolean
  }
}

export const routes: RouteRecordRaw[] = [
  {
    path: '/login',
    name: 'Login',
    component: () => import('@/views/LoginView.vue'),
    meta: { title: '登录 - Hinina' },
  },
  {
    // 比赛工作台外壳：顶栏 + 活动栏 + 多视图 + 状态条
    path: '/contest',
    name: 'Contest',
    component: () => import('@/views/ContestLayout.vue'),
    redirect: { name: 'ProblemSet' },
    meta: { title: 'Hinina', requiresAuth: true },
    children: [
      {
        path: 'problems',
        name: 'ProblemSet',
        component: () => import('@/views/ProblemSetView.vue'),
        meta: { title: '题目总览 - Hinina' },
      },
      {
        path: 'problem/:displayId',
        name: 'ProblemSolve',
        component: () => import('@/views/ProblemSolveView.vue'),
        meta: { title: '解题 - Hinina' },
      },
      {
        path: 'rank',
        name: 'Rank',
        component: () => import('@/views/RankView.vue'),
        meta: { title: '实时榜单 - Hinina' },
      },
      {
        path: 'submissions',
        name: 'Submissions',
        component: () => import('@/views/SubmissionsView.vue'),
        meta: { title: '评测 - Hinina' },
      },
      {
        // 提交详情（测试点/代码/错误信息）；从评测页表格或解题页「最新记录」进入
        path: 'submissions/:submitId',
        name: 'SubmissionDetail',
        component: () => import('@/views/SubmissionDetailView.vue'),
        meta: { title: '提交详情 - Hinina' },
      },
      {
        path: 'announcements',
        name: 'Announcements',
        component: () => import('@/views/AnnouncementsView.vue'),
        meta: { title: '公告 - Hinina' },
      },
      {
        // 设置属于工作台的一部分：放在外壳内，切换时不丢失顶栏/活动栏/状态条。
        // requiresAuth: false 是「管理方钥匙」的另一半：守卫按合并后的 meta 取
        // **最深路由**的值，子级 false 豁免父级 true —— 无会话时只有 --settings
        // 的初始导航会落在这里（守卫直落）；隐藏入口只对已登录会话可见。
        path: 'settings',
        name: 'Settings',
        component: () => import('@/views/SettingsView.vue'),
        meta: { title: '设置 - Hinina', requiresAuth: false },
      },
    ],
  },
  {
    // 统一以登录页作为入口：由 LoginView 恢复会话并根据比赛阶段决定是否进入赛场
    path: '/:pathMatch(.*)*',
    redirect: '/login',
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

/**
 * 会话守卫。
 *
 * 1. 首次导航前统一向后端恢复会话（`get_session`）：既用于受保护路由鉴权，
 *    也让登录页首帧就渲染出正确的会话状态，避免"先显示登录表单再跳转"的闪烁。
 * 2. `meta.requiresAuth` 路由必须持有有效会话，否则重定向登录页 ——
 *    保证登出后（即使组件自身跳转失败）也不会滞留在比赛页面。
 *    判据是**合并后的 `to.meta`**：vue-router 沿匹配链把父级 meta 合并进子级
 *    （子路由同键覆盖父路由），因此 `requiresAuth: true` 写在外壳父级即可保护
 *    全部子级，而设置子级的 `requiresAuth: false` 能豁免父级 —— 判据取最深层
 *    的值（`=== true`，未声明即不受保护）。合并矩阵由 `__tests__/index.spec.ts`
 *    用真实 routes 锁定。
 * 3. `--settings`（管理方钥匙）：**首次导航**落在登录页时直落设置页。无会话时
 *    没有任何 UI 路径可达设置页（隐藏入口在比赛外壳内），钥匙必须在初始导航
 *    就生效；仅对首次导航生效，「返回登录」不受影响。标志读取失败按「无参数」
 *    退化（service 层兜底）。
 */
let firstNavigation = true

router.beforeEach(async (to) => {
  const flags = await systemService.getLaunchFlags()
  if (firstNavigation) {
    firstNavigation = false
    if (flags.settings && to.name === 'Login') return { name: 'Settings' }
  }

  const auth = useAuthStore()
  if (!auth.sessionResolved) await auth.checkSession()

  const requiresAuth = to.meta.requiresAuth === true
  if (!requiresAuth) return true
  if (!auth.isLoggedIn) return { name: 'Login', replace: true }

  // 首次进入工作台（含深链）先确认赛程；赛前/赛后在入口等待。
  // 工作台内切换页面由外壳的时钟处理，避免结束弹窗期间打断其退出流程。
  if (!router.currentRoute.value.matched.some((r) => r.name === 'Contest')) {
    const contest = useContestStore()
    if (!contest.brief) await contest.loadBrief()
    if (!auth.isLoggedIn) return { name: 'Login', replace: true }
    const schedule = getAuthoritativeSchedule(contest.contest, contest.brief)
    if (getContestPhase(schedule, Math.floor(Date.now() / 1000)) !== 'running') {
      return { name: 'Login', replace: true }
    }
  }
  return true
})

export default router
