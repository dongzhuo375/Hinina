<script setup lang="ts">
import SettingsSectionFrame from './SettingsSectionFrame.vue'
import { useMaintenanceSettings } from '@/composables/useMaintenanceSettings'

const props = defineProps<{ model: ReturnType<typeof useMaintenanceSettings> }>()
const {
  resetting, resetConfirmOpen, resetNotice, resetError,
  usage, usageFailed, purgeLogs, purgeSnapshots, purging, purgeConfirmOpen, purgeNotice, purgeError,
  resetClient, loadUsage, purgeLocalData, formatBytes,
} = props.model
</script>

<template>
  <SettingsSectionFrame id="settings-4">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <ellipse cx="12" cy="5" rx="8" ry="3" />
                  <path
                    d="M4 5v6c0 1.66 3.58 3 8 3s8-1.34 8-3V5M4 11v6c0 1.66 3.58 3 8 3s8-1.34 8-3v-6"
                    stroke-linecap="round"
                  />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">重置与清理</h2>
              </div>

              <div class="divide-y divide-[var(--border-color)]">
                <!-- 重置客户端（安全，可反复点） -->
                <div class="px-5 py-4">
                  <div class="flex flex-wrap items-start justify-between gap-3">
                    <p class="min-w-0 flex-1 text-xs leading-relaxed text-[var(--text-secondary)]">
                      <span class="font-medium text-[var(--text-primary)]">重置客户端</span>
                      —— 清空比赛元信息、题面、题目限制、终态提交详情等本地缓存，并清空公告已读标记，清完立即重新拉取。
                      <span class="text-[var(--text-muted)]">
                        不会删除工作区代码、提交源码留档、登录会话与配置。
                      </span>
                    </p>

                    <div class="flex shrink-0 items-center gap-2">
                      <span v-if="resetError" class="text-xs text-[var(--color-error)]">{{ resetError }}</span>
                      <span
                        v-else-if="resetNotice"
                        class="text-xs"
                        :class="resetNotice.warn ? 'text-[var(--color-warning)]' : 'text-[var(--color-success)]'"
                      >
                        {{ resetNotice.text }}
                      </span>

                      <template v-if="resetConfirmOpen || resetting">
                        <button
                          v-if="!resetting"
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
                          @click="resetConfirmOpen = false"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-sm bg-[var(--color-primary)] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:brightness-110 disabled:opacity-60"
                          :disabled="resetting"
                          @click="resetClient"
                        >
                          {{ resetting ? '重置中…' : '确认重置' }}
                        </button>
                      </template>
                      <button
                        v-else
                        type="button"
                        class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-sidebar)]"
                        @click="resetConfirmOpen = true"
                      >
                        重置客户端
                      </button>
                    </div>
                  </div>
                </div>

                <!-- 清理本地数据（不可逆：先看范围与体积，再勾选确认） -->
                <div class="px-5 py-4">
                  <p class="text-xs leading-relaxed text-[var(--text-secondary)]">
                    <span class="font-medium text-[var(--text-primary)]">清理本地数据</span>
                    —— 删除<strong class="font-semibold text-[var(--color-error)]">不可重建</strong>的本地内容。
                    日志是排障线索，提交源码留档是 OJ 不回吐代码时的唯一来源，删掉都无法恢复。
                  </p>

                  <!-- 体积预览：不可逆动作必须让用户看到确切范围 -->
                  <div
                    class="mt-3 flex flex-col gap-2 rounded-sm border border-[var(--border-color)] bg-[var(--bg-sidebar)]/70 px-3.5 py-3"
                  >
                    <p v-if="usageFailed" class="text-xs text-[var(--color-error)]">占用信息获取失败</p>
                    <p v-else-if="!usage" class="text-xs text-[var(--text-muted)]">正在统计占用…</p>
                    <template v-else>
                      <label class="flex items-start gap-2 text-xs text-[var(--text-secondary)]">
                        <input
                          v-model="purgeLogs"
                          type="checkbox"
                          class="mt-0.5 h-3.5 w-3.5 shrink-0 accent-[var(--color-primary)]"
                        />
                        <span class="min-w-0">
                          日志与崩溃记录
                          <span class="font-mono text-[var(--text-primary)]">
                            {{ formatBytes(usage.logBytes + usage.crashBytes) }}
                          </span>
                          <span class="text-[var(--text-muted)]">
                            （日志只清空内容、保留文件；崩溃记录文件在临时目录，会被删除）
                          </span>
                        </span>
                      </label>
                      <label class="flex items-start gap-2 text-xs text-[var(--text-secondary)]">
                        <input
                          v-model="purgeSnapshots"
                          type="checkbox"
                          class="mt-0.5 h-3.5 w-3.5 shrink-0 accent-[var(--color-primary)]"
                        />
                        <span class="min-w-0">
                          {{ usage.keepDays }} 天前的提交源码留档
                          <span class="font-mono text-[var(--text-primary)]">
                            {{ usage.snapshotStaleCount }} 个 · {{ formatBytes(usage.snapshotStaleBytes) }}
                          </span>
                          <span class="text-[var(--text-muted)]">
                            （留档共 {{ usage.snapshotTotalCount }} 个 ·
                            {{ formatBytes(usage.snapshotTotalBytes) }}，近 {{ usage.keepDays }} 天的不受影响）
                          </span>
                        </span>
                      </label>
                    </template>
                  </div>

                  <div class="mt-3 flex flex-wrap items-center justify-between gap-3">
                    <p class="min-w-0 flex-1 text-xs leading-relaxed text-[var(--text-muted)]">
                      此操作<strong class="font-semibold text-[var(--color-error)]">不可撤销</strong>；
                      不会删除工作区代码、配置、登录会话与公告已读状态。
                    </p>
                    <div class="flex shrink-0 items-center gap-2">
                      <span v-if="purgeError" class="text-xs text-[var(--color-error)]">{{ purgeError }}</span>
                      <span
                        v-else-if="purgeNotice"
                        class="text-xs"
                        :class="purgeNotice.warn ? 'text-[var(--color-warning)]' : 'text-[var(--color-success)]'"
                      >
                        {{ purgeNotice.text }}
                      </span>

                      <template v-if="purgeConfirmOpen || purging">
                        <button
                          v-if="!purging"
                          type="button"
                          class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-secondary)] transition-colors hover:bg-[var(--bg-sidebar)]"
                          @click="purgeConfirmOpen = false"
                        >
                          取消
                        </button>
                        <button
                          type="button"
                          class="rounded-sm bg-[var(--color-error)] px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-[var(--color-error)] disabled:opacity-60"
                          :disabled="purging"
                          @click="purgeLocalData"
                        >
                          {{ purging ? '清理中…' : '确认清理（不可恢复）' }}
                        </button>
                      </template>
                      <button
                        v-else
                        type="button"
                        class="rounded-sm border border-[var(--border-color)] px-3 py-1.5 text-xs font-medium text-[var(--text-primary)] transition-colors hover:bg-[var(--bg-sidebar)]"
                        @click="purgeConfirmOpen = true"
                      >
                        清理本地数据
                      </button>
                    </div>
                  </div>
                </div>
              </div>
  </SettingsSectionFrame>
</template>
