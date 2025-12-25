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

export interface ConversionRequest {
  interactionId?: number;
  talentId: number;
  projectId: number;
  stage: ConversionStage;
  source?: "gui" | "crm" | "import";
  meta?: Record<string, unknown>;
}

export interface ConversionResponse {
  id: number;
  status: string;
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

// ============================================================
// Queue Job Detail
// ============================================================

export interface QueueJobDetailResponse {
  job: QueueJobListItem;
  partialFields: Record<string, unknown> | null;
  lastError: string | null;
  llmLatencyMs: number | null;
  processingStartedAt: string | null;
  completedAt: string | null;
  entity: JobEntity | null;
  pairs: PairDetail[] | null;
  sourcePreview: string | null;
}

export type JobEntity =
  | { type: "talent"; id: number; messageId: string; talentName: string | null; summaryText: string | null; desiredPriceMin: number | null; availableDate: string | null; receivedAt: string }
  | { type: "project"; projectCode: number; messageId: string | null; projectName: string; monthlyTankaMin: number | null; monthlyTankaMax: number | null; startDate: string | null; requiresManualReview: boolean; manualReviewReason: string | null }
  | { type: "both"; talent: TalentSnapshot; project: ProjectSnapshot };

export interface TalentSnapshot {
  id: number;
  messageId: string;
  talentName: string | null;
  summaryText: string | null;
  desiredPriceMin: number | null;
  availableDate: string | null;
  receivedAt: string;
}

export interface ProjectSnapshot {
  projectCode: number;
  messageId: string | null;
  projectName: string;
  monthlyTankaMin: number | null;
  monthlyTankaMax: number | null;
  startDate: string | null;
  requiresManualReview: boolean;
  manualReviewReason: string | null;
}

export interface PairDetail {
  matchResult: MatchResultRow;
  latestInteraction: InteractionLogRow | null;
  feedbackEvents: FeedbackEventRow[];
  interactionEvents: InteractionEventRow[];
}

export interface MatchResultRow {
  id: number;
  talentId: number;
  projectId: number;
  isKnockout: boolean;
  koReasons: string[];
  needsManualReview: boolean;
  scoreTotal: number | null;
  scoreBreakdown: Record<string, unknown> | null;
  engineVersion: string | null;
  ruleVersion: string | null;
  createdAt: string;
}

export interface InteractionLogRow {
  id: number;
  matchResultId: number | null;
  talentId: number;
  projectId: number;
  matchRunId: string | null;
  engineVersion: string | null;
  configVersion: string | null;
  twoTowerScore: number | null;
  twoTowerEmbedder: string | null;
  twoTowerVersion: string | null;
  businessScore: number | null;
  outcome: string | null;
  feedbackAt: string | null;
  variant: string | null;
  createdAt: string;
}

export interface FeedbackEventRow {
  id: number;
  interactionId: number | null;
  matchResultId: number | null;
  matchRunId: string | null;
  engineVersion: string | null;
  configVersion: string | null;
  projectId: number;
  talentId: number;
  feedbackType: string;
  ngReasonCategory: string | null;
  comment: string | null;
  actor: string;
  source: string;
  isRevoked: boolean;
  createdAt: string;
}

export interface InteractionEventRow {
  id: number;
  interactionId: number;
  eventType: string;
  actor: string;
  source: string;
  meta: Record<string, unknown> | null;
  createdAt: string;
}
