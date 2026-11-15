import { useSystemStore } from '@/stores/systemStore'
import { useAsyncResource } from '@/composables/useAsyncResource'
import { useTransientValue } from '@/composables/useTransientValue'
import type { StorageInfo } from '@/types/system'

/// About 区块：存储信息读取与逐项复制反馈；失败只影响本区块
export function useAboutSettings() {
  const systemStore = useSystemStore()
  const { data: storage, failed: storageFailed, load: loadStorage } =
    useAsyncResource<StorageInfo>(() => systemStore.getStorageInfo())

  const { value: copiedKey, show: showCopied } = useTransientValue<string | null>(null, 2_000)

  async function copyText(key: string, text: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(text)
      showCopied(key)
    } catch {
      // 剪贴板不可用（权限/环境）时静默忽略，复制不是关键路径
    }
  }

  return { storage, storageFailed, loadStorage, copiedKey, copyText }
}
