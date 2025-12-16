import { createMemo, Show } from "solid-js";
import { useQuery } from "@tanstack/solid-query";
import { SolidUplot } from "@dschz/solid-uplot";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/Card.tsx";
import { ActivityTimeSeriesResponse } from "~/bindings.ts";

const activityApiUrl = (start: string, end: string) => {
  const apiBase = import.meta.env.VITE_API_BASE_URL ?? "http://localhost:8080";
  const base = apiBase.replace(/\/$/, "");
  return `${base}/v1/activity/timeseries?start=${encodeURIComponent(
    start,
  )}&end=${encodeURIComponent(end)}`;
};

type ActivityChartProps = {
  start: string;
  end: string;
};

export function ActivityChart(props: ActivityChartProps) {
  const query = useQuery(() => ({
    queryKey: ["activity-timeseries", props.start, props.end],
    queryFn: async (): Promise<ActivityTimeSeriesResponse> => {
      const resp = await fetch(activityApiUrl(props.start, props.end));
      if (!resp.ok) {
        throw new Error(`Failed to load activity stats: ${resp.status}`);
      }
      return resp.json();
    },
    refetchInterval: 60_000,
  }));

  const chartData = createMemo(() => {
    const d = query.data;
    if (!d) return null;

    const timestamps = d.observations.map((ts) => new Date(ts).getTime() / 1000);
    return [timestamps, d.activeControllers, d.activeCallsigns, d.activePositions];
  });

  const opts: uPlot.Options = {
    width: 800,
    height: 400,
    series: [
      {},
      {
        label: "Controllers",
        stroke: "rgb(52, 211, 153)",
        width: 2,
        fill: "rgba(52, 211, 153, 0.1)",
      },
      {
        label: "Callsigns",
        stroke: "rgb(59, 130, 246)",
        width: 2,
        fill: "rgba(59, 130, 246, 0.1)",
      },
      {
        label: "Positions",
        stroke: "rgb(249, 115, 22)",
        width: 2,
        fill: "rgba(249, 115, 22, 0.1)",
      },
    ],
    axes: [
      {
        stroke: "rgb(156, 163, 175)",
        grid: { show: false },
      },
      {
        stroke: "rgb(156, 163, 175)",
        grid: { stroke: "rgba(156, 163, 175, 0.2)" },
      },
    ],
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle>Activity</CardTitle>
      </CardHeader>
      <CardContent>
        <Show when={query.isLoading}>
          <p>Loading chart...</p>
        </Show>
        <Show when={query.error}>
          {(err) => <p class="text-destructive">Error: {(err() as Error).message}</p>}
        </Show>
        <Show when={chartData()}>
          {(data) => (
            <div class="h-[400px] w-full">
              {/*This is a hack because the uplot chart doesn't seem to resize automatically*/}
              <SolidUplot opts={opts} data={data()} reflow={true} />
            </div>
          )}
        </Show>
      </CardContent>
    </Card>
  );
}
