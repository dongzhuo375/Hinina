# ProblemStatement（题面视图）

> 源文件：`src/components/problem/ProblemStatement.vue`

## 职责

解题页左栏题面：头部元信息（标题 + 时间/空间限制 + 通过率 + 语言倍率说明）与分节滚动正文（题目描述 / 输入格式 / 输出格式 / 样例数据，样例输入可一键复制）。

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
| `samples` / `copySampleInput` / `copiedIndex` | — | 样例列表；复制输入到剪贴板并显示「已复制」1.5s 反馈；剪贴板不可用只 console.error **不打断做题** |

## 直接依赖

- `vue`
- `@/types/problem` / `@/types/rank`（仅类型）
- `@/utils/markdown`（`renderMarkdown`）、`@/utils/limits`（`effectiveLimits` / `isDoubleLimitLanguage` / `formatTimeLimit` / `formatMemoryLimit`）
- `@/services/config.service`（OJ 基址；配置读取按约定一律经该服务，不直接调 config.bridge）
- `@/stores/contestStore`（通过率）/ `problemStore`（limits）/ `workspaceStore`（当前语言）

## 被依赖

- `views/ProblemSolveView.vue` — 左栏题面（`problemStore.currentProblem` 就绪后渲染）

## 逻辑流程

```
props.problem 到达 → 分节正文 renderMarkdown（图片相对地址用 baseUrl 改写）
limits 双源合并：limitsOf(displayId) ?? problem.timeLimit/memoryLimit ?? null → `—`
workspaceStore.language 切换 → isDouble/shownLimits 重算 → 头部阈值实时跟随
                              （与 CodeEditor 语言下拉、后端 set_workspace_language 同一状态源）
样例复制 → navigator.clipboard.writeText → 「已复制」反馈 1.5s（定时器 onUnmounted 清理）
```

设计要点：

- **头部展示生效阈值而非基准值**：选手按显示值估算复杂度才不会误判 TLE；基准值降级为
  ×2 语言时的补充说明行（与 HOJ 网页端「C/C++ 1000MS，其它 2000MS」的口径一致）。
- limits 缺失显示 `—`：403 私有题等失败场景不回退假默认值（§9.5）。
- 提示节（hint）按约定跳过：`Problem` 类型无 hint 字段，服务端未提供数据，不画空节。
- 正文样式用 scoped `:deep()` 收敛在 `.prose` 内，v-html 内容不泄漏全局样式。
