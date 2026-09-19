# SettingsView（设置页）

> 源文件：`src/views/SettingsView.vue`

## 职责

应用配置（`AppConfig`）的可视化编辑入口，五个分组卡片：OJ 服务器 / 编辑器 / 布局 / 主题（只读占位）/ 关于（客户端存储信息）；本地草稿编辑 + 逐字段校验 + dirty 判定，保存经 `configService.updateConfig`（读-改-写整体替换）落盘。OJ 分组含「当前 OJ」下拉：**候选 = 已知 OJ 枚举（`utils/oj`）+ 配置里的其它实例 id**（未配置者标注「（未配置）」）—— 选中尚未配置的类型会引导填地址，保存即创建实例并**自动切换**（用户在选下拉时已表达切换意图）；切换本身是显式动作（`@change` → `onSwitchOj`，即时生效并持久化），与「保存」按钮解耦。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SettingsForm` | interface | 表单草稿；**数字字段保留原始字符串**——输入中间态（空串/半截数字）不应被强转成 NaN 写回，只有校验通过的值才进入保存载荷 |
| `form` | `reactive<SettingsForm>` | activeOj / ojUrl / contestRef / contestPassword / timeoutSecs / pollIntervalSecs / pollTimeoutSecs / cacheTtlSecs / cacheProblemStatement / fontSize / tabSize / defaultLanguage / autoSave / autoSaveIntervalSecs / splitRatio。`activeOj` = 当前 OJ 实例 id（下拉选择），`ojUrl` = 当前选中实例的服务端地址（保存时写回该实例 baseUrl），`contestRef` = 比赛引用（不透明字符串：HOJ 数字 ID / 其它 OJ 资源引用；空 = 未配置） |
| `ojInstances` / `ojOptions` / `persistedActive` / `switchError` / `switchNotice` / `createdOj` | ref/computed | 实例清单（populate 时从 `config.oj.instances` 刷新，保存后也用写回结果同步）/ 下拉候选（`ojSelectOptions`：**已知枚举在前**（含未配置者）+ 枚举外实例追加；禁用者标注「（已禁用）」且不可选）/ 已持久化的当前 OJ（切换失败时回滚下拉显示，保持 UI 与后端一致）/ 切换错误（rose，独立于保存错误）/ **切换引导**（amber：未配置的 OJ 提示「填地址后保存将自动创建并切换」）/ 本次保存新建的实例 id（据此在保存成功后自动完成切换） |
| `onSwitchOj` | `() => Promise<void>` | 切换当前 OJ（显式命令：即时生效 + 持久化 `oj.active` + 发布 `OJSwitched`，经 `configService.switchOj`）；成功后 `form.ojUrl` **跟随新实例地址**（否则表单里仍是旧实例地址，「保存」会把它写进新实例的 `baseUrl` —— 数据损坏；切换前未保存的地址编辑随之丢弃，用户已切换编辑对象）+ `resetSessionForOjSwitch()` 重置会话上下文（旧 OJ 的用户/比赛/题面/提交全部失效，解题页拿旧 `contest.id` 向新 OJ 提交是真实风险）+ `router.replace({ name: 'Login' })`（守卫按新 OJ 会话文件恢复：登录过则无感续用，否则落在登录表单）；失败时回滚 `form.activeOj` 到 `persistedActive` 并写 `switchError` |
| `baseline` / `snapshot` / `dirty` | ref/fn/computed | 基线 = 上次加载/保存成功时的表单 JSON 序列化；dirty 判定与「放弃更改」共用同一快照 |
| `parseIntStrict` | `(raw) => number \| null` | 严格非负整数解析：正则 `^\d+$` 拒绝空串/小数/负号/科学计数法等 `Number()` 会宽容接受的形式 |
| `intError` / `errors` / `isValid` / `canSave` | computed | 逐字段错误映射（ojUrl 须 `http(s)://` 前缀；contestRef 为自由格式字符串，空串 = 未配置，合法不校验；各整数字段带值域：超时 1–120、轮询间隔 1–30、轮询总超时 30–3600、缓存 TTL 0–600、字号 8–32、自动保存间隔 5–300）；canSave = dirty && valid && !saving |
| `TAB_SIZES` / `LANGUAGE_OPTIONS` / `clampRatio` | 常量/fn | Tab 宽度档位（取 `utils/editor.EDITOR_TAB_SIZES`，值域唯一权威）；默认语言四选项；分栏比例钳位到滑杆值域 [0.30, 0.70] 两位小数（与 step 0.01 对齐） |
| `populate` / `load` | fn | 配置 → 表单回填（`activeOj` / `persistedActive` 取 `oj.active`，`ojInstances` 取 `config.oj.instances` 全量清单，`ojUrl` 取当前选中实例地址——active 未命中时回退第一个启用实例兜底竞态，`contestRef` 取 `oj.contestRef`；tabSize 不在档位内回退 4、语言经 `normalizeLanguageId` 归一）；加载前**先 `configService.invalidate()`** |
| `save` / `discard` | fn | 保存：`updateConfig(draft => …)` 把校验通过的表单值写入草稿（**`draft.oj.active` 刻意不写** —— active 只由 `switch_oj` 改写，否则新建实例会出现「配置说 A、运行中的 Registry 还是 B」的静默不一致；`ojUrl` 写回当前选中实例的 `baseUrl`，**实例不存在则创建**（`enabled: true`，支撑「从枚举里启用新 OJ」），`ojUrl` 写回当前选中实例的 `baseUrl`——active 不在实例列表会被 Rust validate 拒绝，而下拉候选即实例清单，正常操作不会出现；`contestRef` trim；contestPassword 空串 → null）。成功后基线前移 + 「已保存」提示 3s。放弃：从基线 JSON 恢复表单 |
| `storage` / `storageFailed` / `loadStorage` | ref/fn | 「关于」区块数据（`systemService.getStorageInfo()`：版本 / 存储目录 / 日志路径）；失败**非致命**，仅该区块降级为「获取失败」 |
| `copyText` / `copiedKey` | fn/ref | 关于区块逐项复制（版本/目录/路径），「已复制」2s 回弹；剪贴板不可用静默忽略 |
| `onSplitInput` / `splitPercent` | fn/computed | 布局滑杆输入（钳位后写回）与百分比文案 |
| `INPUT` / `INPUT_ERROR` / `inputClass` | 常量/fn | 输入框样式拼接（错误态红框） |

