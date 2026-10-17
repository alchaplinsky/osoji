import { ChartPie, LayoutDashboard, Orbit, Rows3 } from "lucide-react";
import type { Viz } from "../../lib/types";
import { SegmentedControl, type SegmentOption } from "../../components/ui";

const ICON = 14;

const OPTIONS: SegmentOption<Viz>[] = [
  { value: "sunburst", label: "Sunburst", icon: <ChartPie size={ICON} /> },
  { value: "treemap", label: "Treemap", icon: <LayoutDashboard size={ICON} /> },
  { value: "shelf", label: "Shelf", icon: <Rows3 size={ICON} /> },
  { value: "orbit", label: "Orbit", icon: <Orbit size={ICON} /> },
];

type VizSwitcherProps = { viz: Viz; onViz: (v: Viz) => void; className?: string };

/** Icon-only picker for the chart type; compact enough to sit in a corner of the chart stage. */
export default function VizSwitcher({ viz, onViz, className }: VizSwitcherProps) {
  return <SegmentedControl options={OPTIONS} value={viz} onChange={onViz} className={className} />;
}
