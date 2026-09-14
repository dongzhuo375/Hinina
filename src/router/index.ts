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
    path: '/contest',
    name: 'Contest',
    component: () => import('@/views/ContestView.vue'),
    meta: { title: 'Hinina', requiresAuth: true },
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
 */
router.beforeEach(async (to) => {
  const auth = useAuthStore()
  if (!auth.sessionResolved) await auth.checkSession()

  if (!to.meta.requiresAuth) return true
  return auth.isLoggedIn ? true : { name: 'Login', replace: true }
})

export default router