## 直接依赖

- `vue`
- 组件：`ErrorMessage` / `LoadingSpinner`
- `@/services/config.service`（`configService` + `normalizeLanguageId`）、`@/services/system.service`（`systemService`）
- `@/types/config` / `@/types/system`（仅类型）
- `@/utils/error`（`errorMessage` —— 错误文案收敛）
- `@/utils/editor`（`EDITOR_TAB_SIZES` / `EDITOR_FONT_SIZE_MIN` / `EDITOR_FONT_SIZE_MAX` —— 编辑器分组的值域唯一权威）

## 被依赖

- `router/index.ts` — 路由 `Settings`（`/contest/settings`，放在工作台外壳内，切换时不丢失顶栏/活动栏/状态条）

## 逻辑流程

```
onMounted → load() + loadStorage()（并行，互不阻塞）

load():
  configService.invalidate()          // 设置页必须展示磁盘真值：其它页面可能已热改过
                                      // 配置（如拖拽分栏条自动保存比例），进程内缓存不可信
  populate(await getConfig())         // 回填表单（activeOj/ojUrl/contestRef 等）+ baseline = snapshot()

编辑表单 → errors 逐字段实时校验 → dirty = snapshot() !== baseline
切换 OJ（下拉 @change → onSwitchOj）
  ├─ 目标尚未配置（枚举里的新类型）→ 清空地址栏 + switchNotice 引导「填地址后保存」；**不调用后端**（无实例可注册，必失败）
  └─ 已配置 → performSwitch(target) → configService.switchOj(form.activeOj)
  ├─ 成功 → persistedActive 前移 + form.ojUrl 跟随新实例地址（防「保存」把旧实例地址写进新实例）
  │         + resetSessionForOjSwitch()（含匿名简报复位：brief 属旧 OJ，canEnter
  │           决策不得被旧时间窗驱动）+ router.replace(Login)
  └─ 失败 → form.activeOj 回滚到 persistedActive + switchError 展示（不静默停留未生效的 OJ）

切换即离开 = 放弃设置页所有未保存的修改（地址/超时/比赛引用等，下拉提示已明示）——
切换 OJ 是「换服务端」的应用级操作，跨越未保存的草稿继续编辑反而制造混淆。
save(): canSave 才执行 → updateConfig(读-改-写整体替换 + 失效缓存)
        → baseline 前移、showSaved 3s；失败写 saveError（表单值保留）
discard(): Object.assign(form, JSON.parse(baseline))

主题分组：界面主题下拉 disabled（整机只有浅色，暗色即将上线）；编辑器主题下拉同样 disabled，
但文案指向「由解题页编辑器设置控制」—— 编辑器主题已可切换（`theme.editorTheme`），
只是入口在解题页弹层，置灰项不得再宣称「固定浅色」
```

