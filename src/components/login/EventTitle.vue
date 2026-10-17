<script setup lang="ts">
defineProps<{
  title: string
  phaseLabel: string
  description: string
}>()
</script>

<template>
  <section aria-labelledby="event-title" class="max-w-2xl">
    <p class="mb-6 flex items-center gap-3 font-mono text-[10px] font-semibold uppercase tracking-[0.3em] text-[#8c6a43]">
      <span class="h-px w-8 bg-[#8c6a43]"></span>
      Hinina <span class="text-[#a7aaa6]">/</span> Contest client
    </p>

    <div class="flex flex-wrap items-end gap-x-5 gap-y-2">
      <h1 id="event-title" class="max-w-full text-[clamp(2rem,4.1vw,3.8rem)] font-bold leading-[1.16] tracking-[0.035em] text-[#2e3033]">
        {{ title }}
      </h1>
      <span class="mb-1 inline-flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.18em] text-[#5f7a6a]">
        <span class="h-1.5 w-1.5 rounded-full bg-[#5f7a6a]"></span>
        {{ phaseLabel }}
      </span>
    </div>

    <div class="mt-5 h-px w-full max-w-[640px] bg-gradient-to-r from-[#9da4a1]/70 via-[#9da4a1]/35 to-transparent"></div>
    <p class="mt-5 max-w-lg text-sm leading-7 tracking-[0.025em] text-[#68706f]">
      {{ description }}
    </p>
  </section>
</template>
