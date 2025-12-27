import { Link, useParams } from "react-router-dom";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Loader2, ExternalLink, Check, X } from "lucide-react";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useI18n } from "@/lib/i18n";
import { useMatchDecision, useTalentDetail, type TalentMatchProject, type TalentMatchStatus, type TalentProfile } from "@/api";
import type { TranslationKey } from "@/lib/messages";

export function TalentDetailPage() {
  const { id } = useParams<{ id: string }>();
  const talentId = Number(id);
  const { t } = useI18n();

  const { data, isLoading, error } = useTalentDetail(talentId);
  const decisionMutation = useMatchDecision();

  if (isLoading) {
    return <LoadingState message={t("talentDetail.loading")} />;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  if (!data) {
    return null;
  }

  const handleDecision = (projectId: number, decision: "propose" | "reject") => {
    if (!talentId) return;
    decisionMutation.mutate(
      { talentId, projectId, decision },
      {
        onSuccess: () => {
          const label =
            decision === "propose"
              ? t("talentDetail.actions.propose")
              : t("talentDetail.actions.reject");
          toast.success(t("talentDetail.decision.success", { decision: label }));
        },
        onError: (err: unknown) => {
          const message = err instanceof Error ? err.message : String(err);
          toast.error(t("talentDetail.decision.error", { message }));
        },
      }
    );
  };

  return (
    <div className="space-y-8">
      <ProfileHeader talent={data.talent} />
      <MatchedProjects
        matches={data.matches}
        onDecision={handleDecision}
        isSubmitting={decisionMutation.isPending}
      />
    </div>
  );
}

function ProfileHeader({ talent }: { talent: TalentProfile }) {
  const { t } = useI18n();
  const availability = talent.availability ?? t("talentDetail.availabilityUnknown");

  return (
    <Card>
      <CardHeader className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
        <div className="space-y-2">
          <div>
            <CardTitle className="text-2xl">
              {talent.name ?? t("talentDetail.titleFallback", { id: talent.id })}
            </CardTitle>
            {talent.title && (
              <CardDescription className="text-base">{talent.title}</CardDescription>
            )}
          </div>
          <div className="flex flex-wrap gap-2">
            {talent.skills?.length ? (
              talent.skills.map((skill) => (
                <Badge key={skill} variant="secondary">
                  {skill}
                </Badge>
              ))
            ) : (
              <span className="text-sm text-muted-foreground">
                {t("talentDetail.skillsEmpty")}
              </span>
            )}
          </div>
        </div>
        <div className="space-y-2 text-sm text-muted-foreground">
          <Badge variant="outline">
            {t("talentDetail.availability", { availability })}
          </Badge>
          {talent.availableFrom && (
            <div>{t("talentDetail.availableFrom", { date: talent.availableFrom })}</div>
          )}
          {talent.location && (
            <div>{t("talentDetail.location", { location: talent.location })}</div>
          )}
          {talent.desiredPriceMin !== null && talent.desiredPriceMin !== undefined && (
            <div>
              {t("talentDetail.desiredPrice", {
                price: `¥${talent.desiredPriceMin.toLocaleString()}`,
              })}
            </div>
          )}
        </div>
      </CardHeader>
    </Card>
  );
}

interface MatchedProjectsProps {
  matches: TalentMatchProject[];
  onDecision: (projectId: number, decision: "propose" | "reject") => void;
  isSubmitting: boolean;
}

function MatchedProjects({ matches, onDecision, isSubmitting }: MatchedProjectsProps) {
  const { t } = useI18n();

  return (
    <section className="space-y-4">
      <div>
        <h2 className="text-xl font-semibold">
          {t("talentDetail.matchesHeading")}
        </h2>
        <p className="text-sm text-muted-foreground">
          {t("talentDetail.matchesCount", { count: matches.length })}
        </p>
      </div>

      {matches.length === 0 ? (
        <Card>
          <CardContent className="py-10 text-center text-muted-foreground">
            {t("talentDetail.noMatches")}
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {matches.map((match) => (
            <ProjectMatchCard
              key={match.projectId}
              match={match}
              onDecision={onDecision}
              isSubmitting={isSubmitting}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function ProjectMatchCard({
  match,
  onDecision,
  isSubmitting,
}: {
  match: TalentMatchProject;
  onDecision: (projectId: number, decision: "propose" | "reject") => void;
  isSubmitting: boolean;
}) {
  const { t } = useI18n();
  const isActionable =
    match.canPropose || match.canReject || match.status === "pending";

  const statusVariant: Record<TalentMatchStatus, "default" | "secondary" | "outline" | "destructive"> = {
    pending: "outline",
    proposed: "default",
    accepted: "default",
    in_project: "secondary",
    rejected: "destructive",
  };

  const STATUS_LABEL_KEYS: Record<TalentMatchStatus, TranslationKey> = {
    pending: "talentDetail.status.pending",
    proposed: "talentDetail.status.proposed",
    accepted: "talentDetail.status.accepted",
    in_project: "talentDetail.status.in_project",
    rejected: "talentDetail.status.rejected",
  };

  const statusLabel = t(STATUS_LABEL_KEYS[match.status]);

  const handleDecision = (decision: "propose" | "reject") => {
    onDecision(match.projectId, decision);
  };

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-3">
        <div className="space-y-1">
          <CardTitle className="text-lg">
            <Link to={`/projects/${match.projectId}`} className="hover:underline">
              {match.projectName}
            </Link>
          </CardTitle>
          <CardDescription className="text-sm">
            {t("talentDetail.scoreLabel")}:{" "}
            {match.score !== null && match.score !== undefined
              ? match.score.toFixed(2)
              : "-"}
          </CardDescription>
        </div>
        <Badge variant={statusVariant[match.status] ?? "outline"}>
          {statusLabel}
        </Badge>
      </CardHeader>
      <CardContent className="space-y-4">
        {(match.businessScore !== undefined && match.businessScore !== null) ||
        match.lastStatusChange ? (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            {match.businessScore !== undefined && match.businessScore !== null && (
              <span>
                {t("talentDetail.businessScore")}: {match.businessScore.toFixed(2)}
              </span>
            )}
            {match.lastStatusChange && (
              <span className="text-xs">
                {t("talentDetail.lastUpdated", { date: match.lastStatusChange })}
              </span>
            )}
          </div>
        ) : null}

        <div className="flex flex-wrap items-center gap-2">
          {isActionable ? (
            <>
              {match.canPropose !== false && (
                <Button
                  size="sm"
                  onClick={() => handleDecision("propose")}
                  disabled={isSubmitting}
                >
                  {isSubmitting ? (
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  ) : (
                    <Check className="h-4 w-4 mr-2" />
                  )}
                  {t("talentDetail.actions.propose")}
                </Button>
              )}
              {match.canReject !== false && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => handleDecision("reject")}
                  disabled={isSubmitting}
                >
                  {isSubmitting ? (
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                  ) : (
                    <X className="h-4 w-4 mr-2" />
                  )}
                  {t("talentDetail.actions.reject")}
                </Button>
              )}
              <Button variant="link" size="sm" asChild>
                <Link to={`/projects/${match.projectId}`}>
                  <ExternalLink className="h-4 w-4 mr-1" />
                  {t("talentDetail.actions.viewProject")}
                </Link>
              </Button>
            </>
          ) : (
            <Button variant="link" size="sm" asChild>
              <Link to={`/projects/${match.projectId}`}>
                <ExternalLink className="h-4 w-4 mr-1" />
                {t("talentDetail.actions.viewProject")}
              </Link>
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
