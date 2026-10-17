<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ContestPhase } from '@/utils/contest'

const username = defineModel<string>('username', { required: true })
const password = defineModel<string>('password', { required: true })

const props = defineProps<{
  isSubmitting: boolean
  isSigningOut: boolean
  isLoggedIn: boolean
  accountName: string | null
  authError: string | null
  phase: ContestPhase
  briefHtml: string
  briefVisible: boolean
}>()

const emit = defineEmits<{
  submit: []
  switchAccount: []
  toggleBrief: []
}>()

const showPassword = ref(false)
const loginDisabled = computed(() => props.isSubmitting || !username.value.trim() || !password.value)

const accountStatus = computed(() => {
  if (props.phase === 'upcoming') return '账号已就绪，开赛后将自动进入赛场'
  if (props.phase === 'running') return '比赛正在进行，正在打开赛场…'
  if (props.phase === 'ended') return '比赛已结束，你已返回入口页面'
  return '账号已登录，正在确认比赛信息'
})
</script>

<template>
  <section class="login-card-wrap w-full max-w-[470px]" aria-label="参赛账号验证">
    <div class="login-card relative px-7 py-8 sm:px-10 sm:py-10">
      <header class="mb-8">
        <p class="font-sans text-[10px] font-semibold uppercase tracking-[1px] text-[#8c6a43]">
          Hinina <span class="mx-1 text-[#b5bbb7]">/</span> Secure entry
        </p>
        <h2 class="mt-3 text-2xl font-semibold tracking-[0.025em] text-[#2e3033]">
          {{ isLoggedIn ? '账号已验证' : '探索竞赛之旅' }}
        </h2>
        <span class="mt-4 block h-[2px] w-12 bg-[#2e3033]"></span>
      </header>

      <div v-if="isLoggedIn" class="min-h-[250px]">
        <div class="flex items-start gap-4 border-b border-[#b9bfbc]/70 py-5">
          <span class="flex h-10 w-10 shrink-0 items-center justify-center border border-[#c8ceca] text-[#5f7a6a]" aria-hidden="true">
            <svg class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.6">
              <path stroke-linecap="square" stroke-linejoin="miter" d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
              <circle cx="12" cy="7" r="4" />
            </svg>
          </span>
          <div class="min-w-0 flex-1">
            <p class="font-sans text-[10px] uppercase tracking-[1px] text-[#6b7280]">Current account</p>
            <p class="mt-1 truncate text-sm font-semibold text-[#2e3033]" :title="accountName ?? undefined">
              {{ accountName }}
            </p>
          </div>
          <span class="mt-1 flex items-center gap-1.5 font-sans text-[10px] uppercase tracking-[1px] text-[#5f7a6a]">
            <span class="h-1.5 w-1.5 bg-[#5f7a6a]"></span> Ready
          </span>
        </div>

        <p role="status" class="min-h-[52px] py-5 text-sm leading-6 text-[#6b7280]">
          {{ accountStatus }}
        </p>

        <button
          type="button"
          class="w-full border border-[#aeb6b2] px-4 py-3 text-[13px] font-semibold tracking-[0.08em] text-[#49504d] transition-colors hover:border-[#8c6a43] hover:text-[#8c6a43] focus-visible:outline-2 focus-visible:outline-offset-3 focus-visible:outline-[#8c6a43] disabled:cursor-wait disabled:opacity-60"
          :disabled="isSigningOut"
          @click="emit('switchAccount')"
        >
          {{ isSigningOut ? '正在安全退出…' : '切换参赛账号' }}
        </button>
        <p v-if="authError" role="alert" class="mt-4 text-[13px] leading-5 text-[#a33b2a]">
          {{ authError }}
        </p>
      </div>

      <form v-else class="space-y-6" :aria-busy="isSubmitting" @submit.prevent="emit('submit')">
        <div>
          <label for="contest-username" class="mb-2 block text-[13px] font-semibold tracking-[0.08em] text-[#6b7280]">
            参赛账号 <span class="ml-1 font-sans text-[10px] font-normal tracking-[1px] text-[#6b7280]">/ TEAM ID</span>
          </label>
          <div class="login-field flex items-center gap-3">
            <svg class="h-[18px] w-[18px] shrink-0 text-[#55615d]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
              <path stroke-linecap="square" stroke-linejoin="miter" d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
              <circle cx="12" cy="7" r="4" />
            </svg>
            <input
              id="contest-username"
              v-model="username"
              type="text"
              autocomplete="username"
              placeholder="请输入参赛账号"
              class="min-w-0 flex-1 bg-transparent py-3 text-sm text-[#2e3033] placeholder:text-[#6b7280] focus:outline-none"
              :disabled="isSubmitting"
              :aria-invalid="authError ? true : undefined"
              :aria-describedby="authError ? 'login-error' : undefined"
              required
            />
          </div>
        </div>

        <div>
          <label for="contest-password" class="mb-2 block text-[13px] font-semibold tracking-[0.08em] text-[#6b7280]">
            参赛密码 <span class="ml-1 font-sans text-[10px] font-normal tracking-[1px] text-[#6b7280]">/ CONTEST PIN</span>
          </label>
          <div class="login-field flex items-center gap-3">
            <svg class="h-[18px] w-[18px] shrink-0 text-[#55615d]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
              <rect height="11" rx="1" width="18" x="3" y="11" />
              <path stroke-linecap="square" d="M7 11V7a5 5 0 0 1 10 0v4" />
            </svg>
            <input
              id="contest-password"
              v-model="password"
              :type="showPassword ? 'text' : 'password'"
              autocomplete="current-password"
              placeholder="请输入比赛认证密码"
              class="min-w-0 flex-1 bg-transparent py-3 font-mono text-sm tracking-[0.12em] text-[#2e3033] placeholder:font-sans placeholder:tracking-normal placeholder:text-[#6b7280] focus:outline-none"
              :disabled="isSubmitting"
              :aria-invalid="authError ? true : undefined"
              :aria-describedby="authError ? 'login-error' : undefined"
              required
            />
            <button
              type="button"
              class="flex h-9 w-9 shrink-0 items-center justify-center text-[#68706f] transition-colors hover:text-[#8c6a43] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-[#8c6a43]"
              :aria-label="showPassword ? '隐藏密码' : '显示密码'"
              :aria-pressed="showPassword"
              :disabled="isSubmitting"
              @click="showPassword = !showPassword"
            >
              <svg v-if="!showPassword" class="h-[18px] w-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
                <path stroke-linecap="square" d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z" />
                <circle cx="12" cy="12" r="3" />
              </svg>
              <svg v-else class="h-[18px] w-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
                <path stroke-linecap="square" d="M3 3l18 18M10.6 5.1A10.8 10.8 0 0 1 12 5c6.5 0 10 7 10 7a17.4 17.4 0 0 1-4.1 4.8M6.2 6.4C3.5 8.2 2 12 2 12s3.5 7 10 7c1.2 0 2.3-.2 3.3-.6" />
                <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
              </svg>
            </button>
          </div>
        </div>

        <p v-if="authError" id="login-error" role="alert" class="-mt-2 text-[13px] leading-5 text-[#a33b2a]">
          {{ authError }}
        </p>

        <button
          type="submit"
          class="login-submit group relative flex min-h-[54px] w-full items-center justify-center gap-3 overflow-hidden bg-[#2e3033] px-5 text-sm font-semibold tracking-[0.1em] text-white transition-colors hover:bg-[#3a3d3f] focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-[#8c6a43] disabled:cursor-not-allowed"
          :disabled="loginDisabled"
          :aria-busy="isSubmitting"
        >
          <span>{{ isSubmitting ? '正在验证参赛资格' : '登录进入赛场' }}</span>
          <svg v-if="!isSubmitting" class="h-4 w-4 transition-transform duration-200 group-hover:translate-x-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
            <path stroke-linecap="square" d="M4 12h15m-6-6 6 6-6 6" />
          </svg>
          <span v-if="isSubmitting" class="login-loading-line" aria-hidden="true"></span>
        </button>
      </form>

      <div class="mt-7">
        <button
          type="button"
          class="group flex w-full items-center justify-between py-1 text-[13px] text-[#6b7280] transition-colors hover:text-[#8c6a43] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#8c6a43]"
          :aria-expanded="briefVisible"
          aria-controls="contest-brief"
          @click="emit('toggleBrief')"
        >
          <span class="mx-auto">{{ briefVisible ? '收起比赛简介' : '查看比赛简介' }}</span>
          <svg class="h-3.5 w-3.5 transition-transform duration-200" :class="briefVisible ? 'rotate-180' : ''" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.6" aria-hidden="true">
            <path stroke-linecap="square" d="m6 9 6 6 6-6" />
          </svg>
        </button>
        <div v-if="briefVisible" id="contest-brief" class="mt-3 max-h-48 overflow-y-auto border-l border-[#8c6a43]/50 pl-4 text-[13px] leading-6 text-[#6b7280]">
          <div v-if="briefHtml" class="prose prose-sm max-w-none" v-html="briefHtml"></div>
          <p v-else>暂无比赛简介</p>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.login-card {
  clip-path: polygon(18px 0, calc(100% - 18px) 0, 100% 18px, 100% calc(100% - 18px), calc(100% - 18px) 100%, 18px 100%, 0 calc(100% - 18px), 0 18px);
  background: #e5e7eb;
  backdrop-filter: blur(14px);
}

