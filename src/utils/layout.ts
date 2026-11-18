/// 布局偏好值域的唯一权威模块（题面/编辑器分栏比例）。
///
/// 与 Rust `core::entity::config` 的 `LayoutConfig::sanitize` 保持一致：配置可被手改，
/// 前后端各自做一次归一，保证喂给 UI 的恒为合法值（保存路径 Rust 已钳制；
/// 读取路径不经过 sanitize，由本模块在前端兜底，否则手改 config.json 写入的
/// 0.25 会原样进解题页、首次拖拽即跳变）。
/// 值域变更需同步两处：本文件、Rust `LayoutConfig::sanitize`（存储侧钳制）；
/// 前端消费方（`config.service` / 设置页滑杆 / 解题页拖拽）一律取本文件常量，不再复制字面量。

/// 分栏比例兜底值（设计稿 48% / 52%）；与 Rust `default_split_ratio` 一致
export const DEFAULT_SPLIT_RATIO = 0.48
export const SPLIT_RATIO_MIN = 0.3
export const SPLIT_RATIO_MAX = 0.7

/// 分栏比例归一：镜像 Rust sanitize 语义 —— 非数字/非有限值回退默认 0.48，
/// 有限越界值钳到 [0.30, 0.70] 边界（保留手改者意图的方向，而非整值回退）。
/// 刻意不做两位小数舍入：解题页拖拽的单次增量可能小于 0.005，舍入会卡死增量；
/// 与滑杆 step 对齐的舍入由设置页的 `clampRatio` 包装完成。
export function clampSplitRatio(raw: unknown): number {
  if (typeof raw !== 'number' || !Number.isFinite(raw)) return DEFAULT_SPLIT_RATIO
  return Math.min(SPLIT_RATIO_MAX, Math.max(SPLIT_RATIO_MIN, raw))
}
