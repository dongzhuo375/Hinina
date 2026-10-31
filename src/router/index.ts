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
        // meta **保持继承父级的 requiresAuth: true**（默认受保护）—— 免登录豁免
        // 是守卫里的**条件豁免**（settingsMode && to.name === 'Settings'，管理方
        // 钥匙），不是 meta 声明：无条件豁免会让无钥匙访客直达设置页，且 401 时
        // sessionGuard 因 meta 为 false 不再回登录页（评审 P2）。
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
 *    判据是**合并后的 `to.meta`**：vue-router 沿匹配链把父级 meta 合并进子级
 *    （子路由同键覆盖父路由），`requiresAuth: true` 写在外壳父级即可保护全部
 *    子级；未声明即不受保护（login/catch-all）。合并矩阵由
 *    `__tests__/index.spec.ts` 用真实 routes 锁定。
 * 3. `--settings`（管理方钥匙）：
 *    - **首次导航**落在登录页时直落设置页。无会话时没有任何 UI 路径可达设置页
 *      （隐藏入口在比赛外壳内），钥匙必须在初始导航就生效；仅对首次导航生效，
 *      「返回登录」不受影响。
 *    - **设置页的条件豁免**：`settingsMode && to.name === 'Settings'` 时放行
 *      无会话访问（且不限首次）。豁免在守卫里而不在 meta 里：meta 保持继承
 *      父级保护，普通模式下设置页照常受保护（无钥匙访客直达 → 登录页；页面
 *      上 401 → sessionGuard 读到 requiresAuth=true 照常回登录页）。
 *    - 标志读取失败按「无参数」退化（service 层兜底 + 守卫 try/catch 二道防线）。
 */
let firstNavigation = true

router.beforeEach(async (to) => {
  // 钥匙通道故障不得炸掉导航：service 已 memoize + 失败退化，这里是导航路径上
  // 的第二道防线（守卫抛错 = 整个路由系统卡死在当前页）
  let settingsMode = false
  try {
    settingsMode = (await systemService.getLaunchFlags()).settings
  } catch {
    settingsMode = false
  }
  if (firstNavigation) {
    firstNavigation = false
    if (settingsMode && to.name === 'Login') return { name: 'Settings' }
  }

  const auth = useAuthStore()
  if (!auth.sessionResolved) await auth.checkSession()

  const requiresAuth = to.meta.requiresAuth === true
  if (!requiresAuth) return true
  // 管理方钥匙的条件豁免：仅 --settings 进程中的设置页可无会话进入（不限首次
  // —— 返回登录后想再修一次地址仍可直达）。普通模式设置页照常受保护。
  if (settingsMode && to.name === 'Settings') return true
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
