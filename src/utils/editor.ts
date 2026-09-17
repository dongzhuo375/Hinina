/// 编辑器偏好值域的唯一权威模块（Monaco 主题 / 字号 / Tab 宽度）。
///
/// 与 Rust `core::entity::config` 的默认值、取值域保持一致：配置可被手改，
/// 前后端各自做一次归一，保证喂给 Monaco 的恒为合法值。
/// 值域变更需同步两处：本文件、Rust `EditorConfig::sanitize`（存储侧钳制）；
/// 前端消费方（`config.service` / 设置页 / 解题页弹层）一律取本文件常量，不再复制字面量。

/** 编辑器主题选项（`id` 即 `monaco.editor.setTheme` 的取值） */
export interface EditorThemeOption {
  id: string
  label: string
  hint: string
}

/// 可选编辑器主题：**仅影响编辑器区域**，客户端界面仍只有浅色（dark UI 未实现）
export const EDITOR_THEMES: readonly EditorThemeOption[] = [
  { id: 'vs', label: '浅色', hint: '与客户端界面一致' },
  { id: 'vs-dark', label: '深色', hint: '长时间编码更护眼' },
]

export const DEFAULT_EDITOR_THEME = 'vs'
export const DEFAULT_EDITOR_FONT_SIZE = 14
export const DEFAULT_EDITOR_TAB_SIZE = 4
export const EDITOR_FONT_SIZE_MIN = 8
export const EDITOR_FONT_SIZE_MAX = 32

/// Tab 宽度**存储域**（与 Rust `EditorConfig::sanitize` 的 1..=8 一致）。
/// 刻意宽于下方候选集：候选是 UI 快捷档位，手改 `config.json` 写入的 6 仍是合法宽度，
/// 弹层会把它一并呈现为选中态而不是「无选中」。
export const EDITOR_TAB_SIZE_MIN = 1
export const EDITOR_TAB_SIZE_MAX = 8

/// Tab 宽度候选档位（ICPC 惯例 4；与设置页候选一致）
export const EDITOR_TAB_SIZES: readonly number[] = [2, 4, 8]

/// 主题归一：未知/非法值（手改配置、未来自定义主题名）回退默认 `'vs'`。
///
/// 不归一的风险：`monaco.editor.setTheme` 收到未知主题名会静默保留上一个主题，
/// 界面与配置就此不一致，且用户无从察觉。
export function normalizeEditorTheme(raw: unknown): string {
  return EDITOR_THEMES.some((t) => t.id === raw) ? (raw as string) : DEFAULT_EDITOR_THEME
}
