import { createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { useQuery } from "@tanstack/solid-query";
import dayjs from "dayjs";
import utc from "dayjs/plugin/utc";
import "./index.css";
import { IronMicResponse } from "~/bindings.ts";

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

  const grouped = createMemo(() => {
    const data = query.data;
    if (!data) return null;
    const elapsed = Math.max(data.actualElapsedDurationSeconds, 1);
    const map: Record<CategoryKey, LeaderboardItem[]> = {
      ground: [],
      tower: [],
      tracon: [],
      center: [],
    };
    for (const cs of data.callsigns) {
      const suffix = cs.suffix.toUpperCase();
      const uptimePercent = (cs.durationSeconds / elapsed) * 100;
      const item: LeaderboardItem = {
        prefix: cs.prefix,
        suffix,
        duration: cs.durationSeconds,
        uptimePercent,
        isActive: cs.isActive,
      };
      (Object.keys(CATEGORY_SUFFIXES) as CategoryKey[]).forEach((key) => {
        if (CATEGORY_SUFFIXES[key].includes(suffix)) {
          map[key].push(item);
        }
      });
    }
    (Object.keys(map) as CategoryKey[]).forEach((key) => {
      map[key].sort((a, b) => b.duration - a.duration);
    });
    return map;
  });

  return (
    <main class="page">
      <header class="header">
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

      {query.isPending && <p>Loading…</p>}
      {query.error && <p class="error">{(query.error as Error).message}</p>}

      {grouped() && (
        <div class="grid">
          {(["ground", "tower", "tracon", "center"] as CategoryKey[]).map((key) => {
            const items = grouped()![key] ?? [];
            return (
              <section class="card" aria-label={CATEGORY_LABELS[key]}>
                <div class="card-header">
                  <h2>{CATEGORY_LABELS[key]}</h2>
                </div>
                {items.length === 0 ? (
                  <p class="muted">No sessions recorded.</p>
                ) : (
                  <table class="table">
                    <thead>
                      <tr>
                        <th>#</th>
                        <th>Callsign</th>
                        <th class="right">Duration</th>
                        <th class="right">Uptime%</th>
                      </tr>
                    </thead>
                    <tbody>
                      {items.slice(0, 25).map((item, idx) => (
                        <tr>
                          <td>{idx + 1}</td>
                          <td>
                            <span class={`pill ${item.isActive ? "live" : ""}`}>
                              {item.prefix}_{item.suffix}
                            </span>
                          </td>
                          <td class="right">{formatDuration(item.duration)}</td>
                          <td class="right">{item.uptimePercent.toFixed(1)}%</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </section>
            );
          })}
        </div>
      )}
    </main>
  );
}

function formatDuration(seconds: number): string {
  const hrs = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return `${hrs}h ${mins}m`;
}

function formatDateUtc(value: string): string {
  return dayjs.utc(value).format("YYYY-MM-DD HH:mm[Z]");
}
