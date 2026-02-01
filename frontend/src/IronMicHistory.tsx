import { createEffect, createSignal, For, Show } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
import { ironMicHistoryRoute } from "~/router.tsx";
import dayjs from "dayjs";
import utc from "dayjs/plugin/utc";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/Card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/Table";
import { cn } from "~/libs/cn.ts";
import { useIronMicStatsQuery } from "~/queries/iron-mic";
import { Layout } from "~/components/layout/Layout.tsx";

dayjs.extend(utc);

type CategoryKey = "ground" | "tower" | "tracon" | "center";

const CATEGORY_SUFFIXES: Record<CategoryKey, string[]> = {
  ground: ["GND"],
  tower: ["TWR"],
  tracon: ["APP", "DEP"],
  center: ["CTR"],
};

const CATEGORY_LABELS: Record<CategoryKey, string> = {
  ground: "Ground (GND)",
  tower: "Tower (TWR)",
  tracon: "Tracon (APP/DEP)",
  center: "Center (CTR)",
};

type LeaderboardItem = {
  id: string;
  prefix: string;
  suffix: string;
  duration: number;
  uptimePercent: number;
  isActive: boolean | null;
};

export default function IronMicHistory() {
  const params = ironMicHistoryRoute.useParams();

  const monthStart = () => dayjs.utc(`${params().year}-${params().month}-01`);
  const [start] = createSignal(monthStart().toISOString());
  const [end] = createSignal(monthStart().add(1, "month").toISOString());

  const [store, setStore] = createStore<Record<CategoryKey, LeaderboardItem[]>>({
    ground: [],
    tower: [],
    tracon: [],
    center: [],
  });

  const query = useIronMicStatsQuery(start, end, { refetchInterval: false });

  createEffect(() => {
    const data = query.data;
    if (!data) return;
    const elapsed = Math.max(data.actualElapsedDurationSeconds, 1);
    const next: Record<CategoryKey, LeaderboardItem[]> = {
      ground: [],
      tower: [],
      tracon: [],
      center: [],
    };
    for (const cs of data.callsigns) {
      const suffix = cs.suffix.toUpperCase();
      const uptimePercent = (cs.durationSeconds / elapsed) * 100;
      const item: LeaderboardItem = {
        id: `${cs.prefix}_${cs.suffix}`,
        prefix: cs.prefix,
        suffix,
        duration: cs.durationSeconds,
        uptimePercent,
        isActive: cs.isActive,
      };
      (Object.keys(CATEGORY_SUFFIXES) as CategoryKey[]).forEach((key) => {
        if (CATEGORY_SUFFIXES[key].includes(suffix)) {
          next[key].push(item);
        }
      });
    }
    (Object.keys(next) as CategoryKey[]).forEach((key) => {
      next[key].sort((a, b) => b.duration - a.duration);
      next[key] = next[key].slice(0, 25);
      setStore(key, reconcile(next[key], { key: "id", merge: true }));
    });
  });

  return (
    <Layout>
      <main class="flex-1 overflow-y-auto px-6 py-6">
        <Show when={query.data}>
          {(data) => (
            <div class="mb-4">
              <h1 class="text-2xl font-semibold">
                Iron Mic: {dayjs.utc(data().start).format("MMMM YYYY")}
              </h1>
              <p class="text-sm text-muted-foreground">
                {formatDateUtc(data().start)} → {formatDateUtc(data().end)}
              </p>
            </div>
          )}
        </Show>
        <div class="flex flex-col gap-6">
          <Show when={query.error}>
            {(err) => <p class="text-destructive">Error: {(err() as Error).message}</p>}
          </Show>
          <div class="grid gap-6 md:grid-cols-2 xl:grid-cols-4">
            <For each={["ground", "tower", "tracon", "center"] as CategoryKey[]}>
              {(key) => (
                <Card>
                  <CardHeader>
                    <CardTitle>{CATEGORY_LABELS[key]}</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <Table class="table-auto font-mono text-sm">
                      <TableHeader>
                        <TableRow>
                          <TableHead>#</TableHead>
                          <TableHead>Callsign</TableHead>
                          <TableHead class="text-right">Duration</TableHead>
                          <TableHead class="text-right">Uptime %</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody class="text-xs 2xl:text-sm">
                        <For each={store[key]}>
                          {(item, index) => (
                            <TableRow>
                              <TableCell class="py-1">{index() + 1}</TableCell>
                              <TableCell class="py-1">
                                <div
                                  class={cn("w-fit rounded-md px-2 py-1", {
                                    "bg-muted text-foreground": true,
                                  })}
                                >
                                  {item.prefix}_{item.suffix}
                                </div>
                              </TableCell>
                              <TableCell class="py-1 text-right">
                                {formatDuration(item.duration)}
                              </TableCell>
                              <TableCell class="py-1 text-right">
                                {item.uptimePercent.toFixed(1)}%
                              </TableCell>
                            </TableRow>
                          )}
                        </For>
                        <Show when={store[key].length === 0}>
                          <TableRow>
                            <TableCell class="py-4 text-sm text-muted-foreground" colSpan={4}>
                              No data for this window.
                            </TableCell>
                          </TableRow>
                        </Show>
                      </TableBody>
                    </Table>
                  </CardContent>
                </Card>
              )}
            </For>
          </div>
        </div>
      </main>
    </Layout>
  );
}

function formatDuration(seconds: number): string {
  const hrs = Math.floor(seconds / 3600);
  const mins = String(Math.floor((seconds % 3600) / 60)).padStart(2, "0");
  return `${hrs}h ${mins}m`;
}

function formatDateUtc(value: string): string {
  return dayjs.utc(value).format("YYYY-MM-DD HH:mm[Z]");
}
