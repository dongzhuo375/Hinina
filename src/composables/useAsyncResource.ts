import { ref } from 'vue'
import type { Ref } from 'vue'

/**
 * 「一次性加载 + failed 标志」的资源壳：成功写值并复位 failed，失败置 failed
 * 且不动已有数据。用于设置页各区块的辅助信息（存储信息/数据目录/本地占用），
 * 失败只影响本区块、不参与配置表单的 dirty/save。
 *
 * 加载失败的具体原因不落 UI：调用方在失败态只展示「获取失败」回退行，
 * 排障走前端日志转发。
 */
export function useAsyncResource<T>(loader: () => Promise<T>) {
  const data = ref<T | null>(null) as Ref<T | null>
  const failed = ref(false)

  async function load(): Promise<void> {
    try {
      data.value = await loader()
      failed.value = false
    } catch {
      failed.value = true
    }
  }

  return { data, failed, load }
}
