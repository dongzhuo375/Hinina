# CodeEditor（Monaco 代码编辑器）

> 源文件：`src/components/editor/CodeEditor.vue`

## 职责

封装 Monaco Editor（浅色 `vs` 主题）：代码双向绑定、**浮层化语言下拉**（Teleport 到工作台外壳 + fixed 定位 + 动态最大高度/上下翻转）、Ctrl+Enter 提交快捷键、清空（二次确认）/ 上传（原生 input）工具条、自动备份指示与光标位置上报，并向父级暴露 `focus()`。

## 核心类型/函数

**props**：`modelValue: string`（代码）、`language: string`（**HOJ 显示名**，Monaco id 经 `utils/language.monacoIdOf` 派生）、`languages?: string[]`（本题允许的提交语言列表，来自题目详情；空则回退内置默认）、`isDirty: boolean`（驱动备份指示）、`readonly?: boolean`（只读模式：隐藏工具条、禁用编辑与 Ctrl+Enter 提交快捷键，供提交详情页代码查看复用）、`locked?: boolean`（**加载锁定**，PR21-8：Monaco 只读但**工具条保留** —— 与 readonly 的「隐藏工具条」语义分立；随加载完成自动解锁，解锁时以 modelValue 回写自愈）。
**emits**：`update:modelValue`、`update:language`、`submit`、`cursor: [{ line, column }]`、`prefs-change: [EditorPrefs]`（挂载读配置后 + 每次弹层改动后上报，供父级同步状态行等派生展示）。
**expose**：`focus()` —— 供 `?focus=1` 快捷提交联动程序化聚焦；Monaco 未就绪时静默降级为 no-op，不抛错不阻塞。

| 名称 | 用途 |
|------|------|
| `MonacoEnvironment.getWorker` | 手动配置 5 个 worker（editor/ts/css/html/json，Vite `?worker` 导入），避免 worker 打包问题 |
| `editor: shallowRef<IStandaloneCodeEditor>` | Monaco 实例用 **shallowRef**：编辑器实例巨大且自带内部状态，深层响应式代理既昂贵又可能破坏其内部引用 |
| `availableLanguages` | computed | 语言下拉候选：**服务端列表存在时原样呈现、不补入当前语言**（列表外语言可见可选只会换来被拒的提交；归位由 ProblemSolveView 加载后统一做）；仅列表未提供（未加载/未返回）时回退 `DEFAULT_LANGUAGES` 并补入当前语言保持可见 |
| `suppressChangeEmit` | 外部改写代码（切题加载/清空/上传）时 `setValue` 会触发 change 事件，此标志抑制回流，避免把程序化写入误标为「用户编辑（dirty）」 |
| `langTrigger` / `langMenu` | `ref<HTMLButtonElement>` / `ref<HTMLDivElement>` | 语言按钮与语言菜单元素引用（定位基准 / 打开后聚焦当前项） |
| `langMenuTarget` | `shallowRef<HTMLElement \| null>` | 菜单的 Teleport 目标：`langTrigger.closest('.contest-theme') ?? document.body` —— 避开编辑器容器的 `overflow` 与快捷提交卡片的裁切，同时继承赛场主题变量 |
| `langMenuStyle` | `ref<Record<string, string>>` | 计算出的 fixed 定位样式 |
| `positionLangMenu` | `() => void` | 定位算法：`margin = 12`、`gap = 4`；`below = innerHeight - rect.bottom - gap - margin`、`above = rect.top - gap - margin`；`opensBelow = below >= Math.min(320, above)`；`width = min(max(192, rect.width), innerWidth - margin * 2)`；`left` 夹在 `[margin, innerWidth - width - margin]`；`maxHeight = max(0, min(320, opensBelow ? below : above))`；输出 `top` 或 `bottom`。挂 `window resize` 重定位（onUnmounted 摘除） |
| `toggleLangMenu` | `async () => void` | 切换开合；打开时设 Teleport 目标 → 定位 → `await nextTick()` → 聚焦菜单内 `[aria-current="true"]` 的当前语言按钮 |
| `handleKeydown` | window 级监听 Ctrl/Cmd+Enter → emit submit；onUnmounted 移除 |
| `handleFileChange` | 原生 `input[type=file]` + `file.text()` 读取上传代码（**不引入 Tauri dialog 插件**）；读后立即重置 `input.value` 允许连续选同一文件；**按扩展名自动识别语言**（`hojLanguageOfFileName` → `resolveAllowedLanguage` 按语言族解析为允许列表中的服务端原名，"Python3" 等变体也能命中），无命中不切换（切到不允许的语言只会换来一次提交失败） |
| `prefs` | `reactive<EditorPrefs>` | 当前偏好（字号 / Tab 宽度 / 主题）：挂载时经 `applyLoadedPrefs` 并入配置值，弹层改动就地更新（**不重读配置**，避免与在途写入打架） |
| `touched` | `Set<keyof EditorPrefs>` | 用户**已改动过**的偏好字段。挂载读配置到达前动过的字段以用户值为准；落盘只写这些字段 —— 未触碰字段保持磁盘原值，避免把存量配置抹成默认值 |
| `applyLoadedPrefs` | `(loaded: EditorPrefs) => void` | 逐字段并入配置读到的偏好，**跳过已触碰字段**（读取失败时 service 已返回兜底值，同样只并入未触碰字段） |
| `handlePrefsChange` | `(patch: Partial<EditorPrefs>) => void` | 弹层变更入口：登记 `touched` → 更新 `prefs` → `applyPrefs()` 即时生效 → `emit('prefs-change')` → 400ms debounce 落盘 |
| `touchedPatch` | `() => Partial<EditorPrefs>` | 由 `touched` 组装落盘载荷（只含用户改过的字段） |
| `applyPrefs` | `fn` | `editor.updateOptions({ fontSize, tabSize })` + `monaco.editor.setTheme(editorTheme)`；**主题是全局选项**（Monaco 无按实例主题），同页其它编辑器一并跟随；实例未就绪时直接返回 |
| `resetPrefs` | `fn` | 恢复默认值（`utils/editor` 常量）：与手动改动同一条链路（即时生效 + 落盘），不是只改本地内存 |
| `persistPrefs` | `async fn` | `configService.updateEditorPrefs(touchedPatch())` —— **只写用户改过的字段**（部分写语义；载荷为空则直接返回）；失败置 `prefsError`（「保存失败，设置仅本次会话生效」）但**不回滚已应用的值** |
| `prefsError` | ref | 落盘失败提示，透传给 `EditorSettingsPopover` 的 `error`（成功路径不展示任何状态文案） |
| `editorSurfaceClass` | computed | 容器底色跟随主题（`vs-dark` → `#1e1e1e`，否则 `var(--bg-card)`）：Monaco 实例创建前与尺寸重算瞬间不露白底。**编辑器背景本身由 Monaco 主题绘制** —— `styles/global.css` 刻意不再用 `!important` 覆写 `.margin` / `.monaco-editor-background`，否则 vs-dark 只换字色、底色仍被钉在浅色 |
| 清空确认面板 | 模板 | `v-focus-scope="() => { confirmClearOpen = false }"` + `role="dialog" aria-label="确认清空代码"`：键盘用户 Esc 可取消，焦点不会逃出面板 |

