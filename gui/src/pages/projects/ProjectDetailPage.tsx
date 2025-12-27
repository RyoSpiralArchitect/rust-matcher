import { useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft, CalendarClock, Loader2, MessageCircle, ThumbsDown, ThumbsUp } from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useProjectDetail, useProjectFeedback, trackViewedDetail } from "@/api";
import type { FeedbackType, ProjectMatch, ProjectMatchStatus } from "@/api";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useI18n } from "@/lib/i18n";
import { Breadcrumbs } from "@/components/Breadcrumbs";
import type { TranslationKey } from "@/lib/messages";

const FEEDBACK_LABEL_KEYS: Record<FeedbackType, TranslationKey> = {
  thumbs_up: "feedback.thumbs_up",
  thumbs_down: "feedback.thumbs_down",
  review_ok: "feedback.review_ok",
  review_ng: "feedback.review_ng",
  accepted: "feedback.accepted",
  rejected: "feedback.rejected",
  review_pending: "feedback.review_pending",
  interview_scheduled: "feedback.interview_scheduled",
  no_response: "feedback.no_response",
};

const STATUS_META: Record<
  ProjectMatchStatus,
  { labelKey: TranslationKey; variant?: "default" | "secondary" | "outline" | "destructive"; className?: string }
> = {
  pending: { labelKey: "projectDetail.status.pending", variant: "outline" },
  proposed: { labelKey: "projectDetail.status.proposed", className: "bg-blue-600 text-white border-blue-600" },
  rejected: {
    labelKey: "projectDetail.status.rejected",
    variant: "secondary",
    className: "bg-muted text-muted-foreground border-transparent",
  },
  interview_scheduled: {
    labelKey: "projectDetail.status.interviewing",
    className: "bg-orange-500 text-white border-orange-500",
  },
  accepted: { labelKey: "projectDetail.status.accepted", className: "bg-emerald-500 text-white border-emerald-500" },
  no_response: {
    labelKey: "projectDetail.status.no_response",
    variant: "secondary",
    className: "bg-slate-200 text-slate-800 border-transparent dark:bg-slate-800 dark:text-slate-50",
  },
};

function SummaryField({ label, value }: { label: string; value: string | number | null | undefined }) {
  return (
    <div className="space-y-1">
      <div className="text-sm text-muted-foreground">{label}</div>
      <div className="text-base font-medium text-foreground">{value ?? "-"}</div>
    </div>
  );
}

function TalentCard({
  match,
  onFeedback,
  onViewDetail,
  isBusy,
  isDisabled,
  labels,
}: {
  match: ProjectMatch;
  onFeedback: (interactionId: number, feedbackType: FeedbackType) => void;
  onViewDetail: (interactionId: number) => void;
  isBusy: boolean;
  isDisabled: boolean;
  labels: { propose: string; reject: string; interview: string; details: string };
}) {
  const { t } = useI18n();
  const resolvedStatus: ProjectMatchStatus = (match.status as ProjectMatchStatus) ?? "pending";
  const statusMeta = STATUS_META[resolvedStatus] ?? STATUS_META.pending;
  const statusLabel = t(statusMeta.labelKey);
  const skills = match.keySkills ?? [];
  const rateRange =
    match.desiredRateMin !== null && match.desiredRateMin !== undefined
      ? `¥${match.desiredRateMin.toLocaleString()}${match.desiredRateMax ? ` - ¥${match.desiredRateMax.toLocaleString()}` : ""}`
      : null;

  return (
    <Card>
      <CardHeader className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
        <div className="space-y-2">
          <CardTitle>{match.talentName ?? `Talent #${match.talentId}`}</CardTitle>
          {match.headline ? <CardDescription>{match.headline}</CardDescription> : null}
          <div className="flex flex-wrap gap-2">
            <Badge variant={statusMeta.variant ?? "default"} className={statusMeta.className}>
              {statusLabel}
            </Badge>
            <Badge variant="secondary">Score: {match.score.toFixed(1)}</Badge>
            {skills.map((skill) => (
              <Badge key={skill} variant="outline">
                {skill}
              </Badge>
            ))}
          </div>
        </div>
        <div className="text-sm text-muted-foreground space-y-1 text-left md:text-right">
          {rateRange ? <div>Rate: {rateRange}</div> : null}
        </div>
      </CardHeader>
      <CardContent className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => onFeedback(match.interactionId, "thumbs_up")}
            disabled={isDisabled}
            aria-label={labels.propose}
          >
            {isBusy ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <ThumbsUp className="mr-2 h-4 w-4" />}
            {labels.propose} 👍
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onFeedback(match.interactionId, "thumbs_down")}
            disabled={isDisabled}
            aria-label={labels.reject}
          >
            {isBusy ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <ThumbsDown className="mr-2 h-4 w-4" />}
            {labels.reject} 👎
          </Button>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => onFeedback(match.interactionId, "interview_scheduled")}
            disabled={isDisabled}
            aria-label={labels.interview}
          >
            {isBusy ? (
              <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            ) : (
              <CalendarClock className="mr-2 h-4 w-4" />
            )}
            {labels.interview}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onViewDetail(match.interactionId)}
            disabled={isDisabled}
            aria-label={labels.details}
          >
            <MessageCircle className="mr-2 h-4 w-4" />
            {labels.details} 💬
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}

