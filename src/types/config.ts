/// 应用配置，对应 Rust `core::entity::config::AppConfig`。

export interface AppConfig {
  user: UserConfig
  oj: OjConfig
  editor: EditorConfig
  theme: ThemeConfig
  layout: LayoutConfig
}

export interface UserConfig {
  lastUsername: string
}

/** 单个 OJ 实例的连接配置（对应 Rust `OjInstance`） */
export interface OjInstance {
  /// OJ 身份（与适配器工厂 id 一致，如 "HOJ"）
  id: string
  /// 服务端地址（站点根，不带 /api 等前缀）
  baseUrl: string
  /// 是否启用（false = 后端不注册该实例的 Provider）
  enabled: boolean
  /// OJ 私有旋钮（弱类型：值域由各 OJ 自行约定）
  options: Record<string, unknown>
}

export interface OjConfig {
  /// 当前 OJ 实例 id（OJ 选择是应用级状态）
  active: string
  instances: OjInstance[]
  /// 当前比赛引用 —— 不透明字符串（HOJ 数字串 / Hydro ObjectId）；空串 = 未配置
  contestRef: string
  contestPassword: string | null
  timeoutSecs: number
  pollIntervalSecs: number
  pollTimeoutSecs: number
  cacheTtlSecs: number
  /// 题面缓存开关（内存 + 磁盘）；默认开启，关闭后每次打开题目都直连服务端
  cacheProblemStatement: boolean
}

export interface EditorConfig {
  fontSize: number
  tabSize: number
  autoSave: boolean
  autoSaveIntervalSecs: number
  defaultLanguage: string
}

export interface ThemeConfig {
  themeName: string
  editorTheme: string
}

export interface LayoutConfig {
  sidebarWidth: number
  splitRatio: number
}
