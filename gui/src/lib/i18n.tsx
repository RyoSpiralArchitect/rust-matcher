/* eslint-disable react-refresh/only-export-components */
import { createContext, useContext, useMemo, useState, type ReactNode } from "react";

const MESSAGES = {
  en: {
    "loading.default": "Loading...",
    "error.prefix": "Error",
    "error.unknown": "Unknown error",

    "status.pending": "Pending",
    "status.processing": "Processing",
    "status.completed": "Completed",
    "status.review": "Review",
    "status.reviewRequired": "Review Required",
    "status.ok": "OK",
    "status.ko": "KO",

    "queue.dashboard.title": "Queue Dashboard",
    "queue.dashboard.manualReview": "Manual Review",
    "queue.dashboard.errors": "Errors",
    "queue.dashboard.staleProcessing": "Stale Processing",
    "queue.dashboard.viewAllJobs": "View All Jobs →",
    "queue.dashboard.viewPendingJobs": "View Pending Jobs →",

    "queue.jobs.title": "Queue Jobs",
    "queue.jobs.filter.statusLabel": "Status:",
    "queue.jobs.filter.statusAria": "Filter queue by status",
    "queue.jobs.filter.all": "All",
    "queue.jobs.filter.reviewOnly": "Manual Review Only",
    "queue.jobs.filter.clear": "Clear Filters",
    "queue.jobs.empty.title": "No queue jobs found",
    "queue.jobs.empty.description": "Try adjusting the filters to see jobs that match your criteria.",
    "queue.jobs.table.id": "ID",
    "queue.jobs.table.messageId": "Message ID",
    "queue.jobs.table.status": "Status",
    "queue.jobs.table.priority": "Priority",
    "queue.jobs.table.retry": "Retry",
    "queue.jobs.table.review": "Review",
    "queue.jobs.table.updated": "Updated",
    "queue.jobs.row.link": "View queue job {id}",
    "queue.jobs.loadingMore": "Loading more jobs...",

    "jobDetail.breadcrumb.jobs": "Jobs",
    "jobDetail.title": "Job #{id}",
    "jobDetail.reviewRequired": "Review Required",
    "jobDetail.retrying": "Retrying...",
    "jobDetail.retry": "Retry",
    "jobDetail.summary": "Summary",
    "jobDetail.priority": "Priority",
    "jobDetail.retryCount": "Retry Count",
    "jobDetail.finalMethod": "Final Method",
    "jobDetail.llmLatency": "LLM Latency",
    "jobDetail.decision": "Decision",
    "jobDetail.updated": "Updated",
    "jobDetail.lastError": "Last Error",
    "jobDetail.extractedFields": "Extracted Fields",
    "jobDetail.entity": "Entity",
    "jobDetail.matchPairs": "Match Pairs ({count})",
    "jobDetail.timeline": "Timeline",
    "jobDetail.timeline.more": "... and {count} more events",
    "jobDetail.timeline.byActor": "by {actor}",
    "jobDetail.retry.success": "Job retry initiated",
    "jobDetail.retry.failed": "Failed to retry job: {message}",
    "jobDetail.feedback.submitted": "Feedback \"{label}\" submitted",
    "jobDetail.feedback.failed": "Failed to submit feedback: {message}",
    "jobDetail.conversion.success": "Stage updated to \"{label}\"",
    "jobDetail.conversion.failed": "Failed to update stage: {message}",

    "jobDetail.field.talent": "Talent",
    "jobDetail.field.project": "Project",
    "jobDetail.field.id": "ID",
    "jobDetail.field.name": "Name",
    "jobDetail.field.desiredPriceMin": "Desired Price Min",
    "jobDetail.field.desiredPrice": "Desired Price",
    "jobDetail.field.availableDate": "Available Date",
    "jobDetail.field.summary": "Summary",
    "jobDetail.field.code": "Code",
    "jobDetail.field.priceRange": "Price Range",
    "jobDetail.field.startDate": "Start Date",

    "jobDetail.table.talentId": "Talent ID",
    "jobDetail.table.projectId": "Project ID",
    "jobDetail.table.score": "Score",
    "jobDetail.table.ko": "KO",
    "jobDetail.table.feedback": "Feedback",
    "jobDetail.table.stage": "Stage",
    "jobDetail.table.actions": "Actions",
    "jobDetail.table.reviewOk": "Review OK",
    "jobDetail.table.reviewNg": "Review NG",
    "jobDetail.table.thumbsUp": "Thumbs Up",
    "jobDetail.table.thumbsDown": "Thumbs Down",
    "jobDetail.table.reviewLabel": "Review",
    "jobDetail.table.okLabel": "OK",

    "conversionStage.contacted": "Contacted",
    "conversionStage.entry": "Entry",
    "conversionStage.interview_scheduled": "Interview",
    "conversionStage.offer": "Offer",
    "conversionStage.contract_signed": "Contract",
    "conversionStage.lost": "Lost",

    "feedback.thumbs_up": "Thumbs Up",
    "feedback.thumbs_down": "Thumbs Down",
    "feedback.review_ok": "Review OK",
    "feedback.review_ng": "Review NG",
    "feedback.review_pending": "Review Pending",
    "feedback.accepted": "Accepted",
    "feedback.rejected": "Rejected",
    "feedback.interview_scheduled": "Interview Scheduled",
    "feedback.no_response": "No Response",

    "event.viewed_candidate_detail": "Viewed candidate detail",
    "event.copied_template": "Copied template",
    "event.clicked_contact": "Clicked contact",
    "event.shortlisted": "Shortlisted",

    "candidates.loading": "Loading candidates...",
    "candidates.titleFallback": "Project",
    "candidates.count": "{count} candidates found",
    "candidates.none": "No candidates found for this project.",
    "candidates.score": "Score",
    "candidates.twoTowerScore": "TT",
    "candidates.priceLabel": "Desired Rate",
    "candidates.locationAvailability": "{location} / {availability}",
    "candidates.reviewRequired": "Needs review: {reason}",
    "candidates.buttons.shortlist": "Shortlist",
    "candidates.buttons.copyTemplate": "Copy Template",
    "candidates.buttons.contact": "Contact",
    "candidates.buttons.good": "Good",
    "candidates.buttons.ng": "NG",
    "candidates.buttons.submitting": "Submitting...",
    "candidates.feedback.success": "Feedback sent",
    "candidates.feedback.error": "Failed to send feedback: {message}",
    "candidates.talentFallback": "Talent #{id}",
  },
  ja: {
    "loading.default": "読み込み中...",
    "error.prefix": "エラー",
    "error.unknown": "不明なエラー",

    "status.pending": "待機中",
    "status.processing": "処理中",
    "status.completed": "完了",
    "status.review": "レビュー",
    "status.reviewRequired": "要レビュー",
    "status.ok": "OK",
    "status.ko": "KO",

    "queue.dashboard.title": "キューダッシュボード",
    "queue.dashboard.manualReview": "要レビュー",
    "queue.dashboard.errors": "エラー",
    "queue.dashboard.staleProcessing": "処理中（滞留）",
    "queue.dashboard.viewAllJobs": "すべてのジョブを見る →",
    "queue.dashboard.viewPendingJobs": "待機中のジョブを見る →",

    "queue.jobs.title": "キュージョブ",
    "queue.jobs.filter.statusLabel": "ステータス:",
    "queue.jobs.filter.statusAria": "ステータスでキューをフィルタ",
    "queue.jobs.filter.all": "すべて",
    "queue.jobs.filter.reviewOnly": "要レビューのみ",
    "queue.jobs.filter.clear": "フィルタをクリア",
    "queue.jobs.empty.title": "キュージョブが見つかりません",
    "queue.jobs.empty.description": "フィルタを調整して条件に合うジョブを表示してください。",
    "queue.jobs.table.id": "ID",
    "queue.jobs.table.messageId": "メッセージID",
    "queue.jobs.table.status": "ステータス",
    "queue.jobs.table.priority": "優先度",
    "queue.jobs.table.retry": "リトライ",
    "queue.jobs.table.review": "レビュー",
    "queue.jobs.table.updated": "更新日時",
    "queue.jobs.row.link": "キュージョブ {id} を表示",
    "queue.jobs.loadingMore": "さらに読み込み中...",

    "jobDetail.breadcrumb.jobs": "ジョブ一覧",
    "jobDetail.title": "ジョブ #{id}",
    "jobDetail.reviewRequired": "要レビュー",
    "jobDetail.retrying": "リトライ中...",
    "jobDetail.retry": "リトライ",
    "jobDetail.summary": "サマリー",
    "jobDetail.priority": "優先度",
    "jobDetail.retryCount": "リトライ回数",
    "jobDetail.finalMethod": "最終メソッド",
    "jobDetail.llmLatency": "LLM レイテンシ",
    "jobDetail.decision": "判定理由",
    "jobDetail.updated": "更新日時",
    "jobDetail.lastError": "最新エラー",
    "jobDetail.extractedFields": "抽出フィールド",
    "jobDetail.entity": "エンティティ",
    "jobDetail.matchPairs": "マッチ結果 ({count})",
    "jobDetail.timeline": "タイムライン",
    "jobDetail.timeline.more": "他 {count} 件のイベント",
    "jobDetail.timeline.byActor": "{actor} により",
    "jobDetail.retry.success": "ジョブのリトライを開始しました",
    "jobDetail.retry.failed": "ジョブのリトライに失敗: {message}",
    "jobDetail.feedback.submitted": "フィードバック「{label}」を送信しました",
    "jobDetail.feedback.failed": "フィードバック送信に失敗: {message}",
    "jobDetail.conversion.success": "ステージを「{label}」に更新しました",
    "jobDetail.conversion.failed": "ステージ更新に失敗: {message}",

    "jobDetail.field.talent": "タレント",
    "jobDetail.field.project": "プロジェクト",
    "jobDetail.field.id": "ID",
    "jobDetail.field.name": "名称",
    "jobDetail.field.desiredPriceMin": "希望単価（下限）",
    "jobDetail.field.desiredPrice": "希望単価",
    "jobDetail.field.availableDate": "稼働開始日",
    "jobDetail.field.summary": "サマリー",
    "jobDetail.field.code": "コード",
    "jobDetail.field.priceRange": "単価レンジ",
    "jobDetail.field.startDate": "開始日",

    "jobDetail.table.talentId": "タレントID",
    "jobDetail.table.projectId": "プロジェクトID",
    "jobDetail.table.score": "スコア",
    "jobDetail.table.ko": "KO",
    "jobDetail.table.feedback": "フィードバック",
    "jobDetail.table.stage": "ステージ",
    "jobDetail.table.actions": "アクション",
    "jobDetail.table.reviewOk": "レビューOK",
    "jobDetail.table.reviewNg": "レビューNG",
    "jobDetail.table.thumbsUp": "👍",
    "jobDetail.table.thumbsDown": "👎",
    "jobDetail.table.reviewLabel": "レビュー",
    "jobDetail.table.okLabel": "OK",

    "conversionStage.contacted": "コンタクト済み",
    "conversionStage.entry": "エントリー",
    "conversionStage.interview_scheduled": "面談予定",
    "conversionStage.offer": "オファー",
    "conversionStage.contract_signed": "契約締結",
    "conversionStage.lost": "失注",

    "feedback.thumbs_up": "いいね",
    "feedback.thumbs_down": "よくない",
    "feedback.review_ok": "レビューOK",
    "feedback.review_ng": "レビューNG",
    "feedback.review_pending": "レビュー保留",
    "feedback.accepted": "受注",
    "feedback.rejected": "失注",
    "feedback.interview_scheduled": "面談予定",
    "feedback.no_response": "未回答",

    "event.viewed_candidate_detail": "候補詳細を閲覧",
    "event.copied_template": "テンプレートをコピー",
    "event.clicked_contact": "連絡先をクリック",
    "event.shortlisted": "ショートリストに追加",

    "candidates.loading": "候補者を読み込み中...",
    "candidates.titleFallback": "プロジェクト",
    "candidates.count": "{count} 件の候補が見つかりました",
    "candidates.none": "このプロジェクトの候補者が見つかりません。",
    "candidates.score": "スコア",
    "candidates.twoTowerScore": "TT",
    "candidates.priceLabel": "希望単価",
    "candidates.locationAvailability": "{location} / {availability}",
    "candidates.reviewRequired": "要レビュー: {reason}",
    "candidates.buttons.shortlist": "ショートリスト",
    "candidates.buttons.copyTemplate": "テンプレートをコピー",
    "candidates.buttons.contact": "連絡する",
    "candidates.buttons.good": "良い",
    "candidates.buttons.ng": "NG",
    "candidates.buttons.submitting": "送信中...",
    "candidates.feedback.success": "フィードバックを送信しました",
    "candidates.feedback.error": "フィードバック送信に失敗: {message}",
    "candidates.talentFallback": "タレント #{id}",
  },
} as const;

