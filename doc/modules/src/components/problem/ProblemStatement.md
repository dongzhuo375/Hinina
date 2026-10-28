# ProblemStatement（题面视图）

> 源文件：`src/components/problem/ProblemStatement.vue`

## 职责

解题页左栏题面：头部元信息（标题 + 时间/空间限制 + 通过率 + 语言倍率说明）与分节滚动正文（题目描述 / 输入格式 / 输出格式 / 样例数据，样例输入可一键复制）。正文排版按**容器宽度**自适应（容器查询 `cqi` + 视口高度约束），阅读行宽封顶 48em。

## 核心类型/函数

**props**：`problem: Problem`（详情实体）、`displayId: string`（比赛内展示题号，用于 limits 查询与标题前缀）。无 emits。

| 名称 | 签名 | 用途 |
|------|------|------|
| `baseUrl` | ref | OJ 基址（onMounted 经 `configService.getOjBaseUrl()` 异步读取，失败返回空串、相对图片按原样输出），用于题面相对图片 URL 改写 |
| `contestProblem` | computed | 从 `contestStore.problems` 按 displayId 找摘要（通过率 ac/total 来源） |
| `baseLimits` | computed | C/C++ 基准 limits：**优先 limits API 精确值**（`problemStore.limitsOf`），缺失回退题目详情自带值，两者皆无 → null（展示 `—`，不留假默认值） |
| `isDouble` / `shownLimits` | computed | 当前语言是否 ×2（`isDoubleLimitLanguage(workspaceStore.language)`）；头部展示的是**当前语言实际生效阈值**（`effectiveLimits`），与判题行为一致（HOJ-Problem-Limits-API.md §5） |
| `timeText` / `memoryText` / `baseTimeText` / `baseMemoryText` / `languageLabel` | computed | 生效值与基准值文案；×2 语言时头部下方标注「题面限制为 C/C++ 基准（x / y），当前语言 {lang} 判题时时间与内存 ×2」 |
| `renderedDescription/Input/Output` | computed | `renderMarkdown(文本, baseUrl)` → v-html |
| `samples` / `copySampleInput` / `copiedIndex` | — | 样例列表（`problem.samples ?? []`）；复制输入到剪贴板后调用共享 `useTransientValue(-1, 1500)` 显示反馈，scope dispose 清理 timer；剪贴板不可用只 `log.error` 记录，**不打断做题** |
| `acText` | computed | 通过率文案（`ac / total` + 百分比），无摘要数据时不编造 |

模板结构：根 `.problem-statement`（`container-type: inline-size`）→ `.statement-header`（`.statement-title` + 限制/通过率元信息）→ `.statement-content`（各 `section` 的 `h2.statement-section-title` + 正文 `.prose`）→ 样例区（`.sample` 输入/输出 + 复制按钮）。

scoped 样式要点：`--statement-font: clamp(14px, min(calc(7px + 1.1cqi), calc(14px + (100vh - 800px) / 70)), 20px)`、`--statement-gutter: clamp(28px, 4.5cqi, 56px)`、`.statement-content { max-width: 48em; margin-inline: auto }`、`.statement-section-title { font-size: 1.0714em }`；表格 `display: block; width: max-content; overflow-x: auto`（宽表格横向滚动而不是撑破分栏）。

## 直接依赖

- `vue`
- `@/types/problem` / `@/types/rank`（仅类型）
- `@/utils/markdown`（`renderMarkdown`）、`@/utils/limits`（`effectiveLimits` / `isDoubleLimitLanguage` / `formatTimeLimit` / `formatMemoryLimit`）
- `@/services/config.service`（OJ 基址；配置读取按约定一律经该服务，不直接调 config.bridge）
- `@/stores/contestStore`（通过率）/ `problemStore`（limits）/ `workspaceStore`（当前语言）
- `@/utils/logger`（`createLogger` —— 作用域日志）
- `@/composables/useTransientValue`（复制反馈临时值与 timer 生命周期）

## 被依赖

- `views/ProblemSolveView.vue` — 左栏题面（`problemStore.currentProblem` 就绪后渲染）

## 逻辑流程

```
props.problem 到达 → 分节正文 renderMarkdown（图片相对地址用 baseUrl 改写）
limits 双源合并：limitsOf(displayId) ?? problem.timeLimit/memoryLimit ?? null → `—`
workspaceStore.language 切换 → isDouble/shownLimits 重算 → 头部阈值实时跟随
                              （与 CodeEditor 语言下拉、后端 set_workspace_language 同一状态源）
样例复制 → navigator.clipboard.writeText → useTransientValue.show(index) → 1.5s 回弹（scope dispose 清理）
```

设计要点：

- **头部展示生效阈值而非基准值**：选手按显示值估算复杂度才不会误判 TLE；基准值降级为
  ×2 语言时的补充说明行（与 HOJ 网页端「C/C++ 1000MS，其它 2000MS」的口径一致）。
- limits 缺失显示 `—`：403 私有题等失败场景不回退假默认值（§9.5）。
- 提示节（hint）按约定跳过：`Problem` 类型无 hint 字段，服务端未提供数据，不画空节。
- 正文样式用 scoped `:deep()` 收敛在 `.prose` 内，v-html 内容不泄漏全局样式。
- **排版按容器宽度而非视口自适应**（PR #36）：题面在可拖拽分栏里，窗口最大化后
  左栏可能只有 40% 宽；用 `cqi`（容器查询单位）让字号跟随**左栏实际宽度**，
  同时用 `(100vh - 800px) / 70` 让大屏不把字撑得过大；`48em` 行宽上限保证长段落
  不会横跨整个宽屏而难以阅读。节标题去掉英文括号（「题目描述 (Problem Description)」
  → 「题目描述」）与装饰色条：中文界面里重复两遍标题只是噪声。
- 配色统一取赛场主题变量（`--text-primary` / `--text-secondary` / `--border-color` /
  `--bg-sidebar`），与工作台其余区域同源。
