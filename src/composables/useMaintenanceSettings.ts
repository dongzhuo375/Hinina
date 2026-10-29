import { ref } from 'vue'
import { useTransientValue } from '@/composables/useTransientValue'
import { systemService } from '@/services/system.service'
import { useAnnouncementStore } from '@/stores/announcementStore'
import { useContestStore } from '@/stores/contestStore'
import { useProblemStore } from '@/stores/problemStore'
import { errorMessage } from '@/utils/error'
import type { LocalDataUsage } from '@/types/system'

/// 文件体积展示（自适应单位；与代码长度的固定 KB 口径刻意不同）
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '-'
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`
}

/// 重置与清理两个维护动作的状态和副作用；不参与配置表单的 dirty/save
export function useMaintenanceSettings() {
  const resetting = ref(false)
  const resetConfirmOpen = ref(false)
  const {
    value: resetNotice,
    show: showResetNotice,
    clear: clearResetNotice,
  } = useTransientValue<{ text: string; warn: boolean } | null>(null, 3_000)
  const resetError = ref<string | null>(null)

  const usage = ref<LocalDataUsage | null>(null)
  const usageFailed = ref(false)
  const purgeLogs = ref(true)
  const purgeSnapshots = ref(true)
  const purging = ref(false)
  const purgeConfirmOpen = ref(false)
  const {
    value: purgeNotice,
    show: showPurgeNotice,
    clear: clearPurgeNotice,
  } = useTransientValue<{ text: string; warn: boolean } | null>(null, 4_000)
  const purgeError = ref<string | null>(null)

  async function resetClient(): Promise<void> {
    resetConfirmOpen.value = false
    resetting.value = true
    clearResetNotice()
    resetError.value = null
    try {
      await systemService.resetClient()

      // 只等待比赛重拉；公告 refresh 仍是 void，不改变现有提示判定
      let refreshed = true
      await useContestStore()
        .loadContest()
        .catch(() => {
          refreshed = false
        })
      useProblemStore().invalidateMyStatus()
      void useAnnouncementStore().refresh()

      showResetNotice(refreshed
        ? { text: '已重置，数据已重新拉取', warn: false }
        : { text: '已重置，但数据重新拉取失败，请手动刷新', warn: true })
    } catch (e) {
      resetError.value = errorMessage(e, '重置失败')
    } finally {
      resetting.value = false
    }
  }

  async function loadUsage(): Promise<void> {
    try {
      usage.value = await systemService.localDataUsage()
      usageFailed.value = false
    } catch {
      usageFailed.value = true
    }
  }

  async function purgeLocalData(): Promise<void> {
    if (!purgeLogs.value && !purgeSnapshots.value) {
      purgeError.value = '请先选择要清理的内容'
      purgeConfirmOpen.value = false
      return
    }
    purgeConfirmOpen.value = false
    purging.value = true
    clearPurgeNotice()
    purgeError.value = null
    try {
      const report = await systemService.purgeLocalData(purgeLogs.value, purgeSnapshots.value)
      const parts = [`释放 ${formatBytes(report.freedBytes)}`]
      if (report.removedSnapshots > 0) parts.push(`删除留档 ${report.removedSnapshots} 个`)
      const logMissed = purgeLogs.value && !report.logCleared
      if (logMissed) parts.push('日志未清理（文件层不可用或写入失败）')

      showPurgeNotice({ text: `已清理：${parts.join('，')}`, warn: logMissed })
      await loadUsage()
    } catch (e) {
      purgeError.value = errorMessage(e, '清理本地数据失败')
    } finally {
      purging.value = false
    }
  }

  return {
    resetting,
    resetConfirmOpen,
    resetNotice,
    resetError,
    usage,
    usageFailed,
    purgeLogs,
    purgeSnapshots,
    purging,
    purgeConfirmOpen,
    purgeNotice,
    purgeError,
    resetClient,
    loadUsage,
    purgeLocalData,
    formatBytes,
  }
}
