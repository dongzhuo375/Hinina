<script setup lang="ts">
defineProps<{
  title: string
}>()
</script>

<template>
  <section aria-labelledby="event-title" class="max-w-[640px]">
    <h1 id="event-title" class="max-w-full break-words text-[clamp(2rem,4.1vw,3.8rem)] font-semibold leading-[1.16] tracking-[0.035em] text-[#2e3033]">
      {{ title }}
    </h1>
    <div class="mt-4 h-px w-full bg-[#9da4a1]/55"></div>
  </section>
</template>
