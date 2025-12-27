import { useQuery, useMutation, useQueryClient, useInfiniteQuery } from "@tanstack/react-query";
import { get, post } from "../client";
import type {
  QueueDashboard,
  QueueJobDetailResponse,
  QueueJobListResponse,
  QueueJobStatus,
} from "../types";

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
  status?: QueueJobStatus;
  requiresManualReview?: boolean;
}) {
  const searchParams = new URLSearchParams();
  if (params?.limit) searchParams.set("limit", String(params.limit));
  if (params?.offset) searchParams.set("offset", String(params.offset));
  if (params?.status) searchParams.set("status", params.status);
  if (params?.requiresManualReview !== undefined) {
    searchParams.set(
      "requires_manual_review",
      String(params.requiresManualReview),
    );
  }

  const query = searchParams.toString();
  const path = query ? `/api/queue/jobs?${query}` : "/api/queue/jobs";

  return useQuery({
    queryKey: ["queue", "jobs", params],
    queryFn: () => get<QueueJobListResponse>(path),
  });
}

export function useInfiniteQueueJobs(params?: {
  limit?: number;
  status?: QueueJobStatus;
  requiresManualReview?: boolean;
}) {
  const baseSearchParams = new URLSearchParams();
  if (params?.status) baseSearchParams.set("status", params.status);
  if (params?.requiresManualReview !== undefined) {
    baseSearchParams.set(
      "requires_manual_review",
      String(params.requiresManualReview),
    );
  }

  const limit = params?.limit ?? 50;
  const queryKey = [
    "queue",
    "jobs",
    "infinite",
    limit,
    params?.status ?? null,
    params?.requiresManualReview ?? null,
  ] as const;

  return useInfiniteQuery({
    queryKey,
    initialPageParam: 0,
    queryFn: ({ pageParam }) => {
      const searchParams = new URLSearchParams(baseSearchParams);
      searchParams.set("limit", String(limit));
      searchParams.set("offset", String(pageParam));

      const query = searchParams.toString();
      const path = query ? `/api/queue/jobs?${query}` : "/api/queue/jobs";

      return get<QueueJobListResponse>(path);
    },
    getNextPageParam: (lastPage) =>
      lastPage.hasMore ? lastPage.offset + lastPage.limit : undefined,
  });
}

/**
 * Job 詳細取得
 */
export function useJobDetail(jobId: number | string | undefined) {
  const include = "entity,matches,feedback,events";
  return useQuery({
    queryKey: ["queue", "job", jobId],
    queryFn: () => get<QueueJobDetailResponse>(`/api/queue/jobs/${jobId}?include=${include}`),
    enabled: !!jobId,
  });
}

/**
 * Job リトライ
 */
export function useRetryJob() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (jobId: number | string) =>
      post<{ success: boolean; status: QueueJobStatus }>(`/api/queue/retry/${jobId}`, {}),
    onSuccess: (_data, jobId) => {
      queryClient.invalidateQueries({ queryKey: ["queue", "job", jobId] });
      queryClient.invalidateQueries({ queryKey: ["queue", "jobs"] });
      queryClient.invalidateQueries({ queryKey: ["queue", "dashboard"] });
    },
  });
}
