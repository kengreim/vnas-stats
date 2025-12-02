import {useEffect, useMemo, useState} from "react";
import {Box, Card, Container, Flex, Group, Loader, SimpleGrid, Stack, Table, Text, Title} from "@mantine/core";
import dayjs from "dayjs";
import utc from "dayjs/plugin/utc";
import type {IronMicResponse} from "~/bindings";

dayjs.extend(utc);

export function meta() {
  return [
    {title: "Iron Mic - Current Month"},
    {name: "description", content: "Top VNAS callsign sessions for the current month"},
  ];
}

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

export default function Home() {
  const [data, setData] = useState<IronMicResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [nextRefreshAt, setNextRefreshAt] = useState<number | null>(null);
  const [countdown, setCountdown] = useState<number | null>(null);

  useEffect(() => {
    let timer: ReturnType<typeof setInterval> | undefined;

    const fetchData = async () => {
      try {
        const now = dayjs.utc();
        const start = now.startOf("month");
        const end = start.add(1, "month");
        const apiBase = import.meta.env.VITE_API_BASE_URL ?? "http://localhost:8080";
        const base = apiBase.replace(/\/$/, "");
        const url = `${base}/v1/callsigns/top?start=${encodeURIComponent(
          start.toISOString()
        )}&end=${encodeURIComponent(end.toISOString())}`;
        const resp = await fetch(url, {headers: {Accept: "application/json"}});
        if (!resp.ok) {
          throw new Error(`Failed to load stats: ${resp.status}`);
        }
        const json = await resp.json() as IronMicResponse;
        setData(json);

        // schedule refresh if still within window
        const windowEnd = dayjs.utc(json.end);
        if (windowEnd.isAfter(dayjs.utc())) {
          if (!timer) {
            timer = setInterval(fetchData, 60_000);
          }
          setNextRefreshAt(Date.now() + 60_000);
        } else if (timer) {
          clearInterval(timer);
          timer = undefined;
          setNextRefreshAt(null);
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : "Unknown error");
      } finally {
        setLoading(false);
      }
    };
    void fetchData();

    return () => {
      if (timer) {
        clearInterval(timer);
      }
    };
  }, []);

  // countdown ticker
  useEffect(() => {
    if (nextRefreshAt == null) {
      setCountdown(null);
      return;
    }
    const update = () => {
      const diff = nextRefreshAt - Date.now();
      setCountdown(Math.max(0, Math.ceil(diff / 1000)));
    };
    update();
    const tick = setInterval(update, 1000);
    return () => clearInterval(tick);
  }, [nextRefreshAt]);

  const grouped = useMemo(() => {
    if (!data) return null;
    const elapsed = Math.max(data.actualElapsedDurationSeconds, 1);
    return groupByCategory(data.callsigns, elapsed);
  }, [data]);

  return (
    <Flex justify={"center"}>
      <Stack gap="md">
        <Group justify="space-between">
          <div>
            <Title order={2}>Iron Mic · Current Month</Title>
            {data && (
              <Text c="dimmed" size="sm">
                Window: {formatDate(data.start)} → {formatDate(data.end)} ·{" "}
                {countdown != null ? `Next update in ${countdown}s` : "Window ended"}
              </Text>
            )}
          </div>
        </Group>

        {loading && (
          <Flex justify="center" py="xl">
            <Loader/>
          </Flex>
        )}
        {error && (
          <Card withBorder shadow="sm" c="red">
            <Text>{error}</Text>
          </Card>
        )}
        {grouped && (
          <SimpleGrid
            cols={4}
            spacing="md"
          >
            {(["ground", "tower", "tracon", "center"] as CategoryKey[]).map((key) => {
              const items = grouped[key] ?? [];
              return (
                <Card key={key} withBorder radius="md" shadow="sm">
                  <Stack gap="xs">
                    <Title order={4}>{CATEGORY_LABELS[key]}</Title>
                    {items.length === 0 ? (
                      <Text c="dimmed" size="sm">
                        No sessions recorded.
                      </Text>
                    ) : (
                      <Table withRowBorders={true} horizontalSpacing="xs" verticalSpacing="xs">
                        <Table.Thead>
                          <Table.Tr>
                            <Table.Th>#</Table.Th>
                            <Table.Th>Callsign</Table.Th>
                            <Table.Th ta={"center"}>Duration</Table.Th>
                            <Table.Th ta={"center"}>Uptime%</Table.Th>
                          </Table.Tr>
                        </Table.Thead>
                        <Table.Tbody>
                          {items.slice(0, 25).map((item, idx) => (
                            <Table.Tr key={`${item.prefix}_${item.suffix}`}>
                              <Table.Td>{idx + 1}</Table.Td>
                              <Table.Td>
                                <Text
                                  fw={600}
                                  c={item.is_active ? "white" : undefined}
                                  bg={item.is_active ? "teal.7" : undefined}
                                  px="xs"
                                  py={4}
                                  style={{borderRadius: 6, display: "inline-block"}}
                                >
                                  {item.prefix}_{item.suffix}
                                </Text>
                              </Table.Td>
                              <Table.Td ta={"center"}>
                                {formatDuration(item.duration_seconds)}
                              </Table.Td>
                              <Table.Td ta={"center"}>
                                {item.uptimePercent.toFixed(1)}%
                              </Table.Td>
                            </Table.Tr>
                          ))}
                        </Table.Tbody>
                      </Table>
                    )}
                  </Stack>
                </Card>
              );
            })}
          </SimpleGrid>
        )}
      </Stack>
    </Flex>
  );
}

type LeaderboardItem = {
  prefix: string;
  suffix: string;
  duration_seconds: number;
  is_active: boolean | null;
  uptimePercent: number;
};

function groupByCategory(
  callsigns: IronMicResponse["callsigns"],
  elapsed: number
): Record<CategoryKey, LeaderboardItem[]> {
  const result: Record<CategoryKey, LeaderboardItem[]> = {
    ground: [],
    tower: [],
    tracon: [],
    center: [],
  };

  for (const cs of callsigns) {
    const suffix = cs.suffix.toUpperCase();
    const uptimePercent = (cs.durationSeconds / elapsed) * 100;
    const item: LeaderboardItem = {
      prefix: cs.prefix,
      suffix,
      duration_seconds: cs.durationSeconds,
      is_active: cs.isActive,
      uptimePercent,
    };

    (Object.keys(CATEGORY_SUFFIXES) as CategoryKey[]).forEach((key) => {
      if (CATEGORY_SUFFIXES[key].includes(suffix)) {
        result[key].push(item);
      }
    });
  }

  // sort each category descending duration
  (Object.keys(result) as CategoryKey[]).forEach((key) => {
    result[key].sort((a, b) => b.duration_seconds - a.duration_seconds);
  });

  return result;
}

function formatDuration(seconds: number): string {
  const hrs = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return `${hrs}h ${mins}m`;
}

function formatDate(value: string): string {
  return dayjs(value).format("YYYY-MM-DD HH:mm");
}
