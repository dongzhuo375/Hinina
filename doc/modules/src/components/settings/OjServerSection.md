# OjServerSection（OJ 服务器分组）

> 源文件：`src/components/settings/OjServerSection.vue`

## 职责

展示 OJ 实例、地址、比赛引用、超时、轮询、缓存和题面缓存字段，并上报草稿补丁与 OJ 切换意图。保存和切换事务仍由 `useSettingsForm` 统一执行，本组件不直接调用 service/store/router。

**字段级分层锁**：`locked` 时禁用连接与缓存调优字段（当前 OJ 下拉、服务器地址、请求超时、轮询间隔、轮询总超时、列表缓存 TTL、题面缓存开关），并在区块头部显示一行锁提示；**比赛 ID/引用与比赛密码不受影响**（公开分发模式下这是自助装机用户唯一能填这两个值的地方，锁了选手连比赛都进不去）。提权唯一来源是 `--settings` 启动参数。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `form` | prop `SettingsForm` | 中央 reactive 草稿；文本/选择用 `v-model` 编辑同一对象，不替换对象身份 |
| `errors` | prop `Record<string,string \| null>` | URL 与数字字段错误；模板直接读 `props.errors`，保证 computed 替换后仍更新 |
| `options` | prop `ReturnType<typeof ojSelectOptions>` | OJ 下拉候选；模板直接读 `props.options`，保存新实例后 configured 标签实时更新 |
| `switchError` / `switchNotice` | prop `string \| null` | 切换错误与引导提示，直接读 `props` 保持响应式 |
| `inputClass` | prop fn | 共享输入框错误态样式；基础样式直接 import `SETTINGS_INPUT` |
| `locked` | prop `boolean`（默认 false） | 维护者层锁定：7 个连接/缓存字段 `:disabled` + 标签置灰（沿自动保存间隔的 disabled 先例）+ 头部锁提示 |
| `update:form` | emit `SettingsFormPatch` | switch 按钮的局部草稿补丁；文本/选择用 `v-model` 直接编辑同一 reactive 草稿并保留 IME composition |
| `switch-oj` | emit | `activeOj` v-model 更新后通知父级执行切换（locked 时 select disabled，不可达） |

## 直接依赖

- `SettingsSectionFrame`
- `@/utils/oj`（`ojBaseUrlHint` / `ojSelectOptions` 类型）
- `@/composables/useSettingsForm`（`SETTINGS_INPUT` 常量和类型）

## 被依赖

- `views/SettingsView.vue`

## 逻辑流程

文本/选择字段用 `v-model` 直接编辑同一 reactive 草稿：`activeOj` 落草稿后 `@change` 只 emit `switch-oj`，不发补丁；只有题面缓存开关 emit `update:form` 补丁（locked 时按钮 disabled，不可达）。未配置 OJ 的引导、失败回滚和保存新建实例后的自动切换全部留在 `useSettingsForm`。锁定的字段不可变脏 → 保存事务整体写回等值无害，无需在事务层重复设防。

## 测试

由 `useSettingsForm` 单测和 `SettingsView.behavior.spec.ts` 的组合根测试覆盖：保存、创建后自动切换、已配置切换失败回滚及错误提示；分层锁的禁用矩阵（维护者字段 disabled、比赛 ID/密码可编辑、锁提示、`--settings` 解锁）在行为表征 spec 锁定。
