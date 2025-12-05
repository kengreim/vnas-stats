import { createEffect, createMemo, createSignal, For, onCleanup } from "solid-js";
import { useQuery } from "@tanstack/solid-query";
import { createStore, reconcile } from "solid-js/store";
import dayjs from "dayjs";
import utc from "dayjs/plugin/utc";
import { IronMicResponse } from "@/bindings";

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

const ironMicApiUrl = (start: string, end: string) => {
  const apiBase = import.meta.env.VITE_API_BASE_URL ?? "http://localhost:8080";
  const base = apiBase.replace(/\/$/, "");

  return `${base}/v1/callsigns/top?start=${encodeURIComponent(
    start,
  )}&end=${encodeURIComponent(end)}`;
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

  const query = useQuery(() => ({
    queryKey: ["iron-mic", start(), end()],
    queryFn: async (): Promise<IronMicResponse> => {
      const resp = await fetch(ironMicApiUrl(start(), end()));
      if (!resp.ok) {
        throw new Error(`Failed to load stats: ${resp.status}`);
      }
      setNextRefreshAt(Date.now() + REFETCH_INTERVAL);
      return resp.json();
    },
    refetchInterval: REFETCH_INTERVAL,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    retry: false,
  }));

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
    <main class="h-dvh bg-background">
      {query.isPending && <p>Loading…</p>}
      {query.error && <p className="error">{(query.error as Error).message}</p>}

      {store && (
        <div class="flex justify-center space-x-16">
          <For each={["ground", "tower", "tracon", "center"] as CategoryKey[]}>
            {(key) => {
              return (
                <section class="card" aria-label={CATEGORY_LABELS[key]}>
                  <Card>
                    <CardHeader>
                      <CardTitle>{CATEGORY_LABELS[key]}</CardTitle>
                    </CardHeader>
                    <CardContent>
                      <Table class="text-md table-auto font-mono">
                        <TableHeader>
                          <TableRow>
                            <TableHead>#</TableHead>
                            <TableHead>Callsign</TableHead>
                            <TableHead class="text-right">Duration</TableHead>
                            <TableHead class="text-right">Uptime %</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          <For each={store[key]}>
                            {(item, index) => (
                              <TableRow>
                                <TableCell>{index() + 1}</TableCell>
                                <TableCell>
                                  <div
                                    class={cn("rounded-md p-1", {
                                      "bg-emerald-700 font-bold text-muted": item.isActive,
                                    })}
                                  >
                                    {item.prefix}_{item.suffix}
                                  </div>
                                </TableCell>
                                <TableCell class="text-right">
                                  {formatDuration(item.duration)}
                                </TableCell>
                                <TableCell class="text-right">
                                  {item.uptimePercent.toFixed(1)}%
                                </TableCell>
                              </TableRow>
                            )}
                          </For>
                        </TableBody>
                      </Table>
                    </CardContent>
                  </Card>
                </section>
              );
            }}
          </For>
        </div>
      )}
      <header>
        <div>
          <h1>Iron Mic · Current Month</h1>
          {query.data && (
            <p class="muted">
              Window: {formatDateUtc(query.data.start)} → {formatDateUtc(query.data.end)} ·{" "}
              {countdown() != null ? `Next update ~${countdown()}s` : "Window ended"}
            </p>
          )}
        </div>
        {query.isFetching && <span class="badge">Refreshing…</span>}
      </header>
    </main>
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
