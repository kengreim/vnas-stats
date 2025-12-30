import { createEffect, createSignal, For, Show, onCleanup } from "solid-js";
import { createStore, reconcile } from "solid-js/store";
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

export default function App() {
  const REFETCH_INTERVAL = 60_000;

  const monthStart = dayjs.utc().startOf("month");
  const [start, setStart] = createSignal(monthStart.toISOString());
  const [end, setEnd] = createSignal(monthStart.add(1, "month").toISOString());

  const [nextRefreshAt, setNextRefreshAt] = createSignal<number | null>(null);
  const [countdown, setCountdown] = createSignal<number | null>(null);
  const [store, setStore] = createStore<Record<CategoryKey, LeaderboardItem[]>>({
    ground: [],
    tower: [],
    tracon: [],
    center: [],
  });

  const query = useIronMicStatsQuery(start, end);

  createEffect(() => {
    if (query.dataUpdatedAt) {
      setNextRefreshAt(query.dataUpdatedAt + REFETCH_INTERVAL);
    }
  });

  createEffect(() => {
    const id = setInterval(() => {
      const monthStart = dayjs.utc().startOf("month");
      const new_start = monthStart.toISOString();
      if (new_start != start()) {
        setStart(new_start);
        setEnd(monthStart.add(1, "month").toISOString());
      }
    }, 60_000);

    onCleanup(() => clearInterval(id));
  });

  createEffect(() => {
    const target = nextRefreshAt();
    if (!target) {
      setCountdown(null);
      return;
    }
    const update = () => {
      const diff = target - Date.now();
      setCountdown(Math.max(0, Math.ceil(diff / 1000)));
    };
    update();
    const id = setInterval(update, 1000);

    onCleanup(() => clearInterval(id));
  });

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
                {countdown() != null && (
                  <>
                    {" "}
                    · Refreshing in{" "}
                    <span class="font-semibold text-foreground">{countdown()}s</span>
                  </>
                )}
              </p>
            </div>
          )}
        </Show>
        <div class="flex flex-col gap-6">
          {/*<ActivityChart start={start()} end={end()} />*/}
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
                                    "bg-emerald-700 font-bold text-primary-foreground":
                                      item.isActive,
                                    "bg-muted text-foreground": !item.isActive,
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
