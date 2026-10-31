<script setup lang="ts">
import SettingsSectionFrame from './SettingsSectionFrame.vue'
import { ojBaseUrlHint } from '@/utils/oj'
import { ojSelectOptions } from '@/utils/oj'
import { SETTINGS_INPUT as INPUT } from '@/composables/useSettingsForm'
import type { SettingsForm, SettingsFormPatch } from '@/composables/useSettingsForm'

const props = defineProps<{
  form: SettingsForm
  errors: Record<string, string | null>
  options: ReturnType<typeof ojSelectOptions>
  switchError: string | null
  switchNotice: string | null
  inputClass: (field: string) => string
  /** 维护者层锁定（--settings 启动解锁）：连接与缓存调优字段禁用 + 提示；比赛 ID/密码不受影响 */
  locked?: boolean
}>()
const emit = defineEmits<{
  (event: 'update:form', patch: SettingsFormPatch): void
  (event: 'switch-oj'): void
}>()

function onSwitchOj(): void {
  emit('switch-oj')
}
</script>

<template>
  <SettingsSectionFrame id="settings-0">
              <div class="flex items-center gap-2 border-b border-[var(--border-color)] px-5 py-3.5">
                <svg
                  class="h-4 w-4 text-[var(--color-primary)]"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <rect x="3" y="4" width="18" height="7" rx="1.5" />
                  <rect x="3" y="13" width="18" height="7" rx="1.5" />
                  <path d="M7 7.5h.01M7 16.5h.01" stroke-linecap="round" />
                </svg>
                <h2 class="text-sm font-semibold text-[var(--text-primary)]">OJ 服务器</h2>
                <span v-if="props.locked" class="ml-auto text-xs text-[var(--text-muted)]">
                  连接与缓存字段已锁定：管理方以 --settings 启动后可修改
                </span>
              </div>
              <div class="grid grid-cols-1 gap-x-4 gap-y-4 px-5 py-4 sm:grid-cols-2">
                <label class="block">
                  <span
                    class="mb-1 block text-xs font-medium"
                    :class="props.locked ? 'text-[var(--text-muted)]' : 'text-[var(--text-secondary)]'"
                  >
                    当前 OJ
                  </span>
                  <select v-model="props.form.activeOj" :class="INPUT" :disabled="props.locked" @change="onSwitchOj">
                    <option
                      v-for="o in props.options"
                      :key="o.id"
                      :value="o.id"
                      :disabled="o.disabled"
                    >
                      {{ o.id }}{{ o.disabled ? '（已禁用）' : o.configured ? '' : '（未配置）' }}
                    </option>
                  </select>
                  <span v-if="props.switchError" class="mt-1 block text-xs text-[var(--color-error)]">
                    {{ props.switchError }}
                  </span>
                  <!-- 引导/安抚提示与错误**并存**：两者语义独立（如「实例已保存但切换未成功」
                       既是错误也需要安抚），用 v-else-if 会让提示永不可见 -->
                  <span v-if="props.switchNotice" class="mt-1 block text-xs text-[var(--color-warning)]">
                    {{ props.switchNotice }}
                  </span>
                  <span
                    v-if="!props.switchError && !props.switchNotice"
                    class="mt-1 block text-xs text-[var(--text-muted)]"
                  >
                    切换即时生效并持久化，将离开本页并放弃所有未保存的修改；候选 = 已知 OJ 类型 + 配置里的其它实例
                  </span>
                </label>

                <label class="block">
                  <span
                    class="mb-1 block text-xs font-medium"
                    :class="props.locked ? 'text-[var(--text-muted)]' : 'text-[var(--text-secondary)]'"
                  >
                    服务器地址
                  </span>
                  <input
                    v-model="props.form.ojUrl"
                    type="text"
                    spellcheck="false"
                    :placeholder="ojBaseUrlHint(props.form.activeOj)"
                    :class="props.inputClass('ojUrl')"
                    :disabled="props.locked"
                  />
                  <span v-if="props.errors.ojUrl" class="mt-1 block text-xs text-[var(--color-error)]">
                    {{ props.errors.ojUrl }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    属于当前 OJ（{{ props.form.activeOj }}）；改动当前 OJ 的地址后需重启生效，新建实例保存后会自动切换
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    比赛 ID / 引用
                  </span>
                  <input
                    v-model="props.form.contestRef"
                    type="text"
                    spellcheck="false"
                    :class="props.inputClass('contestRef')"
                  />
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    HOJ 为数字 ID，其它 OJ 为资源引用；留空 = 不自动加载
                  </span>
                </label>

                <label class="block">
                  <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
                    比赛密码
                  </span>
                  <input
                    v-model="props.form.contestPassword"
                    type="password"
                    autocomplete="off"
                    placeholder="无私有赛密码可留空"
                    :class="INPUT"
                  />
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    仅用于进入私有赛，保存在本地
                  </span>
                </label>

                <label class="block">
                  <span
                    class="mb-1 block text-xs font-medium"
                    :class="props.locked ? 'text-[var(--text-muted)]' : 'text-[var(--text-secondary)]'"
                  >
                    请求超时（秒）
                  </span>
                  <input
                    v-model="props.form.timeoutSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="props.inputClass('timeoutSecs')"
                    :disabled="props.locked"
                  />
                  <span v-if="props.errors.timeoutSecs" class="mt-1 block text-xs text-[var(--color-error)]">
                    {{ props.errors.timeoutSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    单次 HTTP 请求的超时，1–120
                  </span>
                </label>

                <label class="block">
                  <span
                    class="mb-1 block text-xs font-medium"
                    :class="props.locked ? 'text-[var(--text-muted)]' : 'text-[var(--text-secondary)]'"
                  >
                    轮询间隔（秒）
                  </span>
                  <input
                    v-model="props.form.pollIntervalSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="props.inputClass('pollIntervalSecs')"
                    :disabled="props.locked"
                  />
                  <span v-if="props.errors.pollIntervalSecs" class="mt-1 block text-xs text-[var(--color-error)]">
                    {{ props.errors.pollIntervalSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    评测结果查询间隔，1–30
                  </span>
                </label>

                <label class="block">
                  <span
                    class="mb-1 block text-xs font-medium"
                    :class="props.locked ? 'text-[var(--text-muted)]' : 'text-[var(--text-secondary)]'"
                  >
                    轮询总超时（秒）
                  </span>
                  <input
                    v-model="props.form.pollTimeoutSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="props.inputClass('pollTimeoutSecs')"
                    :disabled="props.locked"
                  />
                  <span v-if="props.errors.pollTimeoutSecs" class="mt-1 block text-xs text-[var(--color-error)]">
                    {{ props.errors.pollTimeoutSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    单次提交轮询的总时长上限，30–3600
                  </span>
                </label>

                <label class="block">
                  <span
                    class="mb-1 block text-xs font-medium"
                    :class="props.locked ? 'text-[var(--text-muted)]' : 'text-[var(--text-secondary)]'"
                  >
                    列表缓存 TTL（秒）
                  </span>
                  <input
                    v-model="props.form.cacheTtlSecs"
                    type="text"
                    inputmode="numeric"
                    spellcheck="false"
                    :class="props.inputClass('cacheTtlSecs')"
                    :disabled="props.locked"
                  />
                  <span v-if="props.errors.cacheTtlSecs" class="mt-1 block text-xs text-[var(--color-error)]">
                    {{ props.errors.cacheTtlSecs }}
                  </span>
                  <span v-else class="mt-1 block text-xs text-[var(--text-muted)]">
                    0 表示禁用缓存，0–600
                  </span>
                </label>

                <div class="block">
                  <span
                    class="mb-1 block text-xs font-medium"
                    :class="props.locked ? 'text-[var(--text-muted)]' : 'text-[var(--text-secondary)]'"
                  >
                    题面缓存
                  </span>
                  <div class="flex items-center gap-3 py-1">
                    <button
                      type="button"
                      role="switch"
                      :aria-checked="props.form.cacheProblemStatement"
                      :disabled="props.locked"
                      class="settings-switch relative h-6 w-11 shrink-0 transition-colors disabled:cursor-not-allowed disabled:opacity-60"
                      :class="
                        props.form.cacheProblemStatement ? 'bg-[var(--color-primary)]' : 'bg-[var(--bg-sidebar)]'
                      "
                      @click="emit('update:form', { cacheProblemStatement: !props.form.cacheProblemStatement })"
                    >
                      <span
                        class="settings-switch-knob absolute top-0.5 left-0.5 h-5 w-5 rounded-sm bg-[var(--bg-card)] transition-transform"
                        :class="props.form.cacheProblemStatement ? 'translate-x-5' : ''"
                      ></span>
                    </button>
                    <span class="text-xs text-[var(--text-secondary)]">
                      {{ props.form.cacheProblemStatement ? '已开启' : '已关闭' }}
                    </span>
                  </div>
                  <span class="mt-1 block text-xs text-[var(--text-muted)]">
                    缓存题面（内存 + 磁盘），切题来回与断网时秒开；关闭后每次打开都请求服务端
                  </span>
                </div>
              </div>
  </SettingsSectionFrame>
</template>
