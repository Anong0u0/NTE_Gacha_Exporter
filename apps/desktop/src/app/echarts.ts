import { PieChart } from "echarts/charts";
import { TooltipComponent } from "echarts/components";
import { use } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";

export function installEcharts() {
  use([PieChart, TooltipComponent, CanvasRenderer]);
}
