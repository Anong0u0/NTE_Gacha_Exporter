<script setup lang="ts">
import type { Component } from "vue";

type SummaryMetricTone = "default" | "rarity-5" | "rarity-4" | "currency";

withDefaults(defineProps<{
  icon: Component;
  title: string;
  value: string | number;
  tone?: SummaryMetricTone;
  progressWidth?: string;
}>(), {
  tone: "default",
});
</script>

<template>
  <div class="summary-metric-card" :class="`summary-metric-card--${tone}`">
    <div class="summary-metric-header">
      <div class="summary-metric-title">
        <component :is="icon" class="summary-metric-icon" :size="15" aria-hidden="true" />
        <span>{{ title }}</span>
      </div>
      <strong class="summary-metric-value">{{ value }}</strong>
    </div>
    <div v-if="progressWidth" class="summary-metric-progress" aria-hidden="true">
      <span :style="{ width: progressWidth }"></span>
    </div>
  </div>
</template>
