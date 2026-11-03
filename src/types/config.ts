/// 应用配置，对应 Rust `core::entity::config::AppConfig`。

export interface AppConfig {
  oj: OjConfig
  editor: EditorConfig
  theme: ThemeConfig
  layout: LayoutConfig
  /**
   * Rust 端 `AppConfig.user`（`UserConfig`）不在本镜像中：该段现仅存旧版
   * 迁移字段 `lastOjType`（读入即迁往 `oj.active`，永不写回），前端从不
   * 读写；`updateConfig` 的 read-modify-write 原样保留运行时对象里的未知键。
   */
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
  /**
   * 编辑器主题（Monaco 主题名，vs / vs-dark），由解题页编辑器设置切换。
   * Rust 端另有旧版迁移字段 `themeName`（`legacy_theme_name`，读入即消费、
   * 永不写回），前端从不读写，不在本镜像中。
   */
  editorTheme: string
}

export interface LayoutConfig {
  sidebarWidth: number
  splitRatio: number
}
