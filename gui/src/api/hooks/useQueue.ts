import { useQuery } from "@tanstack/react-query";
import { get } from "../client";
import type { QueueDashboard, QueueJobListResponse } from "../types";

/**
 * Queue Dashboard 取得
 */
export function useQueueDashboard() {
  return useQuery({
    queryKey: ["queue", "dashboard"],
    queryFn: () => get<QueueDashboard>("/api/queue/dashboard"),
  });
}

/**
 * Queue Job 一覧取得
 */
export function useQueueJobs(params?: {
  limit?: number;
  offset?: number;
  status?: string;
}) {
  const searchParams = new URLSearchParams();
  if (params?.limit) searchParams.set("limit", String(params.limit));
  if (params?.offset) searchParams.set("offset", String(params.offset));
  if (params?.status) searchParams.set("status", params.status);

  const query = searchParams.toString();
  const path = query ? `/api/queue/jobs?${query}` : "/api/queue/jobs";

  return useQuery({
    queryKey: ["queue", "jobs", params],
    queryFn: () => get<QueueJobListResponse>(path),
  });
}
