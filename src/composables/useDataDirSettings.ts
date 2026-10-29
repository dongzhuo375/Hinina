import { computed, ref } from 'vue'
import { useTransientValue } from '@/composables/useTransientValue'
import { systemService } from '@/services/system.service'
import { errorMessage } from '@/utils/error'
import type { DataDirInfo } from '@/types/system'

/// 数据目录：只校验并记录位置指针，真正迁移在下次启动完成
export function useDataDirSettings() {
  const dataDir = ref<DataDirInfo | null>(null)
  const dataDirFailed = ref(false)
  const pendingDir = ref<string | null>(null)
  const migrateData = ref(true)
  const changingDir = ref(false)
  const {
    value: changeNotice,
    show: showChangeNotice,
    clear: clearChangeNotice,
  } = useTransientValue<string | null>(null, 5_000)
  const changeError = ref<string | null>(null)

  const dataDirSourceMeta = computed(() => {
    switch (dataDir.value?.source) {
      case 'custom':
        return { text: '用户指定', cls: 'text-[var(--color-primary)]' }
      case 'fallbackTemp':
        return { text: '临时目录（数据可能被系统清理）', cls: 'text-[var(--color-error)]' }
      default:
        return { text: '默认位置', cls: 'text-[var(--text-muted)]' }
    }
  })

  async function loadDataDir(): Promise<void> {
    try {
      dataDir.value = await systemService.getDataDir()
      dataDirFailed.value = false
    } catch {
      dataDirFailed.value = true
    }
  }

  function cancelDataDirChoice(): void {
    pendingDir.value = null
    migrateData.value = true
  }

  async function chooseDataDir(): Promise<void> {
    changeError.value = null
    clearChangeNotice()
    try {
      const picked = await systemService.pickDataDir()
      if (!picked) return
      pendingDir.value = picked
      migrateData.value = true
    } catch (e) {
      changeError.value = errorMessage(e, '打开目录选择器失败')
    }
  }

  async function confirmDataDir(): Promise<void> {
    const target = pendingDir.value
    if (!target) return
    // 先取快照：复位后通知仍须如实反映用户本次选择
    const migrate = migrateData.value
    changingDir.value = true
    changeError.value = null
    try {
      await systemService.setDataDir(target, migrate)
      pendingDir.value = null
      migrateData.value = true
      await loadDataDir()
      showChangeNotice(migrate
        ? '已记录，重启后生效并自动迁移现有数据'
        : '已记录，重启后生效（未迁移现有数据）')
    } catch (e) {
      changeError.value = errorMessage(e, '更改数据目录失败')
    } finally {
      changingDir.value = false
    }
  }

  async function restoreDefaultDataDir(): Promise<void> {
    const migrate = migrateData.value
    changingDir.value = true
    changeError.value = null
    try {
      await systemService.resetDataDir(migrate)
      pendingDir.value = null
      migrateData.value = true
      await loadDataDir()
      showChangeNotice(migrate
        ? '已记录，重启后回到默认目录并迁移现有数据'
        : '已记录，重启后回到默认目录（未迁移现有数据）')
    } catch (e) {
      changeError.value = errorMessage(e, '恢复默认目录失败')
    } finally {
      changingDir.value = false
    }
  }

  return {
    dataDir,
    dataDirFailed,
    pendingDir,
    migrateData,
    changingDir,
    changeNotice,
    changeError,
    dataDirSourceMeta,
    loadDataDir,
    cancelDataDirChoice,
    chooseDataDir,
    confirmDataDir,
    restoreDefaultDataDir,
  }
}
