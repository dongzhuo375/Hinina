<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { NCard, NForm, NFormItem, NInput, NButton, NSpace } from 'naive-ui'
import { useAuthStore } from '@/stores/authStore'

const router = useRouter()
const auth = useAuthStore()

const username = ref('')
const password = ref('')
const isSubmitting = ref(false)

/// 登录表单提交
async function handleLogin() {
  if (!username.value || !password.value) return
  isSubmitting.value = true
  try {
    await auth.login(username.value, password.value)
    router.replace('/contest')
  } catch {
    // 错误由 store 处理并设置 error
  } finally {
    isSubmitting.value = false
  }
}

/// 启动时检查已有 session
auth.checkSession().then((hasSession) => {
  if (hasSession) router.replace('/contest')
})
</script>

<template>
  <div class="flex h-screen items-center justify-center bg-[var(--bg-body)]">
    <n-card class="w-[380px]" :bordered="false" content-class="!p-8">
      <div class="mb-8 text-center">
        <h1 class="text-2xl font-bold tracking-tight text-[var(--color-primary)]">Hinina</h1>
        <p class="mt-1 text-sm text-[var(--text-secondary)]">XCPC 竞赛客户端</p>
      </div>

      <n-form size="large" @submit.prevent="handleLogin">
        <n-form-item>
          <n-input
            v-model:value="username"
            placeholder="用户名"
            :disabled="isSubmitting"
            clearable
          />
        </n-form-item>

        <n-form-item>
          <n-input
            v-model:value="password"
            type="password"
            placeholder="密码"
            :disabled="isSubmitting"
            show-password-on="click"
            @keyup.enter="handleLogin"
          />
        </n-form-item>

        <n-form-item v-if="auth.error" class="!mb-2">
          <p class="text-sm text-[var(--color-error)]">{{ auth.error }}</p>
        </n-form-item>

        <n-form-item>
          <n-button
            type="primary"
            block
            :loading="isSubmitting"
            :disabled="!username || !password"
            @click="handleLogin"
          >
            登录
          </n-button>
        </n-form-item>
      </n-form>
    </n-card>
  </div>
</template>
