/**
 * API DTO Types - sr-api の HTTP 契約
 *
 * これらの型は sr-api のレスポンス構造と一致する必要がある
 * Rust 側のソースを直接参照せず、HTTP 契約として管理
 */

// ============================================================
// Talents
// ============================================================

export interface TalentListItem {
  id: number;
  name: string;
  role: string | null;
  location: string | null;
  availabilityDate: string | null;
  skills: string[];
  score: number | null;
  experienceHighlights: string[];
}

export interface TalentSearchResponse {
  items: TalentListItem[];
  limit: number;
  offset: number;
  total: number;
  hasMore: boolean;
}

// ============================================================
// Talent Detail
// ============================================================

export interface TalentProfile {
  id: number;
  name: string | null;
  title?: string | null;
  skills?: string[];
  availability?: string | null;
  availableFrom?: string | null;
  location?: string | null;
  desiredPriceMin?: number | null;
}

export type TalentMatchStatus =
  | "pending"
  | "proposed"
  | "accepted"
  | "in_project"
  | "rejected";

export interface TalentMatchProject {
  projectId: number;
  projectName: string;
  score: number | null;
  businessScore?: number | null;
  status: TalentMatchStatus;
  canPropose?: boolean;
  canReject?: boolean;
  lastStatusChange?: string | null;
}

export interface TalentDetailResponse {
  talent: TalentProfile;
  matches: TalentMatchProject[];
}

export interface TalentMatchDecisionRequest {
  talentId: number;
  projectId: number;
  decision: "propose" | "reject";
}

export interface TalentMatchDecisionResponse {
  status: TalentMatchStatus;
}

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
  interactionId: number;
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
// Projects List
// ============================================================

export interface ProjectListItem {
  id?: number | null;
  projectId?: number | null;
  projectName: string | null;
  monthlyTankaMin: number | null;
  monthlyTankaMax: number | null;
  matchedCount?: number | null;
  proposedCount?: number | null;
  interviewingCount?: number | null;
}

export type ProjectsListResponse =
  | ProjectListItem[]
  | {
      items: ProjectListItem[];
      limit?: number;
      offset?: number;
      total?: number;
      hasMore?: boolean;
    };

// ============================================================
// Project Detail
// ============================================================

export interface ProjectMatch {
  interactionId: number;
  talentId: number;
  talentName: string | null;
  score: number;
  headline: string | null;
  keySkills: string[];
  desiredRateMin: number | null;
  desiredRateMax: number | null;
}

export interface ProjectDetailResponse {
  id: number;
  name: string;
  summary?: string | null;
  rateMin: number | null;
  rateMax: number | null;
  workStyle: string | null;
  skills: string[];
  matches: ProjectMatch[];
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
  | "rejected"
  | "review_pending"
  | "interview_scheduled"
  | "no_response";

export type FeedbackSource = "gui" | "crm" | "api" | "import";

export interface FeedbackRequest {
  interactionId: number;
  feedbackType: FeedbackType;
  ngReasonCategory?: string;
  comment?: string;
  source: FeedbackSource;
}

export type FeedbackStatus = "created" | "already_exists";

export interface FeedbackResponse {
  id?: number;
  status: FeedbackStatus;
  feedbackType: FeedbackType;
  interactionId: number;
  matchResultId?: number | null;
  matchRunId?: string | null;
  projectId: number;
  talentId: number;
}

// ============================================================
// Interaction Events (Behavioral Logs)
// ============================================================

export type InteractionEventType =
  | "viewed_candidate_detail"
  | "copied_template"
  | "clicked_contact"
  | "shortlisted";

export type InteractionEventSource = "gui" | "crm" | "api" | "import";

export interface InteractionEventRequest {
  interactionId: number;
  eventType: InteractionEventType;
  idempotencyKey: string;
  meta?: Record<string, unknown>;
  source?: InteractionEventSource;
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

export const QUEUE_JOB_STATUSES = ["pending", "processing", "completed"] as const;
export type QueueJobStatus = (typeof QUEUE_JOB_STATUSES)[number];
export type QueueStatusCounts = Record<QueueJobStatus, number>;

export interface QueueDashboard {
  statusCounts: QueueStatusCounts;
  manualReviewCount: number;
  errorCount: number;
  staleProcessingCount: number;
  updatedAt: string;
}

export interface QueueJobListItem {
  id: number;
  messageId: string;
  status: QueueJobStatus;
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
  total: number;
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
