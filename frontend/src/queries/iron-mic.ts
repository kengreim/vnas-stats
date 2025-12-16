import { useQuery } from "@tanstack/solid-query";
import { IronMicResponse } from "@/bindings";

const REFETCH_INTERVAL = 60_000;
const DEFAULT_API_BASE_URL = "http://localhost:8080";

const getIronMicApiUrl = (start: string, end: string) => {
  const apiBase = import.meta.env.VITE_API_BASE_URL ?? DEFAULT_API_BASE_URL;
  const base = apiBase.replace(/\/$/, "");

  return `${base}/v1/callsigns/top?start=${encodeURIComponent(
    start,
  )}&end=${encodeURIComponent(end)}`;
};

export const fetchIronMicStats = async (start: string, end: string): Promise<IronMicResponse> => {
  const resp = await fetch(getIronMicApiUrl(start, end));
  if (!resp.ok) {
    throw new Error(`Failed to load stats: ${resp.status}`);
  }
  return resp.json();
};

export function useIronMicStatsQuery(start: () => string, end: () => string) {
  return useQuery(() => ({
    queryKey: ["iron-mic", start(), end()],
    queryFn: () => fetchIronMicStats(start(), end()),
    refetchInterval: REFETCH_INTERVAL,
    refetchOnWindowFocus: false,
    refetchOnReconnect: false,
    retry: false,
  }));
}
