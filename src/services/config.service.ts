import type { AppConfig } from '@/types/config'
import * as configBridge from '@/bridge/config.bridge'

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
 * 缓存策略：配置属于应用级只读数据、无跨视图同步需求，故不引入额外 store，
 * 由本服务在进程内缓存（缓存 Promise，使并发调用共享同一次 IPC）。
 * v0.x 尚无设置界面，配置在运行期稳定；后续接入热重载时调用 `invalidate()` 即可。
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
}

/// 秒 → 毫秒；非正数或非法值回退兜底
function secsToMs(secs: number | undefined, fallbackMs: number): number {
  return typeof secs === 'number' && secs > 0 ? secs * 1000 : fallbackMs
}

export const configService = new ConfigService()