export type Locale = keyof typeof MESSAGES;
export type TranslationKey = keyof typeof MESSAGES.en;

interface I18nContextValue {
  locale: Locale;
  setLocale: (locale: Locale) => void;
  t: (key: TranslationKey, values?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function normalizeLocale(input?: string | null): Locale {
  if (!input) return "en";
  const normalized = input.toLowerCase();
  if (normalized.startsWith("ja")) return "ja";
  return "en";
}

function formatMessage(template: string, values?: Record<string, string | number>) {
  if (!values) return template;
  return Object.entries(values).reduce(
    (result, [key, value]) => result.replaceAll(`{${key}}`, String(value)),
    template,
  );
}

export function I18nProvider({ children, locale }: { children: ReactNode; locale?: string }) {
  const initialLocale =
    typeof navigator !== "undefined" && !locale ? normalizeLocale(navigator.language) : normalizeLocale(locale);
  const [currentLocale, setCurrentLocale] = useState<Locale>(initialLocale);

  const value = useMemo<I18nContextValue>(() => {
    const t = (key: TranslationKey, values?: Record<string, string | number>) => {
      const message =
        MESSAGES[currentLocale][key] ??
        MESSAGES.en[key] ??
        key;
      return formatMessage(message, values);
    };

    return {
      locale: currentLocale,
      setLocale: setCurrentLocale,
      t,
    };
  }, [currentLocale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const ctx = useContext(I18nContext);
  if (!ctx) {
    throw new Error("useI18n must be used within I18nProvider");
  }
  return ctx;
}
