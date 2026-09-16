# limits（题目限制格式化与语言倍率）

> 源文件：`src/utils/limits.ts`

## 职责

题目时限/内存的格式化与「按语言倍率换算实际生效 limits」的纯函数集合，保证客户端展示的阈值与 HOJ 判题端一致。

## 核心类型/函数

| 名称 | 签名 | 用途 |
|------|------|------|
| `formatTimeLimit` | `(ms: number) => string` | `1000` → `1.0s`、`500` → `500ms`（<1s 按毫秒整数，否则秒保留 1 位小数）；非法/≤0 → `—` |
| `formatMemoryLimit` | `(mb: number) => string` | `256` → `256 MB`、`1536` → `1.5 GB`（≥1024 MB 换算 GB，最多 2 位小数去尾零）；非法/≤0 → `—` |
| `isDoubleLimitLanguage` | `(language: string) => boolean` | 该语言是否适用 2 倍 limits（经 `utils/language.isCLikeLanguage` 严格判定：仅 C/C++ 族为 1 倍；空/未知语言保守放大为 true） |
| `effectiveLimits` | `(limits: ProblemLimits, language: string) => ProblemLimits` | 按倍率返回实际生效 limits；不修改入参，1 倍语言也返回新对象 |
| `formatLimitsSummary` | `(limits: ProblemLimits) => string` | 题目卡片紧凑文案，如 `1.0s / 256 MB`（C/C++ 基准值） |
| `INVALID_PLACEHOLDER` | `'—'`（模块内私有） | 非法/零值 limits 的占位符 |

## 直接依赖

- `@/types/rank`（仅 `ProblemLimits` 类型）

## 被依赖

- `components/problem/ProblemStatement.vue` — `effectiveLimits` / `isDoubleLimitLanguage` / `formatTimeLimit` / `formatMemoryLimit`（题面页按当前语言展示实际生效阈值）
- `components/problem/ProblemCard.vue` — `formatLimitsSummary`（卡片紧凑文案）
- `utils/__tests__/limits.spec.ts` — 单元测试

## 逻辑流程

```
单位约定（HOJ-Problem-Limits-API.md §4）：timeLimit 毫秒、memoryLimit MB
（sql 里 memory_limit 注释「单位kb」是过时注释，全链路实际按 MB）

倍率换算（同文档 §5）：
  判题端 JudgeContext 按源文件后缀判定 —— `.c`/`.cpp` 为 1 倍基准，
  其它语言时间与内存都 ×2（栈限制不放大，本模块也不建模栈）
  语言权威值是 HOJ 显示名（见 `utils/language`），经 `isCLikeLanguage` 严格判定 → 与判题端后缀行为一致
  未知/空 id → 按 2 倍处理（保守放大，宁可显示宽松阈值也不误导用户 TLE 判据）

effectiveLimits(limits, lang):
  isDoubleLimitLanguage(lang) = false → { ...limits }（拷贝，调用方可安全持有）
  否则 → { displayId, timeLimit×2, memoryLimit×2 }
```

设计要点：

- **非法值返回占位符 `—` 而不是假默认值**：limits 缺失通常意味着后端获取失败
  （如 403 私有题不可访问，HOJ-Problem-Limits-API.md §9.5），回退成「1s / 256MB」这类
  默认值会让用户按错误阈值估算复杂度；占位符明确表达「数据未取得」。
- 与后端约定一致：Rust `ProblemService::load_problem_limits` 对获取失败的题**不产出条目**
  （而不是填 0），前端 `problemStore.limitsOf` 返回 null，最终由本模块渲染为 `—`。

## 测试

`src/utils/__tests__/limits.spec.ts` 锁定：ms/s 与 MB/GB 格式化边界（999ms、1023MB、尾零去除）、非法/≤0 值返回 `—`、`c`/`cpp` 判定对大小写与首尾空白容错、未知/空语言按 2 倍保守处理、`effectiveLimits` 不修改入参且 1 倍语言也返回新对象、与 `formatLimitsSummary` 组合展示 2 倍阈值。
