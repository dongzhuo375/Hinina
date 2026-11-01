# keyed-settings-policy（`--settings` 豁免策略）

> 源文件：`src/guards/keyed-settings-policy.ts`

## 职责

「管理方钥匙」豁免规则的**唯一判定点**（评审二十四 #4）。历史上该规则在路由守卫、
sessionGuard、ContestLayout、useSettingsForm、LoginView、ActivityBar 等约 7 处各自
内联、各自注释，多轮评审反复修补——本模块把两条判据收敛为一处，消费方一律经它取。

## 核心类型/函数

| 名称 | 签名 | 语义 |
|------|------|------|
| `isKeyedProcess` | `() => boolean` | **进程级**：本次启动是否带 `--settings`。用于分层锁、登录页「返回设置」、ActivityBar 恒显入口 |
| `isKeyedSettingsRoute` | `(routeName: string \| symbol \| null \| undefined) => boolean` | **路由级**：钥匙模式 **且** 该路由是设置页。设置页全部豁免（免登录进入、401 不跳登录、阶段闸门拦回、OJ 切换停留、导航落地不变量）的共同判据 |

两者都委托 `systemService`（`isKeyed()` / `isKeyedSettings()`），判据读取的仍是
`launchFlags` 的同步快照。

## 直接依赖

- `services/system.service`（`isKeyed` / `isKeyedSettings`）

## 被依赖

- `router/index.ts`（beforeEach 的免登录与阶段闸门、afterEach 落地不变量）
- `guards/sessionGuard.ts`（401 不跳登录）
- `views/ContestLayout.vue`（最小壳与阶段闸门短路）
- `composables/useSettingsForm.ts`（OJ 切换后的「停留」与持久化失败分支）
- `views/SettingsView.vue` / `views/LoginView.vue` / `components/layout/ActivityBar.vue`（进程级）

## 逻辑流程

```
isKeyedProcess()            = systemService.isKeyed()
isKeyedSettingsRoute(name)  = systemService.isKeyedSettings(name)
```

**协议**：豁免只属于设置页。新增「钥匙模式下某处也照做」的逻辑时，先在本模块加具名
函数（或复用已有函数）并写清与豁免的关系，不要就地拼 `route.name === 'Settings'`。

## 测试

无独立单测：两个函数是透明委托，真正判据由 `services/__tests__/system.service.spec.ts`
直接覆盖（`isKeyedSettings` 的设置×路由矩阵、未填充快照默认值等）；各消费方的行为由其
自身 spec/e2e 锁定。
