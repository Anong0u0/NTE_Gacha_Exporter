import { init, type ECharts } from "echarts/core";
import type { Ref } from "vue";
import type { DashboardSelectionDetail } from "../api";
import type { I18nKey } from "./i18n";
import { rarityColor } from "./rarityColors";
import { dashboardRaritySlices } from "./rarityBuckets";

type Translator = (key: I18nKey) => string;

export function createChartTools(chartEl: Ref<HTMLElement | null>, detail: Ref<DashboardSelectionDetail | null>, t: Translator) {
  let chart: ECharts | null = null;
  let chartElement: HTMLElement | null = null;
  let resizeObserver: ResizeObserver | null = null;

  function disposeChartInstance() {
    resizeObserver?.disconnect();
    resizeObserver = null;
    chart?.dispose();
    chart = null;
    chartElement = null;
  }

  function renderChart() {
    if (!chartEl.value || !detail.value) return;
    if (chartElement !== chartEl.value) {
      disposeChartInstance();
      chartElement = chartEl.value;
      chart = init(chartElement);
      resizeObserver = new ResizeObserver(() => chart?.resize());
      resizeObserver.observe(chartElement);
    }
    const activeChart = chart;
    if (!activeChart) return;
    const data = dashboardRaritySlices(detail.value, t).map((bucket) => ({
      name: bucket.label,
      value: bucket.count,
      percent: bucket.percent,
      itemStyle: { color: rarityColor(bucket.rarity) },
    }));
    activeChart.setOption({
      animationDuration: 220,
      tooltip: {
        trigger: "item",
        formatter: (params: { name?: string; value?: number; data?: { percent?: number } }) => {
          const percent = params.data?.percent == null ? "-" : `${(params.data.percent * 100).toFixed(1)}%`;
          return `${params.name ?? ""}: ${params.value ?? 0} (${percent})`;
        },
      },
      series: [
        {
          type: "pie",
          radius: "66%",
          center: ["48%", "52%"],
          avoidLabelOverlap: true,
          label: {
            show: true,
            formatter: "{b}",
            color: "#33423d",
            fontSize: 12,
          },
          labelLine: {
            show: true,
            length: 18,
            length2: 14,
            lineStyle: { color: "#9aa8a1" },
          },
          data,
          itemStyle: { borderColor: "#ffffff", borderWidth: 2 },
        },
      ],
    });
  }

  function disposeChart() {
    disposeChartInstance();
  }

  return { renderChart, disposeChart };
}