export function ProjectDetailPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const { t } = useI18n();
  const [pendingInteractionId, setPendingInteractionId] = useState<number | null>(null);
  const { data, isLoading, error } = useProjectDetail(projectId);
  const { data: matches = [], isLoading: matchesLoading } = useProjectMatches(projectId);
  const feedbackMutation = useProjectFeedback(Number(projectId ?? 0));

  const matches = useMemo(() => data?.matches ?? [], [data?.matches]);

  const hasMatches = matches.length > 0;

  const minRate = data?.rateMin;
  const maxRate = data?.rateMax;
  let rateLabel = "-";
  if (minRate !== null && minRate !== undefined && maxRate !== null && maxRate !== undefined) {
    rateLabel = `¥${minRate.toLocaleString()} - ¥${maxRate.toLocaleString()}`;
  } else if (minRate !== null && minRate !== undefined) {
    rateLabel = `¥${minRate.toLocaleString()}~`;
  } else if (maxRate !== null && maxRate !== undefined) {
    rateLabel = `~¥${maxRate.toLocaleString()}`;
  }

  const projectLabel = data?.name ?? t("projectDetail.title.fallback", { id: projectId ?? "" });

  const buildTalentLabel = (interactionId: number) => {
    const target = matches.find((match) => match.interactionId === interactionId);
    if (!target) return t("candidates.talentFallback", { id: interactionId });
    return target.talentName ?? t("candidates.talentFallback", { id: target.talentId });
  };

  const handleFeedback = (interactionId: number, feedbackType: FeedbackType) => {
    setPendingInteractionId(interactionId);
    const actionLabel = t(FEEDBACK_LABEL_KEYS[feedbackType]);
    const talentLabel = buildTalentLabel(interactionId);
    feedbackMutation.mutate(
      { interactionId, feedbackType, source: "gui" },
      {
        onSuccess: () =>
          toast.success(
            t("projectDetail.feedback.successWithContext", {
              action: actionLabel,
              project: projectLabel,
              talent: talentLabel,
            })
          ),
        onError: (err: unknown) => {
          const message = err instanceof Error ? err.message : String(err);
          toast.error(
            t("projectDetail.feedback.errorWithContext", {
              action: actionLabel,
              project: projectLabel,
              talent: talentLabel,
              message,
            })
          );
        },
        onSettled: () => setPendingInteractionId(null),
      }
    );
  };

  const handlePropose = (interactionId: number) => handleFeedback(interactionId, "thumbs_up");
  const handleReject = (interactionId: number) => handleFeedback(interactionId, "thumbs_down");

  const handleViewDetail = (interactionId: number) => {
    trackViewedDetail(interactionId);
    toast(t("projectDetail.cta.detailsToast"));
  };

  if (isLoading || matchesLoading) {
    return <LoadingState />;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  const projectTitle = data?.name ?? t("projectDetail.title.fallback", { id: projectId ?? "" });

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <Breadcrumbs
            items={[
              { label: t("navigation.projects"), href: "/projects" },
              {
                label: projectTitle,
                isCurrent: true,
              },
            ]}
          />
          <Button variant="ghost" size="sm" asChild>
            <Link to="/projects">
              <ArrowLeft className="mr-2 h-4 w-4" />
              {t("projectDetail.backToProjects")}
            </Link>
          </Button>
        </div>
        <div className="space-y-1">
          <h1 className="text-2xl font-bold">{projectTitle}</h1>
          {data?.summary ? <p className="text-muted-foreground">{data.summary}</p> : null}
        </div>
      </div>

      <div className="grid gap-6 xl:grid-cols-[1.05fr,1.45fr]">
        <section className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>{t("projectDetail.summary.title")}</CardTitle>
              <CardDescription>{t("projectDetail.summary.description")}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 sm:grid-cols-2">
                <SummaryField label={t("projectDetail.summary.rate")} value={rateLabel} />
                <SummaryField
                  label={t("projectDetail.summary.workStyle")}
                  value={data?.workStyle ?? t("projectDetail.summary.workStyleEmpty")}
                />
              </div>

              <div className="space-y-2">
                <div className="text-sm text-muted-foreground">{t("projectDetail.summary.skills")}</div>
                {data?.skills && data.skills.length > 0 ? (
                  <div className="flex flex-wrap gap-2">
                    {data.skills.map((skill) => (
                      <Badge key={skill} variant="secondary">
                        {skill}
                      </Badge>
                    ))}
                  </div>
                ) : (
                  <div className="text-sm text-muted-foreground">
                    {t("projectDetail.summary.skillsEmpty")}
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        </section>

        <section className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>{t("projectDetail.matches.title")}</CardTitle>
              <CardDescription>{t("projectDetail.matches.count", { count: matches.length })}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              {!hasMatches ? (
                <div className="py-10 text-center text-muted-foreground">
                  {t("projectDetail.matches.empty")}
                </div>
              ) : (
                matches.map((match) => (
                  <TalentMatchCard
                    key={match.interactionId}
                    match={match}
                    onPropose={handlePropose}
                    onReject={handleReject}
                    onViewDetail={handleViewDetail}
                    isBusy={feedbackMutation.isPending && pendingInteractionId === match.interactionId}
                    isDisabled={feedbackMutation.isPending}
                    labels={{
                      propose: t("projectDetail.cta.propose"),
                      reject: t("projectDetail.cta.reject"),
                      details: t("projectDetail.cta.details"),
                      scoreLabel: "Score",
                      rateLabel: t("projectDetail.summary.rate"),
                    }}
                  />
                ))
              )}
            </CardContent>
          </Card>

          <div className="space-y-4">
            {matches.map((match) => (
              <TalentCard
                key={match.interactionId}
                match={match}
                onFeedback={handleFeedback}
                onViewDetail={handleViewDetail}
                isBusy={feedbackMutation.isPending && pendingInteractionId === match.interactionId}
                isDisabled={feedbackMutation.isPending}
                labels={{
                  propose: t("projectDetail.cta.propose"),
                  reject: t("projectDetail.cta.reject"),
                  interview: t("projectDetail.cta.interview"),
                  details: t("projectDetail.cta.details"),
                }}
              />
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
