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
  if (clock.value.kind === 'elapsed') return '比赛进行中'
  if (clock.value.kind === 'ended') return '比赛已结束'
  return '距离比赛开始'
})

const guidance = computed(() => {
  switch (props.phase) {
    case 'upcoming':
      return props.isLoggedIn
        ? '已登录 · 开赛后自动进入'
        : '距离比赛开始还有'
    case 'running':
      return props.isLoggedIn ? '正在进入赛场' : '比赛进行中 · 登录后即可进入'
    case 'ended':
      return '本场比赛已结束'
    default:
      return '赛程信息连接后显示'
  }
})

const timerLabel = computed(() => {
  if (props.phase === 'none') return guidance.value
  return `${heading.value}：${segments.value.map((part) => `${part.value}${part.label}`).join(' ')}`
})
</script>

<template>
  <section aria-labelledby="countdown-heading" class="max-w-[680px]">
    <h2 id="countdown-heading" class="sr-only">{{ heading }}</h2>

    <p v-if="clock.kind === 'ended'" class="py-3 text-4xl font-semibold tracking-[0.04em] text-[#2e3033] sm:text-5xl">
      比赛已结束
    </p>
    <div
      v-else
      role="timer"
      aria-live="off"
      :aria-label="timerLabel"
      class="grid grid-cols-[minmax(0,1fr)_12px_minmax(0,1fr)_12px_minmax(0,1fr)_12px_minmax(0,1fr)] items-start gap-x-1 sm:gap-x-2"
    >
      <template v-for="(segment, index) in segments" :key="segment.key">
        <div class="min-w-0 text-center">
          <span
            class="countdown-value block whitespace-nowrap font-mono text-[clamp(2.4rem,6.6vw,5.5rem)] font-medium leading-none tabular-nums tracking-[-0.075em] transition-colors duration-200"
            :class="clock.urgent ? 'text-[#a33b2a]' : 'text-[#2e3033]'"
            aria-hidden="true"
          >
            {{ segment.value }}
          </span>
          <span class="mt-5 block font-sans text-[13px] leading-4 tracking-[1px] text-[#6b7280]" aria-hidden="true">
            {{ segment.label }}
          </span>
          <span class="mt-[14px] inline-block h-px w-5 bg-[#7d8582]" aria-hidden="true"></span>
        </div>
        <span v-if="index < 3" class="mt-9 text-center font-mono text-sm font-normal leading-none text-[#b5bbb7]" aria-hidden="true">/</span>
      </template>
    </div>

    <p
      :class="[
        'text-[13px] tracking-[0.025em] text-[#6b7280]',
        props.phase === 'upcoming' && !props.isLoggedIn ? 'mt-12' : 'mt-7',
      ]"
    >
      {{ guidance }}
    </p>
  </section>
</template>

<style scoped>
.countdown-value {
  font-family: var(--login-font-mono);
  font-variant-numeric: tabular-nums slashed-zero;
  font-feature-settings: "zero" 1;
}

@media (prefers-reduced-motion: reduce) {
  span { transition: none; }
}
</style>
