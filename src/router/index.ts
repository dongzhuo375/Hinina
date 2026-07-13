import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'

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
    path: '/:pathMatch(.*)*',
    redirect: '/contest',
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

export default router
