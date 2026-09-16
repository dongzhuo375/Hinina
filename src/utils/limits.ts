/**
 * 题目限制（时限/内存）的格式化与语言倍率换算（纯函数）。
 *
 * 单位约定（HOJ-Problem-Limits-API.md §4）：`timeLimit` 毫秒、`memoryLimit` MB。
 * 倍率约定（同文档 §5）：题面 limits 是 C/C++ 基准值，HOJ 服务端判题时按
 * **源文件后缀**判定 —— `.c`/`.cpp` 为 1 倍，其它语言时间与内存都 ×2
 * （栈限制不放大）。语言权威值是 HOJ 显示名（见 `utils/language`），
 * 经 `monacoIdOf` 归一后按 c/cpp 判定，与判题端后缀行为一致。
 */

import type { ProblemLimits } from '@/types/rank'
import { isCLikeLanguage } from '@/utils/language'

/// 非法/零值 limits 的占位符（服务端未返回或脏数据时不留空白）
const INVALID_PLACEHOLDER = '—'

/**
 * 时限格式化：`1000` → `1.0s`、`2500` → `2.5s`、`500` → `500ms`。
 * 不足 1 秒按毫秒整数显示，否则按秒保留 1 位小数；非法/≤0 返回 `—`。
 */
export function formatTimeLimit(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return INVALID_PLACEHOLDER
  if (ms < 1000) return `${Math.round(ms)}ms`
  return `${(ms / 1000).toFixed(1)}s`
}

/**
 * 内存格式化：`256` → `256 MB`、`2048` → `2 GB`、`1536` → `1.5 GB`。
 * 满 1024 MB 换算为 GB（最多 2 位小数、去掉尾零）；非法/≤0 返回 `—`。
 */
export function formatMemoryLimit(mb: number): string {
  if (!Number.isFinite(mb) || mb <= 0) return INVALID_PLACEHOLDER
  if (mb < 1024) return `${Math.round(mb)} MB`
  const gb = Number.parseFloat((mb / 1024).toFixed(2))
  return `${gb} GB`
}

/**
 * 该语言是否适用 2 倍 limits（时间 ×2、内存 ×2）。
 *
 * 倍率来源：HOJ-Problem-Limits-API.md §5 —— 判题端 `JudgeContext` 以源文件后缀
 * `.c`/`.cpp` 为 1 倍基准，其它语言一律 ×2。入参是 HOJ 显示名（"C"/"C++"/"Java"…，
 * 兼容历史 Monaco id），经 `isCLikeLanguage` 严格判定；空/未知语言按保守放大
 * 处理（×2，宁可显示宽松阈值）。
 */
export function isDoubleLimitLanguage(language: string): boolean {
  return !isCLikeLanguage(language ?? '')
}

/**
 * 按语言倍率返回实际生效的 limits（与判题端阈值一致，供提交页/详情展示）。
 * 不修改入参，1 倍语言也返回新对象，调用方可安全持有。
 */
export function effectiveLimits(limits: ProblemLimits, language: string): ProblemLimits {
  if (!isDoubleLimitLanguage(language)) return { ...limits }
  return {
    displayId: limits.displayId,
    timeLimit: limits.timeLimit * 2,
    memoryLimit: limits.memoryLimit * 2,
  }
}

/** 题目卡片用紧凑文案，如 `1.0s / 256 MB`（C/C++ 基准值） */
export function formatLimitsSummary(limits: ProblemLimits): string {
  return `${formatTimeLimit(limits.timeLimit)} / ${formatMemoryLimit(limits.memoryLimit)}`
}