.login-card::before {
  position: absolute;
  inset: 1px;
  clip-path: polygon(17.5px 0, calc(100% - 17.5px) 0, 100% 17.5px, 100% calc(100% - 17.5px), calc(100% - 17.5px) 100%, 17.5px 100%, 0 calc(100% - 17.5px), 0 17.5px);
  background: rgba(247, 246, 241, 0.96);
  content: '';
  z-index: -1;
}

.login-submit {
  clip-path: polygon(12px 0, 100% 0, 100% calc(100% - 12px), calc(100% - 12px) 100%, 0 100%, 0 12px);
  color: #fff;
}

.login-submit:hover { box-shadow: inset 0 0 20px rgba(255, 255, 255, 0.07); }
.login-submit:focus-visible { clip-path: none; }

.login-field {
  position: relative;
  border-bottom: 1px solid #b9bfbc;
}

.login-field:has([aria-invalid='true']) { border-color: #a33b2a; }
.login-field:focus-within { outline: 1px solid rgba(140, 106, 67, 0.35); outline-offset: 4px; }

.login-field::after {
  position: absolute;
  right: 50%;
  bottom: -1px;
  left: 50%;
  height: 1px;
  background: #8c6a43;
  content: '';
  transition: left 200ms ease-out, right 200ms ease-out;
}

.login-field:focus-within::after {
  right: 0;
  left: 0;
}

.login-loading-line {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: 2px;
  width: 35%;
  background: rgba(255, 255, 255, 0.82);
  animation: login-loading 1.6s ease-in-out infinite;
}

@keyframes login-loading {
  0% { transform: translateX(-110%); }
  100% { transform: translateX(310%); }
}

@media (prefers-reduced-motion: reduce) {
  .login-field::after,
  .login-loading-line,
  button,
  svg {
    animation: none !important;
    transition-duration: 0.01ms !important;
  }

  .login-loading-line {
    width: 100%;
    transform: none;
  }
}
</style>
