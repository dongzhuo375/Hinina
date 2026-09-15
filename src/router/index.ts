import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import { useAuthStore } from '@/stores/authStore'

/// 路由元信息类型声明，避免 `to.meta.*` 退化为 any
declare module 'vue-router' {
  interface RouteMeta {
    title?: string
    requiresAuth?: boolean
  }
}

const routes: RouteRecordRaw[] = [
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
        // 设置属于工作台的一部分：放在外壳内，切换时不丢失顶栏/活动栏/状态条
        path: 'settings',
        name: 'Settings',
        component: () => import('@/views/SettingsView.vue'),
        meta: { title: '设置 - Hinina' },
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
 *    注意：子路由不会继承父路由的 meta，需检查整条匹配链（`to.matched`）。
 */
router.beforeEach(async (to) => {
  const auth = useAuthStore()
  if (!auth.sessionResolved) await auth.checkSession()

  const requiresAuth = to.matched.some((r) => r.meta.requiresAuth)
  if (!requiresAuth) return true
  return auth.isLoggedIn ? true : { name: 'Login', replace: true }
})

export default router
