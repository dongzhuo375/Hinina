/// 应用配置，对应 Rust `core::entity::config::AppConfig`。

export interface AppConfig {
  user: UserConfig
  oj: OjConfig
  editor: EditorConfig
  theme: ThemeConfig
  layout: LayoutConfig
}

export interface UserConfig {
  lastOjType: string
  lastUsername: string
}

export interface OjConfig {
  hojUrl: string
  timeoutSecs: number
  pollIntervalSecs: number
  pollTimeoutSecs: number
  cacheTtlSecs: number
  /// 题面缓存开关（内存 + 磁盘）；默认开启，关闭后每次打开题目都直连服务端
  cacheProblemStatement: boolean
  contestId: number
  contestPassword: string | null
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
