<script setup lang="ts">
import SettingsSectionFrame from './SettingsSectionFrame.vue'
import SettingsField from './SettingsField.vue'
import SettingsSwitch from './SettingsSwitch.vue'
import { DEFAULT_LANGUAGES } from '@/utils/language'
import { EDITOR_TAB_SIZES } from '@/utils/editor'
import { SETTINGS_INPUT as INPUT } from '@/composables/useSettingsForm'
import type { SettingsForm, SettingsFormPatch } from '@/composables/useSettingsForm'

const props = defineProps<{
  form: SettingsForm
  errors: Record<string, string | null>
  inputClass: (field: string) => string
}>()
const emit = defineEmits<{ (event: 'update:form', patch: SettingsFormPatch): void }>()
const LANGUAGE_OPTIONS = DEFAULT_LANGUAGES
const TAB_SIZES = EDITOR_TAB_SIZES

</script>

<template>
  <SettingsSectionFrame
    id="settings-1"
    title="编辑器"
    hint="解题页编辑器设置可即时调整，此处改动对新打开的解题页生效"
  >
    <template #icon>
      <path
        d="m8 9-3 3 3 3m8-6 3 3-3 3M13.5 6l-3 12"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <div class="grid grid-cols-1 gap-x-4 gap-y-4 px-5 py-4 sm:grid-cols-2">
      <SettingsField label="字号" :error="props.errors.fontSize" hint="8–32">
        <input
          v-model="props.form.fontSize"
          type="text"
          inputmode="numeric"
          spellcheck="false"
          :class="props.inputClass('fontSize')"
        />
      </SettingsField>

      <SettingsField label="Tab 宽度" hint="缩进宽度（ICPC 惯例 4）">
        <select v-model.number="props.form.tabSize" :class="INPUT">
          <option v-for="size in TAB_SIZES" :key="size" :value="size">
            {{ size }} 空格
          </option>
        </select>
      </SettingsField>

      <SettingsField label="默认语言" hint="新建代码文件时的初始语言">
        <select v-model="props.form.defaultLanguage" :class="INPUT">
          <option v-for="lang in LANGUAGE_OPTIONS" :key="lang" :value="lang">
            {{ lang }}
          </option>
        </select>
      </SettingsField>

      <div class="block">
        <span class="mb-1 block text-xs font-medium text-[var(--text-secondary)]">
          自动保存
        </span>
        <SettingsSwitch
          :checked="props.form.autoSave"
          @update="(checked) => emit('update:form', { autoSave: checked })"
        />
        <span class="mt-1 block text-xs text-[var(--text-muted)]">
          定时保存解题页代码，崩溃后可恢复；关闭后仅在切题、失焦、离开解题页或关窗时落盘
        </span>
      </div>

      <SettingsField
        label="自动保存间隔（秒）"
        :locked="!props.form.autoSave"
        :error="props.errors.autoSaveIntervalSecs"
        hint="5–300"
      >
        <input
          v-model="props.form.autoSaveIntervalSecs"
          type="text"
          inputmode="numeric"
          spellcheck="false"
          :disabled="!props.form.autoSave"
          :class="props.inputClass('autoSaveIntervalSecs')"
        />
      </SettingsField>
    </div>
  </SettingsSectionFrame>
</template>
