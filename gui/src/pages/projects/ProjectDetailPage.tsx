import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useProjectDetail, useProjectFeedback, useProjectMatches, trackViewedDetail } from "@/api";
import type { FeedbackType, ProjectMatchStatus } from "@/api";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useI18n } from "@/lib/i18n";
import { Breadcrumbs } from "@/components/Breadcrumbs";
import type { TranslationKey } from "@/lib/messages";
import { TalentMatchCard } from "@/components/projects/TalentMatchCard";
import { componentTheme, type InteractionState } from "@/theme/component-theme";

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

const MATCH_STATUS_META: Record<ProjectMatchStatus, { labelKey: TranslationKey; state: InteractionState }> = {
  pending: { labelKey: "projectDetail.status.pending", state: "pending" },
  proposed: { labelKey: "projectDetail.status.proposed", state: "proposed" },
  rejected: { labelKey: "projectDetail.status.rejected", state: "rejected" },
  interview_scheduled: { labelKey: "projectDetail.status.interviewing", state: "interviewing" },
  accepted: { labelKey: "projectDetail.status.accepted", state: "accepted" },
  no_response: { labelKey: "projectDetail.status.no_response", state: "no_response" },
};

function SummaryField({ label, value }: { label: string; value: string | number | null | undefined }) {
  return (
    <div className="space-y-1">
      <div className="text-sm text-muted-foreground">{label}</div>
      <div className="text-base font-medium text-foreground">{value ?? "-"}</div>
    </div>
  );
}

export function ProjectDetailPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const { t } = useI18n();
  const [pendingInteractionId, setPendingInteractionId] = useState<number | null>(null);
  const { data, isLoading, error } = useProjectDetail(projectId);
  const { data: matches = [], isLoading: matchesLoading } = useProjectMatches(projectId);
  const feedbackMutation = useProjectFeedback(Number(projectId ?? 0));

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
  const handleInterview = (interactionId: number) => handleFeedback(interactionId, "interview_scheduled");

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
            <CardContent className={componentTheme.spacing.cardStack}>
              {!hasMatches ? (
                <div className="py-10 text-center text-muted-foreground">
                  {t("projectDetail.matches.empty")}
                </div>
              ) : (
                matches.map((match) => {
                  const resolvedStatus = MATCH_STATUS_META[match.status as ProjectMatchStatus] ?? MATCH_STATUS_META.pending;

                  return (
                    <TalentMatchCard
                      key={match.interactionId}
                      match={match}
                      onPropose={handlePropose}
                      onReject={handleReject}
                      onInterview={handleInterview}
                      onViewDetail={handleViewDetail}
                      isBusy={feedbackMutation.isPending && pendingInteractionId === match.interactionId}
                      isDisabled={feedbackMutation.isPending}
                      labels={{
                        propose: t("projectDetail.cta.propose"),
                        reject: t("projectDetail.cta.reject"),
                        interview: t("projectDetail.cta.interview"),
                        details: t("projectDetail.cta.details"),
                        scoreLabel: "Score",
                        rateLabel: t("projectDetail.summary.rate"),
                      }}
                      statusLabel={t(resolvedStatus.labelKey)}
                      statusState={resolvedStatus.state}
                    />
                  );
                })
              )}
            </CardContent>
          </Card>
        </section>
      </div>
    </div>
  );
}