设计要点：

- **数字字段以字符串承载**：`type="text"` + `inputmode="numeric"`，避免 `v-model.number`
  把中间态强转 NaN；校验（`parseIntStrict` + 值域）通过后才 `Number()` 进保存载荷。
- **后端 `update_config` 是整体替换语义**：必须经 `configService.updateConfig` 的
  读-改-写路径，直接提交局部字段会把其余配置冲掉。
- 加载前失效缓存与保存后失效缓存（service 内部）闭环，保证「所见 = 磁盘真值」。
- **编辑器分组与解题页弹层同源**：两处写的是同一份 `editor.*` 配置。解题页
  「编辑器设置」弹层是**即时生效**入口（改完立刻作用当前编辑器），设置页是**批量编辑**
  入口（改动对新打开的解题页生效）—— 分组标题旁的提示文案即表达这一分工。
- **OJ 分组新增「题面缓存」开关**：`oj.cacheProblemStatement`（默认开启）。开启时后端按
  `{contest_id}/{display_id}` 做内存 + 磁盘缓存（TTL 30 分钟），切题来回与断网时秒开；
  关闭后每次打开题目都请求服务端（题面被管理员中途修正时可临时关闭）。表单为布尔字段，
  不参与 `intError` 校验（无值域）。
- **OJ 切换与保存解耦**：「当前 OJ」下拉是显式切换动作（即时生效 + 持久化 `oj.active` +
  发布 `OJSwitched`），保存仍负责地址/比赛引用等其余字段（保存也写 `draft.oj.active`，
  两处写入同源同值）。切换失败回滚下拉到已持久化值，避免 UI 停留在一个未生效的 OJ；
  候选 = 配置文件 `oj.instances` 清单。
- ojUrl 修改后需重启客户端生效、contestRef 保存后下次进入赛场生效（HOJ 为数字 ID，
  其它 OJ 为资源引用；留空 = 不自动加载）——提示文案明示生效时机。
- **自动保存开关的文案随落盘语义更新**：定时落盘是「代码从后端内存写到磁盘」的唯一周期
  路径，关闭后只剩切题 / 失焦 / 关窗三处显式落盘（`ProblemSolveView` 与 `main.ts` 编排），
  故提示文案写明这一后果。开关与间隔的改动需**重启客户端**才生效（后端 auto-save 懒启动
  且只读一次配置，见 `doc/problem.md` P74 遗留项）。
- 存储信息每次挂载实时读取（service 不缓存：版本号构建期固定，但存储目录可能随
  用户数据迁移变化）。
- 定时器（savedTimer/copiedTimer）onBeforeUnmount 统一清理。
