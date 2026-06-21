import { init, type ECharts } from "echarts/core";
import type { Ref } from "vue";
import type { DashboardSelectionDetail } from "../api";

export function createChartTools(chartEl: Ref<HTMLElement | null>, detail: Ref<DashboardSelectionDetail | null>) {
  let chart: ECharts | null = null;

  function renderChart() {
    if (!chartEl.value || !detail.value) return;
    chart ??= init(chartEl.value);
    chart.setOption({
      animationDuration: 220,
      grid: { top: 12, right: 10, bottom: 24, left: 34 },
      tooltip: { trigger: "axis" },
      xAxis: {
        type: "category",
        data: detail.value.rarity_distribution.map((bucket) => `${bucket.rarity}★`),
        axisTick: { show: false },
      },
      yAxis: { type: "value", splitLine: { lineStyle: { color: "#e1e6e2" } } },
      series: [
        {
          type: "bar",
          data: detail.value.rarity_distribution.map((bucket) => bucket.count),
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
