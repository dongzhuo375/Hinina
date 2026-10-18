<script setup lang="ts">
defineProps<{ title: string; index: string; subtitle: string }>()
</script>

<template>
  <header class="page-heading">
    <div class="min-w-0">
      <p class="eyebrow mb-3">{{ index }} / {{ subtitle }}</p>
      <h1 class="page-title">{{ title }}</h1>
      <div v-if="$slots.description" class="page-description"><slot name="description" /></div>
    </div>
    <slot />
  </header>
</template>
