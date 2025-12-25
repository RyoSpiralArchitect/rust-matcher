import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { get, post, postFireAndForget } from "../client";
import type {
  MatchCandidatesResponse,
  FeedbackRequest,
  FeedbackResponse,
  InteractionEventRequest,
  MatchCandidate,
  KoResult,
} from "../types";

type RawMatchResponse = {
  talent_id: number;
  project_id: number;
  interaction_id: number;
  score: number;
  score_breakdown?: { business_total?: number };
  two_tower_score?: number | null;
  ko_decisions?: Record<string, { ko_type?: string }>;
  ko_reasons?: string[];
  manual_review_required: boolean;
  matched_at: string;
};

function toKoResult(raw: RawMatchResponse): KoResult {
  const decisions = Object.values(raw.ko_decisions ?? {});
  const isHardKo = decisions.some((d) => d.ko_type === "hard_ko");
  const isSoftKo = !isHardKo && decisions.some((d) => d.ko_type === "soft_ko");

  return {
    isHardKo,
    isSoftKo,
    needsManualReview: raw.manual_review_required,
    reasons: raw.ko_reasons ?? [],
  };
}

function mapCandidate(raw: RawMatchResponse): MatchCandidate {
  return {
    talentId: raw.talent_id,
    talentName: null,
    skills: [],
    desiredPriceMin: null,
    residentialTodofuken: null,
    availabilityDate: null,
    totalScore: raw.score ?? 0,
    businessScore: raw.score_breakdown?.business_total ?? raw.score ?? 0,
    twoTowerScore: raw.two_tower_score ?? null,
    twoTowerEmbedder: null,
    twoTowerVersion: null,
    koResult: toKoResult(raw),
    interactionId: raw.interaction_id,
  };
}

/**
 * プロジェクトの候補一覧取得
 */
export function useCandidates(projectId: number) {
  return useQuery({
    queryKey: ["candidates", projectId],
    queryFn: () =>
      get<RawMatchResponse[]>(`/api/projects/${projectId}/candidates`).then(
        (raw) => {
          const candidates = raw.map(mapCandidate);
          const createdAt = raw[0]?.matched_at ?? new Date().toISOString();
          return {
            projectId,
            projectName: `Project ${projectId}`,
            candidates,
            matchRunId: "",
            createdAt,
          } satisfies MatchCandidatesResponse;
        }
      ),
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
