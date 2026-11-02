import { createRouter, createWebHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import { useAuthStore } from '@/stores/authStore'
import { useContestStore } from '@/stores/contestStore'
import { systemService } from '@/services/system.service'
import { markSettingsGateBlocked } from '@/utils/settings-gate-notice'
import { isKeyedSettingsRoute } from '@/guards/keyed-settings-policy'
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
        // 是守卫里的**条件豁免**（isKeyedSettingsRoute(to.name)，管理方
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
 *    - **首次导航**落在登录页时直落设置页。无会话时隐藏入口在比赛外壳内不可达，
 *      登录页的「返回设置」也要点一次——钥匙在初始导航就落位，省一步且意图明确；
 *      仅对首次导航生效，「返回登录」不受影响。
 *    - **设置页的条件豁免**：`isKeyedSettingsRoute(to.name)` 时放行
 *      无会话访问（且不限首次）。豁免在守卫里而不在 meta 里：meta 保持继承
 *      父级保护，普通模式下设置页照常受保护（无钥匙访客直达 → 登录页；页面
 *      上 401 → sessionGuard 读到 requiresAuth=true 照常回登录页）。
 *    - 标志读取失败按「无参数」退化（service 层兜底 + 守卫 try/catch 二道防线）。
 */
let firstNavigation = true

router.beforeEach(async (to, from) => {
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
  if (isKeyedSettingsRoute(to.name)) return true
  // 钥匙设置页出发的导航：**未登录与赛程闸门失败**拦回设置页（评审十九 #2、
  // 评审二十补漏）——无会话时点赛场页面若送去登录页，设置视图被卸载、草稿
  // 丢失，正是钥匙要避免的。导航**中途**会话失效不在此列：那种情况由
  // `afterEach` 的落地不变量回登录页（有意的安全兜底，见下），不在这里回设置页。
  // 来源路由在**导航入口**一次性捕获（评审二十一 #5）：异步等待后不再读
  // `router.currentRoute`——那可能已被在途导航改写（评审九/十九的竞态根源）
  const fromKeyedSettings = isKeyedSettingsRoute(from.name)
  if (!auth.isLoggedIn) {
    if (fromKeyedSettings) {
      markSettingsGateBlocked('未登录，已留在设置页（登录后才能进入赛场页面）')
      return { name: 'Settings' }
    }
    return { name: 'Login', replace: true }
  }

  // 首次进入工作台（含深链）先确认赛程；赛前/赛后在入口等待。
  // 工作台内切换页面由外壳的时钟处理，避免结束弹窗期间打断其退出流程。
  // **钥匙设置页出发的导航不跳过闸门**（评审十 #1）：否则赛前/赛后点题目/榜单
  // 会落进被闸门锁定的比赛页，随后外壳 watcher 把维护者踢回登录页——草稿丢失、
  // 维护被打断（登录页的「返回设置」是一次重新进入）。闸门失败时回**设置页**。
  const enteringFromOutside =
    fromKeyedSettings || !from.matched.some((r) => r.name === 'Contest')
  if (enteringFromOutside) {
    const contest = useContestStore()
    if (!contest.brief) await contest.loadBrief()
    // 会话可能在 loadBrief 期间失效（例如一次 OJ 切换完成并清理了会话，
    // 评审十九 #2）：钥匙流程同样回设置页，而不是送去登录页
    if (!auth.isLoggedIn) {
      if (fromKeyedSettings) {
        // 与未登录分支一致：拦回时给一次可见反馈（评审二十六 P3）
        markSettingsGateBlocked('会话已失效，已留在设置页（请重新登录后进入赛场页面）')
        return { name: 'Settings' }
      }
      return { name: 'Login', replace: true }
    }
    const schedule = getAuthoritativeSchedule(contest.contest, contest.brief)
    if (getContestPhase(schedule, Math.floor(Date.now() / 1000)) !== 'running') {
      if (fromKeyedSettings) {
        // 拦回设置页时给一次可见反馈（NavigationFailure 本身静默，评审十五 B）。
        // 简报缺失 = 服务器可能不可达——正是钥匙要修的场景，不能说「比赛未进行」
        //（评审十九 #3）
        markSettingsGateBlocked(
          contest.brief
            ? '比赛未进行，已留在设置页（比赛期间才能进入赛场页面）'
            : '未能加载比赛信息（服务器可能不可达），已留在设置页',
        )
        return { name: 'Settings' }
      }
      return { name: 'Login', replace: true }
    }
  }
  return true
})

/**
 * 导航**落地后**的会话不变量（评审九 P2-2）。
 *
 * `beforeEach` 只能检查导航开始时的会话；认证状态可能在导航中途变化：守卫
 * 批准后、路由确认前，某个在途请求返回 401 → sessionGuard 清理会话。此时它
 * 读到的 `currentRoute` 仍是**尚未确认**的旧路由——若旧路由是设置页，钥匙
 * 豁免会跳过跳转，于是豁免被这条在途的受保护导航「继承」，用户落到无会话的
 * 比赛页。这里在确认后补一次：受保护路由且无会话、又不是钥匙设置页 → 回登录页
 *（与守卫同一判据，避免两处漂移）。登录页/catch-all 无 requiresAuth，不会成环。
 */
router.afterEach((to) => {
  if (to.meta.requiresAuth !== true) return
  const auth = useAuthStore()
  if (auth.isLoggedIn) return
  if (isKeyedSettingsRoute(to.name)) return
  void router.replace({ name: 'Login' })
})

export default router
