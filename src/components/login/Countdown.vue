<script setup lang="ts">
import { computed } from 'vue'
import type { ContestPhase } from '@/utils/contest'
import { getClockSegments, getContestClock } from '@/utils/contest-clock'

const props = defineProps<{
  phase: ContestPhase
  startTime: number | null
  endTime: number | null
  now: number
  isLoggedIn: boolean
}>()

const clock = computed(() => getContestClock(props.phase, props.startTime, props.endTime, props.now))
const segments = computed(() => getClockSegments(clock.value.seconds))

const heading = computed(() => {
  if (clock.value.kind === 'elapsed') return '比赛已进行'
  if (clock.value.kind === 'ended') return '赛事状态'
  return '距离比赛开始'
})

const guidance = computed(() => {
  switch (props.phase) {
    case 'upcoming':
      return props.isLoggedIn
        ? '账号已就绪，开赛时将自动进入赛场'
        : '登录后留在此处等待，开赛时自动进入赛场'
    case 'running':
      return props.isLoggedIn ? '正在为你打开赛场' : '比赛已开始，登录后即可进入赛场'
    case 'ended':
      return '本场比赛已结束'
    default:
      return '比赛时间将在连接成功后显示'
  }
})
</script>

<template>
  <section aria-labelledby="countdown-heading" class="max-w-[680px]">
    <div class="mb-5 flex items-center justify-between border-b border-[#b9bfbc]/70 pb-3">
      <h2 id="countdown-heading" class="font-mono text-[10px] font-semibold uppercase tracking-[0.24em] text-[#7d8582]">
        {{ heading }} <span class="ml-2 text-[#a7aaa6]">/ CLOCK</span>
      </h2>
      <span class="flex items-center gap-2 font-mono text-[9px] uppercase tracking-[0.17em] text-[#7d8582]">
        <span class="h-1.5 w-1.5 rounded-full" :class="phase === 'running' ? 'bg-[#5f7a6a]' : phase === 'ended' ? 'bg-[#a33b2a]' : 'bg-[#8c6a43]'" aria-hidden="true"></span>
        {{ phase === 'running' ? 'LIVE' : phase === 'ended' ? 'CLOSED' : 'SCHEDULED' }}
      </span>
    </div>

    <p v-if="clock.kind === 'ended'" class="py-3 text-4xl font-semibold tracking-[0.04em] text-[#2e3033] sm:text-5xl">
      比赛已结束
    </p>
    <div
      v-else
      role="timer"
      aria-live="off"
      :aria-label="`${heading}：${segments.map((part) => `${part.value}${part.label}`).join(' ')}`"
      class="grid grid-cols-[1fr_16px_1fr_16px_1fr_16px_1fr] items-center gap-1 sm:gap-2"
    >
      <template v-for="(segment, index) in segments" :key="segment.key">
        <div class="min-w-0 text-center sm:text-left">
          <span
            class="block whitespace-nowrap font-mono text-[clamp(2.4rem,6.6vw,5.5rem)] font-medium leading-none tabular-nums tracking-[-0.075em] transition-colors duration-200"
            :class="clock.urgent ? 'text-[#a33b2a]' : 'text-[#2e3033]'"
            aria-hidden="true"
          >
            {{ segment.value }}
          </span>
          <span class="mt-5 block font-mono text-xs tracking-[0.22em] text-[#68706f]" aria-hidden="true">
            {{ segment.label }}
          </span>
          <span class="mt-3 inline-block h-px w-3 bg-[#7d8582]" aria-hidden="true"></span>
        </div>
        <span v-if="index < 3" class="mb-4 text-center text-2xl font-light text-[#b5bbb7]" aria-hidden="true">/</span>
      </template>
    </div>

    <div class="mt-7 flex items-start gap-3 border-l border-[#8c6a43]/55 pl-4">
      <span class="mt-[7px] h-1 w-1 shrink-0 bg-[#8c6a43]" aria-hidden="true"></span>
      <p class="text-xs leading-6 tracking-[0.025em] text-[#68706f]">{{ guidance }}</p>
    </div>
  </section>
</template>

<style scoped>
@media (prefers-reduced-motion: reduce) {
  span { transition: none; }
}
</style>
