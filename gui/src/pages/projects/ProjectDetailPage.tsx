import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { ArrowLeft, Loader2, MessageCircle, ThumbsDown, ThumbsUp } from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useProjectDetail, useProjectFeedback, trackViewedDetail } from "@/api";
import type { FeedbackType, ProjectMatch } from "@/api";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useI18n } from "@/lib/i18n";

function SummaryField({ label, value }: { label: string; value: string | number | null | undefined }) {
  return (
    <div>
      <div className="text-sm text-muted-foreground">{label}</div>
      <div className="text-base font-medium">{value}</div>
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
  labels: { propose: string; reject: string; details: string };
}) {
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
  const feedbackMutation = useProjectFeedback(Number(projectId ?? 0));

  const matches = data?.matches ?? [];

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

  const handleViewDetail = (interactionId: number) => {
    trackViewedDetail(interactionId);
    toast(t("projectDetail.cta.detailsToast"));
  };

  if (isLoading) {
    return <LoadingState />;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <nav className="flex items-center gap-2 text-sm text-muted-foreground">
            <Link to="/projects" className="hover:text-foreground">
              {t("projectDetail.breadcrumb.projects")}
            </Link>
            <span>/</span>
            <span className="text-foreground">
              {data?.name ?? t("projectDetail.title.fallback", { id: projectId ?? "" })}
            </span>
          </nav>
          <Button variant="ghost" size="sm" asChild>
            <Link to="/projects">
              <ArrowLeft className="mr-2 h-4 w-4" />
              {t("projectDetail.backToProjects")}
            </Link>
          </Button>
        </div>
        <div className="space-y-1">
          <h1 className="text-2xl font-bold">{data?.name ?? t("projectDetail.title.fallback", { id: projectId ?? "" })}</h1>
          {data?.summary ? <p className="text-muted-foreground">{data.summary}</p> : null}
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t("projectDetail.summary.title")}</CardTitle>
          <CardDescription>{t("projectDetail.summary.description")}</CardDescription>
        </CardHeader>
        <CardContent className="grid gap-4 md:grid-cols-3">
          <SummaryField label={t("projectDetail.summary.rate")} value={rateLabel} />
          <SummaryField label={t("projectDetail.summary.workStyle")} value={data?.workStyle ?? t("projectDetail.summary.workStyleEmpty")} />
          <SummaryField label={t("projectDetail.summary.skills")} value={data?.skills?.join(", ") ?? t("projectDetail.summary.skillsEmpty")} />
        </CardContent>
      </Card>

      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-xl font-semibold">{t("projectDetail.matches.title")}</h2>
            <p className="text-sm text-muted-foreground">{t("projectDetail.matches.count", { count: matches.length })}</p>
          </div>
        </div>

        {!hasMatches && (
          <Card>
            <CardContent className="py-10 text-center text-muted-foreground">
              {t("projectDetail.matches.empty")}
            </CardContent>
          </Card>
        )}

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
                details: t("projectDetail.cta.details"),
              }}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
