import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { get, post, postFireAndForget } from "../client";
import type {
  MatchCandidatesResponse,
  FeedbackRequest,
  FeedbackResponse,
  InteractionEventRequest,
} from "../types";

/**
 * プロジェクトの候補一覧取得
 */
export function useCandidates(projectId: number) {
  return useQuery({
    queryKey: ["candidates", projectId],
    queryFn: () =>
      get<MatchCandidatesResponse>(`/api/projects/${projectId}/candidates`),
    enabled: projectId > 0,
  });
}

/**
 * フィードバック送信
 */
export function useSendFeedback() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (request: FeedbackRequest) =>
      post<FeedbackResponse>("/api/feedback", request),
    onSuccess: () => {
      // 候補一覧をリフレッシュ
      queryClient.invalidateQueries({ queryKey: ["candidates"] });
      // Job詳細もリフレッシュ（JobDetailPageから使われる場合）
      queryClient.invalidateQueries({ queryKey: ["queue", "job"] });
    },
  });
}

/**
 * 行動イベント送信（Fire-and-forget）
 * UXをブロックしない
 */
export function sendInteractionEvent(request: InteractionEventRequest): void {
  postFireAndForget("/api/interactions/events", {
    source: "gui",
    ...request,
  });
}

/**
 * 詳細閲覧イベント
 */
export function trackViewedDetail(interactionId: number): void {
  sendInteractionEvent({
    interactionId,
    eventType: "viewed_candidate_detail",
    idempotencyKey: `view-${interactionId}-${Date.now()}`,
  });
}

/**
 * コピーイベント
 */
export function trackCopiedTemplate(
  interactionId: number,
  template?: string
): void {
  sendInteractionEvent({
    interactionId,
    eventType: "copied_template",
    idempotencyKey: `copy-${interactionId}-${Date.now()}`,
    meta: template ? { template } : undefined,
  });
}

/**
 * 連絡クリックイベント
 */
export function trackClickedContact(
  interactionId: number,
  method?: string
): void {
  sendInteractionEvent({
    interactionId,
    eventType: "clicked_contact",
    idempotencyKey: `contact-${interactionId}-${Date.now()}`,
    meta: method ? { method } : undefined,
  });
}

/**
 * ショートリストイベント
 */
export function trackShortlisted(
  interactionId: number,
  active: boolean
): void {
  sendInteractionEvent({
    interactionId,
    eventType: "shortlisted",
    idempotencyKey: `shortlist-${interactionId}`, // 同一IDで上書き
    meta: { active },
  });
}
