import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { TalentMatchCard } from "@/components/projects/TalentMatchCard";
import { useProjectDetail, useProjectFeedback, useProjectMatches, trackViewedDetail } from "@/api";
import type { FeedbackType } from "@/api";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useI18n } from "@/lib/i18n";
import { Breadcrumbs } from "@/components/Breadcrumbs";

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

  const handleFeedback = (interactionId: number, feedbackType: FeedbackType) => {
    setPendingInteractionId(interactionId);
    feedbackMutation.mutate(
      { interactionId, feedbackType, source: "gui" },
      {
        onSuccess: () => toast.success(t("projectDetail.feedback.success")),
        onError: (err: unknown) => {
          const message = err instanceof Error ? err.message : String(err);
          toast.error(t("projectDetail.feedback.error", { message }));
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
        </section>
      </div>
    </div>
  );
}
