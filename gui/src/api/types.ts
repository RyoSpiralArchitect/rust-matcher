/**
 * API DTO Types - sr-api の HTTP 契約
 *
 * これらの型は sr-api のレスポンス構造と一致する必要がある
 * Rust 側のソースを直接参照せず、HTTP 契約として管理
 */

// ============================================================
// Matching Candidates
// ============================================================

export interface MatchCandidate {
  talentId: number;
  talentName: string | null;
  skills: string[];
  desiredPriceMin: number | null;
  residentialTodofuken: string | null;
  availabilityDate: string | null;
  totalScore: number;
  businessScore: number;
  twoTowerScore: number | null;
  twoTowerEmbedder: string | null;
  twoTowerVersion: string | null;
  koResult: KoResult;
}

export interface KoResult {
  isHardKo: boolean;
  isSoftKo: boolean;
  needsManualReview: boolean;
  reasons: string[];
}

export interface MatchCandidatesResponse {
  projectId: number;
  projectName: string;
  candidates: MatchCandidate[];
  matchRunId: string;
  createdAt: string;
}

// ============================================================
// Feedback
// ============================================================

export type FeedbackType =
  | "thumbs_up"
  | "thumbs_down"
  | "review_ok"
  | "review_ng"
  | "accepted"
  | "rejected";

export interface FeedbackRequest {
  interactionId: number;
  feedbackType: FeedbackType;
  ngReasonCategory?: string;
  comment?: string;
}

export interface FeedbackResponse {
  id: number;
  createdAt: string;
}

// ============================================================
// Interaction Events (Behavioral Logs)
// ============================================================

export type InteractionEventType =
  | "viewed_candidate_detail"
  | "copied_template"
  | "clicked_contact"
  | "shortlisted";

export interface InteractionEventRequest {
  interactionId: number;
  eventType: InteractionEventType;
  idempotencyKey: string;
  meta?: Record<string, unknown>;
}

// ============================================================
// Conversion Events
// ============================================================

export type ConversionStage =
  | "contacted"
  | "entry"
  | "interview_scheduled"
  | "offer"
  | "contract_signed"
  | "lost";

export interface ConversionEventRequest {
  interactionId?: number;
  talentId: number;
  projectId: number;
  stage: ConversionStage;
  meta?: Record<string, unknown>;
}

// ============================================================
// Queue Dashboard (既存)
// ============================================================

export interface QueueDashboard {
  statusCounts: {
    pending: number;
    processing: number;
    completed: number;
  };
  manualReviewCount: number;
  errorCount: number;
  staleProcessingCount: number;
  updatedAt: string;
}

export interface QueueJobListItem {
  id: number;
  messageId: string;
  status: string;
  priority: number;
  retryCount: number;
  nextRetryAt: string | null;
  finalMethod: string | null;
  requiresManualReview: boolean;
  manualReviewReason: string | null;
  decisionReason: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface QueueJobListResponse {
  items: QueueJobListItem[];
  limit: number;
  offset: number;
  hasMore: boolean;
}
