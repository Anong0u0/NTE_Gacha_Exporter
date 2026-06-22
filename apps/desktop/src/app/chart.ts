import { init, type ECharts } from "echarts/core";
import type { Ref } from "vue";
import type { DashboardSelectionDetail } from "../api";

type ChartRarityBucket = {
  key: string;
  rarity: number;
  label: string;
  count: number;
  percent: number;
};

export function createChartTools(chartEl: Ref<HTMLElement | null>, detail: Ref<DashboardSelectionDetail | null>) {
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
    const rarityColor = (bucket: ChartRarityBucket) => {
      if (bucket.key === "5-up") return "#2d6d64";
      const rarity = bucket.rarity;
      if (rarity === 5) return "#2d6d64";
      if (rarity === 4) return "#efc45a";
      if (rarity === 3) return "#8aa39b";
      return "#c3cec7";
    };
    const data = displayChartBuckets(detail.value).map((bucket) => ({
      name: bucket.label,
      value: bucket.count,
      percent: bucket.percent,
      itemStyle: { color: rarityColor(bucket) },
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

function displayChartBuckets(detail: DashboardSelectionDetail): ChartRarityBucket[] {
  const hitBuckets = detail.hit_rarity_distribution ?? [];
  const sourceBuckets = hitBuckets.length ? hitBuckets : detail.rarity_distribution;
  const upCount = detail.summary.up_count ?? 0;
  const fiveBucket = sourceBuckets.find((bucket) => bucket.rarity === 5);
  const fiveCount = upCount > 0 ? upCount : (fiveBucket?.count ?? 0);
  const buckets: Array<Omit<ChartRarityBucket, "percent">> = [];
  if (fiveCount > 0) {
    buckets.push({
      key: upCount > 0 ? "5-up" : "5",
      rarity: 5,
      label: upCount > 0 ? "5★UP" : "5★",
      count: fiveCount,
    });
  }
  for (const bucket of sourceBuckets) {
    if (bucket.rarity === 5) continue;
    buckets.push({
      key: String(bucket.rarity),
      rarity: bucket.rarity,
      label: `${bucket.rarity}★`,
      count: bucket.count,
    });
  }
  const total = buckets.reduce((sum, bucket) => sum + bucket.count, 0);
  return buckets.map((bucket) => ({
    ...bucket,
    percent: total > 0 ? bucket.count / total : 0,
  }));
}
