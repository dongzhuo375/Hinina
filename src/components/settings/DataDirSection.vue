<script setup lang="ts">
import SettingsSectionFrame from './SettingsSectionFrame.vue'
import { useDataDirSettings } from '@/composables/useDataDirSettings'

const props = defineProps<{
  model: ReturnType<typeof useDataDirSettings>
  /** 维护者层锁定（--settings 启动解锁）：整节操作禁用 + 提示 */
  locked?: boolean
}>()
const {
  dataDir, dataDirFailed, pendingDir, migrateData, changingDir, changeNotice, changeError,
  dataDirSourceMeta, loadDataDir, cancelDataDirChoice, chooseDataDir, confirmDataDir, restoreDefaultDataDir,
} = props.model
</script>

<template>
  <SettingsSectionFrame id="settings-5">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <path
                    d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2V7z"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">数据目录</h2>
                <span v-if="props.locked" class="ml-auto text-xs text-[var(--text-muted)]">
                  维护者选项：管理方以 --settings 启动后可修改
                </span>
              </div>

              <div class="flex flex-col gap-3 px-5 py-4">
                <p v-if="dataDirFailed" class="text-xs text-[var(--color-error)]">数据目录信息获取失败</p>
                <p v-else-if="!dataDir" class="text-xs text-[var(--text-muted)]">正在读取…</p>
                <template v-else>
                  <div class="text-xs leading-relaxed text-[var(--text-secondary)]">
                    <div class="flex items-center gap-2">
                      <span class="shrink-0 text-[var(--text-muted)]">当前</span>
                      <span
                        class="min-w-0 flex-1 truncate font-mono text-[var(--text-primary)]"
                        :title="dataDir.currentDir"
                      >
                        {{ dataDir.currentDir }}
                      </span>
                      <span class="shrink-0 font-medium" :class="dataDirSourceMeta.cls">
                        {{ dataDirSourceMeta.text }}
                      </span>
                    </div>
                    <div
                      v-if="dataDir.source === 'custom'"
                      class="mt-1 flex items-center gap-2 text-[var(--text-muted)]"
                    >
                      <span class="shrink-0">默认</span>
                      <span class="min-w-0 flex-1 truncate font-mono" :title="dataDir.defaultDir">
                        {{ dataDir.defaultDir }}
                      </span>
                    </div>
                  </div>

                  <!-- 回退临时目录必须显眼：这正是本次要消除的状态 -->
                  <p
                    v-if="dataDir.source === 'fallbackTemp'"
                    class="rounded-sm border border-[var(--color-wa-border)] bg-[var(--color-wa-bg)] px-3 py-2 text-xs leading-relaxed text-[var(--color-error)]"
                  >
                    默认数据目录不可用，当前数据存在<strong class="font-semibold">临时目录</strong>中
                    —— 系统清理临时文件时会连带删除你的代码与提交留档，请尽快更改数据目录。
                  </p>

                  <!-- 待重启提示：改动已记录但尚未生效，必须说清「现在仍在用旧目录」 -->
                  <p
                    v-if="dataDir.restartRequired"
                    class="rounded-sm border border-[var(--color-pending-border)] bg-[var(--color-pending-bg)] px-3 py-2 text-xs leading-relaxed text-[var(--color-warning)]"
                  >
                    已记录目录更改，<strong class="font-semibold">重启客户端后生效</strong>；
                    当前仍在使用上面的目录。
                  </p>

                  <!-- 选择器已返回候选路径：确认 + 是否迁移 -->
                  <div
                    v-if="pendingDir"
                    class="flex flex-col gap-2 rounded-sm border border-[var(--border-color)] bg-[var(--bg-sidebar)]/70 px-3.5 py-3"
                  >
                    <div class="flex items-center gap-2 text-xs">
                      <span class="shrink-0 text-[var(--text-muted)]">新目录</span>
                      <span
                        class="min-w-0 flex-1 truncate font-mono text-[var(--text-primary)]"
                        :title="pendingDir"
                      >
                        {{ pendingDir }}
                      </span>
                    </div>
                    <label class="flex items-start gap-2 text-xs text-[var(--text-secondary)]">
                      <input
                        v-model="migrateData"
                        type="checkbox"
                        class="mt-0.5 h-3.5 w-3.5 shrink-0 accent-[var(--color-primary)]"
                        :disabled="props.locked"
                      />
                      <span>
                        把现有数据迁移到新目录（工作区代码、提交留档、配置、会话）
                        <span class="text-[var(--text-muted)]">
                          —— 不勾选则新目录从空开始，旧数据原地保留
                        </span>
                      </span>
                    </label>
                  </div>

                  <div class="flex flex-wrap items-center justify-between gap-3">
                    <p class="min-w-0 flex-1 text-xs leading-relaxed text-[var(--text-muted)]">
                      数据目录决定工作区代码、提交留档、配置与日志的存放位置。
                      更改后需重启客户端生效。
                    </p>
                    <div class="flex shrink-0 items-center gap-2">
                      <span v-if="changeError" class="text-xs text-[var(--color-error)]">{{ changeError }}</span>
                      <span v-else-if="changeNotice" class="text-xs text-[var(--color-warning)]">
                        {{ changeNotice }}
                      </span>

                      <template v-if="pendingDir">
                        <button
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
                          :disabled="changingDir || props.locked"
                          @click="cancelDataDirChoice"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-sm bg-[var(--color-primary)] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-60"
                          :disabled="changingDir || props.locked"
                          @click="confirmDataDir"
                        >
                          {{ changingDir ? '记录中…' : '确认更改' }}
                        </button>
                      </template>
                      <template v-else>
                        <button
                          v-if="dataDir.source === 'custom'"
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)] disabled:cursor-not-allowed disabled:opacity-60"
                          :disabled="changingDir || props.locked"
                          @click="restoreDefaultDataDir"
                        >
                          恢复默认
                        </button>
                        <button
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-sidebar)] disabled:cursor-not-allowed disabled:opacity-60"
                          :disabled="changingDir || props.locked"
                          @click="chooseDataDir"
                        >
                          更改目录…
                        </button>
                      </template>
                    </div>
                  </div>
                </template>
              </div>
  </SettingsSectionFrame>
</template>