编辑器配置：`theme` = 配置 `theme.editorTheme`（弹层可选 `vs` / `vs-dark`，**仅编辑器区域**；客户端界面仍只有浅色）、**fontSize / tabSize / editorTheme 挂载时经 `configService.getEditorPrefs()` 读取**（读取失败服务内部回退 14 / 4 / 'vs'）、JetBrains Mono 字体栈（`"JetBrains Mono Variable", "JetBrains Mono", "Noto Sans SC Variable", "Cascadia Code", Consolas, monospace`）、minimap 关闭、wordWrap on、automaticLayout true（容器尺寸变化自适应，配合可拖拽分栏）、`readOnly` 跟随 `readonly || locked`（只读时行高亮关闭；locked 响应式 —— watcher 经 `updateOptions` 翻转，解锁时 `getValue() !== modelValue` 则按 modelValue 回写，抑制 change 回流）。

工具条容器为 `editor-toolbar flex min-h-12 flex-wrap gap-y-2`：允许换行，窄分栏下语言按钮与设置入口不会被挤出可视区。

## 直接依赖

- `vue`
- `monaco-editor`（生产源码保持裸包导入；Vitest 在测试配置中用 alias 指向包自带 ESM 入口，含 5 个 `?worker` 导入）
- `@/components/editor/EditorSettingsPopover.vue`（工具条设置弹层）
- `@/services/config.service`（编辑器偏好读写 + `EditorPrefs` 类型）
- `@/utils/language`（语言值域/文件名识别）
- `@/utils/editor`（偏好默认值）
- `@/utils/focus-scope`（`vFocusScope` —— 语言菜单与清空确认面板的焦点陷阱 / Esc 关闭）
- `@/utils/logger`（`createLogger` —— 作用域日志）

## 被依赖

- `views/ProblemSolveView.vue` — 右栏编辑器（`v-model` 绑 `workspaceStore.code`，语言切换绑 `workspaceStore.changeLanguage`，cursor 转发给 EditorConsoleBar）
- `views/SubmissionDetailView.vue` — 只读代码查看（readonly 模式）
- `components/problem/QuickSubmitDialog.vue` — 快捷提交对话框内的代码输入

## 逻辑流程

