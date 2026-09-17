# CodeEditor（Monaco 代码编辑器）

> 源文件：`src/components/editor/CodeEditor.vue`

## 职责

封装 Monaco Editor（浅色 `vs` 主题）：代码双向绑定、语言下拉切换、Ctrl+Enter 提交快捷键、清空（二次确认）/ 上传（原生 input）工具条、自动备份指示与光标位置上报，并向父级暴露 `focus()`。

## 核心类型/函数

**props**：`modelValue: string`（代码）、`language: string`（**HOJ 显示名**，Monaco id 经 `utils/language.monacoIdOf` 派生）、`languages?: string[]`（本题允许的提交语言列表，来自题目详情；空则回退内置默认）、`isDirty: boolean`（驱动备份指示）、`readonly?: boolean`（只读模式：隐藏工具条、禁用编辑与 Ctrl+Enter 提交快捷键，供提交详情页代码查看复用）。
**emits**：`update:modelValue`、`update:language`、`submit`、`cursor: [{ line, column }]`、`prefs-change: [EditorPrefs]`（挂载读配置后 + 每次弹层改动后上报，供父级同步状态行等派生展示）。
**expose**：`focus()` —— 供 `?focus=1` 快捷提交联动程序化聚焦；Monaco 未就绪时静默降级为 no-op，不抛错不阻塞。

| 名称 | 用途 |
|------|------|
| `MonacoEnvironment.getWorker` | 手动配置 5 个 worker（editor/ts/css/html/json，Vite `?worker` 导入），避免 worker 打包问题 |
| `editor: shallowRef<IStandaloneCodeEditor>` | Monaco 实例用 **shallowRef**：编辑器实例巨大且自带内部状态，深层响应式代理既昂贵又可能破坏其内部引用 |
| `availableLanguages` | computed | 语言下拉候选：**服务端列表存在时原样呈现、不补入当前语言**（列表外语言可见可选只会换来被拒的提交；归位由 ProblemSolveView 加载后统一做）；仅列表未提供（未加载/未返回）时回退 `DEFAULT_LANGUAGES` 并补入当前语言保持可见 |
| `suppressChangeEmit` | 外部改写代码（切题加载/清空/上传）时 `setValue` 会触发 change 事件，此标志抑制回流，避免把程序化写入误标为「用户编辑（dirty）」 |
| `handleKeydown` | window 级监听 Ctrl/Cmd+Enter → emit submit；onUnmounted 移除 |
| `handleFileChange` | 原生 `input[type=file]` + `file.text()` 读取上传代码（**不引入 Tauri dialog 插件**）；读后立即重置 `input.value` 允许连续选同一文件；**按扩展名自动识别语言**（`hojLanguageOfFileName` → `resolveAllowedLanguage` 按语言族解析为允许列表中的服务端原名，"Python3" 等变体也能命中），无命中不切换（切到不允许的语言只会换来一次提交失败） |
| `prefs` | `reactive<EditorPrefs>` | 当前偏好（字号 / Tab 宽度 / 主题）：挂载时 `Object.assign` 配置值，弹层改动就地更新（**不重读配置**，避免与在途写入打架） |
| `handlePrefsChange` | `(patch: Partial<EditorPrefs>) => void` | 弹层变更入口：更新 `prefs` → `applyPrefs()` 即时生效 → `emit('prefs-change')` → 400ms debounce 落盘 |
| `applyPrefs` | `fn` | `editor.updateOptions({ fontSize, tabSize })` + `monaco.editor.setTheme(editorTheme)`；**主题是全局选项**（Monaco 无按实例主题），同页其它编辑器一并跟随；实例未就绪时直接返回 |
| `resetPrefs` | `fn` | 恢复默认值（`utils/editor` 常量）：与手动改动同一条链路（即时生效 + 落盘），不是只改本地内存 |
| `persistPrefs` | `async fn` | `configService.updateEditorPrefs({...prefs})`；失败置 `prefsError`（「保存失败，设置仅本次会话生效」）但**不回滚已应用的值** |
| `prefsSaving` / `prefsError` | ref | 落盘状态，透传给 `EditorSettingsPopover` 的 `saving` / `error` |

编辑器配置：`theme` = 配置 `theme.editorTheme`（弹层可选 `vs` / `vs-dark`，**仅编辑器区域**；客户端界面仍只有浅色）、**fontSize / tabSize / editorTheme 挂载时经 `configService.getEditorPrefs()` 读取**（读取失败服务内部回退 14 / 4 / 'vs'）、JetBrains Mono 字体栈、minimap 关闭、wordWrap on、automaticLayout true（容器尺寸变化自适应，配合可拖拽分栏）、`readOnly` 跟随 readonly prop（只读时行高亮关闭）。

## 直接依赖

- `vue`
- `monaco-editor`（含 5 个 `?worker` 导入）
- `@/components/editor/EditorSettingsPopover.vue`（工具条设置弹层）
- `@/services/config.service`（编辑器偏好读写 + `EditorPrefs` 类型）
- `@/utils/language`（语言值域/文件名识别）
- `@/utils/editor`（偏好默认值）

## 被依赖

- `views/ProblemSolveView.vue` — 右栏编辑器（`v-model` 绑 `workspaceStore.code`，语言切换绑 `workspaceStore.changeLanguage`，cursor 转发给 EditorConsoleBar）
- `views/SubmissionDetailView.vue` — 只读代码查看（readonly 模式）
- `components/problem/QuickSubmitDialog.vue` — 快捷提交对话框内的代码输入

## 逻辑流程

```
onMounted → nextTick → Object.assign(prefs, await configService.getEditorPrefs())
                                  → emit prefs-change → monaco.editor.create(container, {...})
  onDidChangeModelContent → （非抑制时）emit update:modelValue → workspaceStore.updateCode
                             → 标脏 + 2s 防抖同步到 Rust 后端
  onDidChangeCursorPosition → emit cursor → 父级转发 EditorConsoleBar 状态行

watch props.modelValue → 与编辑器值不同才 setValue（抑制 change 回流）
watch props.language → monaco.editor.setModelLanguage
语言下拉选择 → emit update:language（父级走 workspaceStore.changeLanguage：
               本地乐观更新 + set_workspace_language 立即持久化）
EditorSettingsPopover change/reset → handlePrefsChange → applyPrefs（即时生效）
                → emit prefs-change → 400ms debounce → configService.updateEditorPrefs
onUnmounted → editor.dispose() + 移除 keydown 监听 + 在途偏好改动补一次落盘
```

设计要点：

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
