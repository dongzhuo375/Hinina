# SettingsView（设置页）

> 源文件：`src/views/SettingsView.vue`

## 职责

应用配置（`AppConfig`）的可视化编辑入口，五个分组卡片：OJ 服务器 / 编辑器 / 布局 / 主题（只读占位）/ 关于（客户端存储信息）；本地草稿编辑 + 逐字段校验 + dirty 判定，保存经 `configService.updateConfig`（读-改-写整体替换）落盘。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `SettingsForm` | interface | 表单草稿；**数字字段保留原始字符串**——输入中间态（空串/半截数字）不应被强转成 NaN 写回，只有校验通过的值才进入保存载荷 |
| `form` | `reactive<SettingsForm>` | hojUrl / contestId / contestPassword / timeoutSecs / pollIntervalSecs / pollTimeoutSecs / cacheTtlSecs / fontSize / tabSize / defaultLanguage / autoSave / autoSaveIntervalSecs / splitRatio |
| `baseline` / `snapshot` / `dirty` | ref/fn/computed | 基线 = 上次加载/保存成功时的表单 JSON 序列化；dirty 判定与「放弃更改」共用同一快照 |
| `parseIntStrict` | `(raw) => number \| null` | 严格非负整数解析：正则 `^\d+$` 拒绝空串/小数/负号/科学计数法等 `Number()` 会宽容接受的形式 |
| `intError` / `errors` / `isValid` / `canSave` | computed | 逐字段错误映射（hojUrl 须 `http(s)://` 前缀；各整数字段带值域：超时 1–120、轮询间隔 1–30、轮询总超时 30–3600、缓存 TTL 0–600、字号 8–32、自动保存间隔 5–300）；canSave = dirty && valid && !saving |
| `TAB_SIZES` / `LANGUAGE_OPTIONS` / `clampRatio` | 常量/fn | Tab 宽度档位 [2,4,8]；默认语言四选项；分栏比例钳位到滑杆值域 [0.30, 0.70] 两位小数（与 step 0.01 对齐） |
| `populate` / `load` | fn | 配置 → 表单回填（tabSize 不在档位内回退 4、语言经 `normalizeLanguageId` 归一）；加载前**先 `configService.invalidate()`** |
| `save` / `discard` | fn | 保存：`updateConfig(draft => …)` 把校验通过的表单值写入草稿（contestPassword 空串 → null）；成功后基线前移 + 「已保存」提示 3s。放弃：从基线 JSON 恢复表单 |
| `storage` / `storageFailed` / `loadStorage` | ref/fn | 「关于」区块数据（`systemService.getStorageInfo()`：版本 / 存储目录 / 日志路径）；失败**非致命**，仅该区块降级为「获取失败」 |
| `copyText` / `copiedKey` | fn/ref | 关于区块逐项复制（版本/目录/路径），「已复制」2s 回弹；剪贴板不可用静默忽略 |
| `onSplitInput` / `splitPercent` | fn/computed | 布局滑杆输入（钳位后写回）与百分比文案 |
| `INPUT` / `INPUT_ERROR` / `inputClass` | 常量/fn | 输入框样式拼接（错误态红框） |

## 直接依赖

- `vue`
- 组件：`ErrorMessage` / `LoadingSpinner`
- `@/services/config.service`（`configService` + `normalizeLanguageId`）、`@/services/system.service`（`systemService`）
- `@/types/config` / `@/types/system`（仅类型）

## 被依赖

- `router/index.ts` — 路由 `Settings`（`/contest/settings`，放在工作台外壳内，切换时不丢失顶栏/活动栏/状态条）

## 逻辑流程

```
onMounted → load() + loadStorage()（并行，互不阻塞）

load():
  configService.invalidate()          // 设置页必须展示磁盘真值：其它页面可能已热改过
                                      // 配置（如拖拽分栏条自动保存比例），进程内缓存不可信
  populate(await getConfig())         // 回填表单 + baseline = snapshot()

编辑表单 → errors 逐字段实时校验 → dirty = snapshot() !== baseline
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
- hojUrl 修改后需重启客户端生效、contestId 保存后下次进入赛场生效——提示文案明示生效时机。
- 存储信息每次挂载实时读取（service 不缓存：版本号构建期固定，但存储目录可能随
  用户数据迁移变化）。
- 定时器（savedTimer/copiedTimer）onBeforeUnmount 统一清理。
