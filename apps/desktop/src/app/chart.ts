import { init, type ECharts } from "echarts/core";
import type { Ref } from "vue";
import type { DashboardOverview } from "../api";

export function createChartTools(chartEl: Ref<HTMLElement | null>, summary: Ref<DashboardOverview | null>) {
  let chart: ECharts | null = null;

  function renderChart() {
    if (!chartEl.value || !summary.value) return;
    chart ??= init(chartEl.value);
    chart.setOption({
      animationDuration: 220,
      grid: { top: 12, right: 10, bottom: 24, left: 34 },
      tooltip: { trigger: "axis" },
      xAxis: {
        type: "category",
        data: summary.value.rarity_distribution.map((bucket) => `${bucket.rarity}★`),
        axisTick: { show: false },
      },
      yAxis: { type: "value", splitLine: { lineStyle: { color: "#e1e6e2" } } },
      series: [
        {
          type: "bar",
          data: summary.value.rarity_distribution.map((bucket) => bucket.count),
          itemStyle: { color: "#2d6d64", borderRadius: [3, 3, 0, 0] },
        },
      ],
    });
  }

  function disposeChart() {
    chart?.dispose();
  }

  return { renderChart, disposeChart };
}
