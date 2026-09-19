/**
 * OJ 类型域唯一权威模块。
 *
 * 值域约定（跨端契约）：OJ 身份是**字符串 id**，与 Rust 侧三处同源 ——
 * `core::provider::oj_id::OjId`、适配器工厂的 `id()`、配置里的 `oj.instances[].id`
 * （`OjId` 同时决定会话文件名 `sessions/{id}.json`）。
 *
 * 前端维护「已知 OJ 类型」这份**枚举清单**，用于设置页的下拉候选：用户可以从清单里
 * 挑一个尚未配置的 OJ（如 Hydro），填地址保存后即创建对应实例 —— 无需手改
 * config.json。刻意不做「实例增删管理」：本项目面向校内赛的少数几个已知 OJ，
 * 固定枚举比通用实例管理更简单，也避免用户在 UI 里拼错 id（id 会拼进会话文件名）。
 *
 * 清单外的 id（手改配置的私有部署）不在枚举里，但设置页仍会显示（见
 * `ojSelectOptions`），不丢信息、不悄悄改写用户配置。
 */

/// 已知 OJ 类型（顺序即下拉展示顺序；新增 OJ 时与后端 `factories()` 同步加一项）
export const OJ_TYPES: readonly string[] = ['HOJ', 'Hydro']

/// 各 OJ 的地址占位提示（仅用于输入框 placeholder，不写进配置）
const OJ_BASE_URL_HINT: Readonly<Record<string, string>> = {
  HOJ: 'https://hoj.example.com',
  Hydro: 'https://hydro.ac',
}

/// 地址输入框的占位提示：未知类型回退通用示例
export function ojBaseUrlHint(ojId: string): string {
  return OJ_BASE_URL_HINT[ojId] ?? 'https://example-oj.com'
}

/// 设置页 OJ 下拉的候选项
export interface OjOption {
  /// OJ 身份 id
  id: string
  /// 配置里是否已有该实例（false = 尚未配置：需先填地址保存，之后才能切换）
  configured: boolean
  /// 已有实例但被禁用（禁用实例不会被注册，切过去只会得到 ProviderNotFound）
  disabled: boolean
}

interface InstanceLike {
  id: string
  enabled: boolean
}

/**
 * 下拉候选 = 已知 OJ 枚举（含尚未配置者）+ 配置里出现的其它 id。
 *
 * 尚未配置的类型**也要列出来**（`configured: false`）：这正是「从枚举里启用一个新
 * OJ」的入口；禁用者标注为 disabled 但仍显示，避免用户的配置在 UI 里凭空消失。
 */
export function ojSelectOptions(instances: readonly InstanceLike[]): OjOption[] {
  const byId = new Map(instances.map((instance) => [instance.id, instance]))
  const known: OjOption[] = OJ_TYPES.map((id) => {
    const instance = byId.get(id)
    return {
      id,
      configured: instance !== undefined,
      disabled: instance !== undefined && !instance.enabled,
    }
  })
  // 枚举外的 id 追加在后（手改配置的私有部署），保持「已知在前」的稳定顺序
  const extra: OjOption[] = instances
    .filter((instance) => !OJ_TYPES.includes(instance.id))
    .map((instance) => ({ id: instance.id, configured: true, disabled: !instance.enabled }))
  return [...known, ...extra]
}