```
onMounted → nextTick → 检查容器 → loadedPrefs = await configService.getEditorPrefs()
                     → 再次读取并检查容器（配置等待期间可能已卸载）
                     → applyLoadedPrefs(loadedPrefs) → emit prefs-change
                     → monaco.editor.create(container, {...})
  onDidChangeModelContent → （非抑制时）emit update:modelValue → workspaceStore.updateCode
                             → 标脏 + 2s 防抖同步到 Rust 后端
  onDidChangeCursorPosition → emit cursor → 父级转发 EditorConsoleBar 状态行

watch props.modelValue → 与编辑器值不同才 setValue（抑制 change 回流）
watch props.language → monaco.editor.setModelLanguage
watch readonly/locked → editor.updateOptions({ readOnly })；解锁时 getValue ≠ modelValue
                       → setValue（抑制回流，锁定窗口漏进输入的自愈回写）
语言下拉选择 → emit update:language（父级走 workspaceStore.changeLanguage：
               本地乐观更新 + set_workspace_language 立即持久化）
语言菜单（浮层）→ toggleLangMenu：Teleport 到 .contest-theme|body → fixed 定位（可上下翻转、
               最大高度 ≤320px）→ nextTick 聚焦当前语言项；v-focus-scope 接管 Tab/Esc；
               window resize → positionLangMenu 重定位
EditorSettingsPopover change/reset → handlePrefsChange → applyPrefs（即时生效）
                → emit prefs-change → 400ms debounce → configService.updateEditorPrefs(touchedPatch())
onUnmounted → editor.dispose() + 移除 keydown 监听 + 在途偏好改动补一次落盘
```

设计要点：

- **locked 与 readonly 语义分立**：`readonly` 是终态只读（详情页查看，隐藏工具条、不注册
  提交快捷键）；`locked` 是瞬态锁定（切题加载期间，PR21-8），工具条保留、解锁即恢复。
  locked 置位到 Monaco `updateOptions` 生效之间有一个渲染 tick 窗口，漏进 Monaco 的输入
  会被 workspaceStore 的守卫拒收（store.code 不变 → prop 不变 → modelValue watcher 不
  触发），解锁时按 modelValue 回写自愈，保证「Monaco === store」不变式；加载成功时该
  回写与 modelValue watcher 的 setValue 幂等重合（值已一致，直接跳过）。
- **根节点是 flex 项（`flex-1`），消费方必须把它放进 flex 容器**：父级需自身
  `display:flex` + 确定高度（如 `flex h-[520px] flex-col overflow-hidden`）。
  放进普通块级父容器时 `flex-1` 不生效、高度退回内容高度，Monaco 会塌缩成数像素高
  —— 界面表现为「代码一片空白」，而数据其实是好的（提交详情页曾踩此坑，实测 5px）。
- **备份指示的语义**：`isDirty=true` 显示灰色「编辑中…」，`false` 显示绿色「已自动备份」——
  状态来自 workspaceStore（防抖同步 + 后端 auto-save 落盘后清脏），组件只呈现。
- 清空代码必须二次确认（气泡内确认/取消）：赛场上误删代码是不可逆事故。
- Ctrl+Enter 挂 window 而非编辑器内 keybinding：焦点在题面/工具条时也能提交。
- 图标内联 SVG；不引入外部字体（字体栈回退到系统等宽字体）。
- **编辑器偏好是应用级配置**：弹层改动即时应用到本实例 + debounce 落盘（与设置页同一份
  数据），因此设置页标注的「对新打开的解题页生效」在本弹层被突破 —— 这里是即时生效；
  落盘失败不回滚，只提示「仅本次会话生效」。
- **在途改动不丢**：卸载时若 debounce 未到期，补一次落盘 —— 调完字号立刻切题/离页
  不该丢设置。
- **异步挂载有两个容器检查点**：首次 `nextTick` 后检查覆盖模板挂载窗口；偏好 IPC 返回后必须
  重新读取并复查容器，覆盖秒开秒关窗口。若此处仍用 await 前的 ref，可能 `create(null)`，
  或在卸载钩子已跑完后创建无人 dispose 的 editor 并重新挂上 keydown listener。
- **语言菜单必须浮层化**（PR #36）：HOJ 的语言列表可能很长（十几项），原来 `absolute`
  挂在按钮下方的菜单会被编辑器容器的 `overflow-hidden` 与快捷提交卡片的裁切吃掉，
  表现为「菜单看不见 / 滚不动 / 选不到后面的语言」。现改为 **Teleport 到 `.contest-theme`
  外壳 + `position: fixed` + 动态 `maxHeight`（≤320px）与上下翻转**：空间不足时向上展开，
  空间足够时向下展开，超出部分由 `overflow-y-auto overscroll-contain` 滚动承接；
  `window resize` 时重定位（分栏拖拽/窗口缩放后菜单不会错位）。
  菜单语义为 `role="dialog"` + 触发按钮 `aria-expanded` + 当前项 `aria-current="true"`，
  打开即聚焦当前语言，键盘可直接操作。
- **落盘是部分写**：只有用户改过的字段（`touched`）进入载荷，未触碰字段保持磁盘原值。
  整块覆盖会在「挂载读配置的 IPC 往返期间用户改了任一设置」或「读取失败走兜底值」时，
  把存量的 `fontSize: 20` / `editorTheme: vs-dark` 静默抹成默认值（配置可被设置页或
  手改文件维护，组件无权代其决定）。

## 测试

`src/components/editor/__tests__/CodeEditor.spec.ts`（Monaco/worker 轻量 mock）锁定：偏好读取在途时
卸载，配置随后返回不得创建 Monaco 或补挂 keydown listener；正常挂载仍使用有效容器创建实例并
注册内容/光标回调。
