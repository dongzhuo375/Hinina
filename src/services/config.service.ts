import type { AppConfig } from '@/types/config'
import * as configBridge from '@/bridge/config.bridge'
import { DEFAULT_LANGUAGE, normalizeHojLanguage } from '@/utils/language'

/// 轮询参数兜底值，与 Rust `core::entity::config` 的默认值保持一致（2 秒 / 300 秒）
const DEFAULT_POLL_INTERVAL_MS = 2_000
const DEFAULT_POLL_TIMEOUT_MS = 300_000

/** 评测轮询调度参数 */
export interface PollSchedule {
  /** 相邻两次查询的间隔 */
  intervalMs: number
  /** 单次提交的轮询总时长上限，超时后停止（避免评测丢失时无限轮询） */
  timeoutMs: number
}

/**
 * 配置服务 — 前端读取 Rust 端 `AppConfig` 的唯一入口，并提供派生参数。
 *
 * 分层约定：View / Store 一律不得直接调用 `config.bridge`，配置字段的读取与
 * 兜底逻辑集中在本服务，避免 IPC 细节与魔法数字散落各处。
 *
 * 缓存策略：配置属于应用级数据，由本服务在进程内缓存（缓存 Promise，使并发
 * 调用共享同一次 IPC）。设置页保存走 `updateConfig()`（写后端 + 失效缓存），
 * 其余读取方下次调用即拿到新值。
 */
export class ConfigService {
  private cache: Promise<AppConfig> | null = null

  /** 读取应用配置（进程内缓存；失败时清空缓存以便下次重试） */
  getConfig(): Promise<AppConfig> {
    if (!this.cache) {
      this.cache = configBridge.getConfig().catch((e) => {
        this.cache = null
        throw e
      })
    }
    return this.cache
  }

  /** 使配置缓存失效（配置热重载后调用） */
  invalidate(): void {
    this.cache = null
  }

  /**
   * OJ 基址：用于把题面/比赛简介中的相对图片 URL 改写为绝对地址。
   *
   * 读取失败时返回空串而非抛出 —— 缺基址只影响图片显示，不应阻断题面渲染。
   */
  async getOjBaseUrl(): Promise<string> {
    try {
      const config = await this.getConfig()
      return config.oj.hojUrl
    } catch (e) {
      console.error('[configService] 读取 OJ 基址失败，题面图片将保持相对路径:', e)
      return ''
    }
  }

  /**
   * 评测轮询调度参数（间隔 + 总超时）。
   *
   * 读取失败或配置非法时回退兜底值，保证提交后轮询一定能启动。
   */
  async getPollSchedule(): Promise<PollSchedule> {
    try {
      const config = await this.getConfig()
      return {
        intervalMs: secsToMs(config.oj.pollIntervalSecs, DEFAULT_POLL_INTERVAL_MS),
        timeoutMs: secsToMs(config.oj.pollTimeoutSecs, DEFAULT_POLL_TIMEOUT_MS),
      }
    } catch (e) {
      console.error('[configService] 读取轮询配置失败，使用兜底轮询参数:', e)
      return { intervalMs: DEFAULT_POLL_INTERVAL_MS, timeoutMs: DEFAULT_POLL_TIMEOUT_MS }
    }
  }

  /**
   * 编辑器偏好（字号 / Tab 宽度）。
   *
   * 读取失败或值非法时回退兜底值（14 / 4），编辑器渲染不因配置问题阻断。
   */
  async getEditorPrefs(): Promise<EditorPrefs> {
    try {
      const config = await this.getConfig()
      return {
        fontSize: clampInt(config.editor?.fontSize, 8, 32, DEFAULT_FONT_SIZE),
        tabSize: clampInt(config.editor?.tabSize, 1, 8, DEFAULT_TAB_SIZE),
      }
    } catch (e) {
      console.error('[configService] 读取编辑器配置失败，使用兜底值:', e)
      return { fontSize: DEFAULT_FONT_SIZE, tabSize: DEFAULT_TAB_SIZE }
    }
  }

  /**
   * 默认语言（HOJ 显示名，如 "C++"）。
   *
   * 值域归一：历史配置遗留的 Monaco id（'cpp' 等）经 `normalizeHojLanguage`
   * 映射回显示名，空值回退 "C++"；其它非空值（"Go"/"Rust"…）原样保留 ——
   * OJ 可能提供映射表之外的语言（值域约定见 `utils/language`）。
   */
  async getDefaultLanguage(): Promise<string> {
    try {
      const config = await this.getConfig()
      return normalizeHojLanguage(config.editor?.defaultLanguage)
    } catch (e) {
      console.error('[configService] 读取默认语言失败，回退 C++:', e)
      return DEFAULT_LANGUAGE
    }
  }

  /**
   * 解题页初始分栏比例（左栏占比）。
   *
   * 非法值回退设计稿默认 0.48；读取失败同样回退，不阻断页面渲染。
   */
  async getSplitRatio(): Promise<number> {
    try {
      const config = await this.getConfig()
      const ratio = config.layout?.splitRatio
      return typeof ratio === 'number' && ratio >= 0.2 && ratio <= 0.8 ? ratio : DEFAULT_SPLIT_RATIO
    } catch (e) {
      console.error('[configService] 读取分栏比例失败，使用默认值:', e)
      return DEFAULT_SPLIT_RATIO
    }
  }

  /**
   * 更新配置的唯一入口：读取当前配置 → 应用变更 → 整体写回后端 → 失效本地缓存。
   *
   * 后端 `update_config` 是整体替换语义，故必须先取当前值再改，
   * 避免局部字段把其余配置冲掉。返回写入后的配置供调用方直接使用。
   */
  async updateConfig(mutate: (draft: AppConfig) => void): Promise<AppConfig> {
    const current = await this.getConfig()
    const draft: AppConfig = structuredClone(current)
    mutate(draft)
    try {
      await configBridge.updateConfig(draft)
    } finally {
      // 无论成败都失效缓存：失败时下次读取重新拉后端真值，避免缓存与磁盘漂移
      this.invalidate()
    }
    return draft
  }
}

/// 编辑器偏好（字号 / Tab 宽度）
export interface EditorPrefs {
  fontSize: number
  tabSize: number
}

/// 编辑器兜底值，与 Rust `core::entity::config` 默认值一致
const DEFAULT_FONT_SIZE = 14
const DEFAULT_TAB_SIZE = 4
/// 分栏比例兜底值（设计稿 48% / 52%）
const DEFAULT_SPLIT_RATIO = 0.48

/// 整数钳位：非法值（非数字/越界）回退兜底
function clampInt(value: number | undefined, min: number, max: number, fallback: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return fallback
  const rounded = Math.round(value)
  return rounded >= min && rounded <= max ? rounded : fallback
}

/// 秒 → 毫秒；非正数或非法值回退兜底
function secsToMs(secs: number | undefined, fallbackMs: number): number {
  return typeof secs === 'number' && secs > 0 ? secs * 1000 : fallbackMs
}

export const configService = new ConfigService()
