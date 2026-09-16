# CodeEditor（Monaco 代码编辑器）

> 源文件：`src/components/editor/CodeEditor.vue`

## 职责

封装 Monaco Editor（浅色 `vs` 主题）：代码双向绑定、语言下拉切换、Ctrl+Enter 提交快捷键、清空（二次确认）/ 上传（原生 input）工具条、自动备份指示与光标位置上报，并向父级暴露 `focus()`。

## 核心类型/函数

**props**：`modelValue: string`（代码）、`language: string`（**HOJ 显示名**，Monaco id 经 `utils/language.monacoIdOf` 派生）、`languages?: string[]`（本题允许的提交语言列表，来自题目详情；空则回退内置默认）、`isDirty: boolean`（驱动备份指示）、`readonly?: boolean`（只读模式：隐藏工具条、禁用编辑与 Ctrl+Enter 提交快捷键，供提交详情页代码查看复用）。
**emits**：`update:modelValue`、`update:language`、`submit`、`cursor: [{ line, column }]`。
**expose**：`focus()` —— 供 `?focus=1` 快捷提交联动程序化聚焦；Monaco 未就绪时静默降级为 no-op，不抛错不阻塞。

| 名称 | 用途 |
|------|------|
| `MonacoEnvironment.getWorker` | 手动配置 5 个 worker（editor/ts/css/html/json，Vite `?worker` 导入），避免 worker 打包问题 |
| `editor: shallowRef<IStandaloneCodeEditor>` | Monaco 实例用 **shallowRef**：编辑器实例巨大且自带内部状态，深层响应式代理既昂贵又可能破坏其内部引用 |
| `availableLanguages` | computed | 语言下拉候选：以 props.languages（题目详情允许列表，HOJ 显示名）为准，空则回退 `DEFAULT_LANGUAGES`；当前语言不在列表时补入首位（工作区历史选择优先可见） |
| `suppressChangeEmit` | 外部改写代码（切题加载/清空/上传）时 `setValue` 会触发 change 事件，此标志抑制回流，避免把程序化写入误标为「用户编辑（dirty）」 |
| `handleKeydown` | window 级监听 Ctrl/Cmd+Enter → emit submit；onUnmounted 移除 |
| `handleFileChange` | 原生 `input[type=file]` + `file.text()` 读取上传代码（**不引入 Tauri dialog 插件**）；读后立即重置 `input.value` 允许连续选同一文件 |
| `showSettingsHint` | 设置按钮本轮未实现：点击显示「设置功能开发中」气泡 2s，而不是留一个死按钮 |

编辑器配置：`theme: 'vs'`（浅色）、**fontSize / tabSize 挂载时经 `configService.getEditorPrefs()` 读取**（设置页可调，对新打开的编辑器实例生效；读取失败服务内部回退 14 / 4）、JetBrains Mono 字体栈、minimap 关闭、wordWrap on、automaticLayout true（容器尺寸变化自适应，配合可拖拽分栏）、`readOnly` 跟随 readonly prop（只读时行高亮关闭）。

## 直接依赖

- `vue`
- `monaco-editor`（含 5 个 `?worker` 导入）
- `@/services/config.service`（编辑器偏好）

## 被依赖

- `views/ProblemSolveView.vue` — 右栏编辑器（`v-model` 绑 `workspaceStore.code`，语言切换绑 `workspaceStore.changeLanguage`，cursor 转发给 EditorConsoleBar）
- `views/SubmissionDetailView.vue` — 只读代码查看（readonly 模式）
- `components/problem/QuickSubmitDialog.vue` — 快捷提交对话框内的代码输入

## 逻辑流程

```
onMounted → nextTick → monaco.editor.create(container, {...})
  onDidChangeModelContent → （非抑制时）emit update:modelValue → workspaceStore.updateCode
                             → 标脏 + 2s 防抖同步到 Rust 后端
  onDidChangeCursorPosition → emit cursor → 父级转发 EditorConsoleBar 状态行

watch props.modelValue → 与编辑器值不同才 setValue（抑制 change 回流）
watch props.language → monaco.editor.setModelLanguage
语言下拉选择 → emit update:language（父级走 workspaceStore.changeLanguage：
               本地乐观更新 + set_workspace_language 立即持久化）
onUnmounted → editor.dispose() + 移除 keydown 监听 + 清理设置气泡定时器
```

设计要点：

- **备份指示的语义**：`isDirty=true` 显示灰色「编辑中…」，`false` 显示绿色「已自动备份」——
  状态来自 workspaceStore（防抖同步 + 后端 auto-save 落盘后清脏），组件只呈现。
- 清空代码必须二次确认（气泡内确认/取消）：赛场上误删代码是不可逆事故。
- Ctrl+Enter 挂 window 而非编辑器内 keybinding：焦点在题面/工具条时也能提交。
- 图标内联 SVG；不引入外部字体（字体栈回退到系统等宽字体）。
