import { useParams } from "react-router-dom";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Loader2, ThumbsUp, ThumbsDown, Star, Mail, Copy } from "lucide-react";
import { toast } from "sonner";
import {
  useCandidates,
  useSendFeedback,
  trackViewedDetail,
  trackClickedContact,
  trackCopiedTemplate,
  trackShortlisted,
} from "@/api";
import type { MatchCandidate, FeedbackType } from "@/api";
import { ErrorDisplay } from "@/components/ErrorDisplay";
import { LoadingState } from "@/components/LoadingState";
import { useI18n } from "@/lib/i18n";

export function CandidatesPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const id = Number(projectId);
  const { t } = useI18n();

  const { data, isLoading, error } = useCandidates(id);
  const feedbackMutation = useSendFeedback();

  if (isLoading) {
    return <LoadingState message={t("candidates.loading")} />;
  }

  if (error) {
    return <ErrorDisplay error={error} />;
  }

  const handleFeedback = (interactionId: number, feedbackType: FeedbackType) => {
    feedbackMutation.mutate(
      { interactionId, feedbackType, source: "gui" },
      {
        onSuccess: () => {
          toast.success(t("candidates.feedback.success"));
        },
        onError: (err: unknown) => {
          const message = err instanceof Error ? err.message : String(err);
          toast.error(t("candidates.feedback.error", { message }));
        },
      }
    );
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">{data?.projectName ?? t("candidates.titleFallback")}</h1>
        <p className="text-sm text-muted-foreground">
          {t("candidates.count", { count: data?.candidates.length ?? 0 })}
        </p>
      </div>

      <div className="space-y-4">
        {data?.candidates.map((candidate) => (
          <CandidateCard
            key={candidate.talentId}
            candidate={candidate}
            onFeedback={handleFeedback}
            isSubmitting={feedbackMutation.isPending}
          />
        ))}
      </div>

      {(!data?.candidates || data.candidates.length === 0) && (
        <Card>
          <CardContent className="py-8 text-center text-muted-foreground">
            {t("candidates.none")}
          </CardContent>
        </Card>
      )}
    </div>
  );
}

interface CandidateCardProps {
  candidate: MatchCandidate;
  onFeedback: (interactionId: number, feedbackType: FeedbackType) => void;
  isSubmitting: boolean;
}

function CandidateCard({ candidate, onFeedback, isSubmitting }: CandidateCardProps) {
  const { t } = useI18n();
  const interactionId = candidate.interactionId;

  const handleViewDetail = () => {
    trackViewedDetail(interactionId);
  };

  const handleContact = () => {
    trackClickedContact(interactionId, "email");
  };

  const handleCopy = () => {
    trackCopiedTemplate(interactionId, "default");
  };

  const handleShortlist = () => {
    trackShortlisted(interactionId, true);
  };

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between">
          <div onClick={handleViewDetail} className="cursor-pointer">
            <CardTitle>{candidate.talentName ?? t("candidates.talentFallback", { id: candidate.talentId })}</CardTitle>
            <CardDescription>
              {candidate.skills.join(" / ")}
            </CardDescription>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="secondary">
              {t("candidates.score")}: {candidate.totalScore.toFixed(2)}
            </Badge>
            {candidate.twoTowerScore && (
              <Badge variant="outline">
                {t("candidates.twoTowerScore")}: {candidate.twoTowerScore.toFixed(2)}
              </Badge>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between">
          <div className="text-sm text-muted-foreground space-y-1">
            <div>
              {t("candidates.priceLabel")}: {candidate.desiredPriceMin ? `${candidate.desiredPriceMin / 10000}万〜` : "-"}
            </div>
            <div>
              {t("candidates.locationAvailability", {
                location: candidate.residentialTodofuken ?? "-",
                availability: candidate.availabilityDate ?? "-",
              })}
            </div>
            {candidate.koResult.needsManualReview && (
              <Badge variant="secondary" className="mt-1">
                {t("candidates.reviewRequired", {
                  reason: candidate.koResult.reasons.join(", "),
                })}
              </Badge>
            )}
          </div>

          <div className="flex items-center gap-2">
            {/* Action Buttons */}
            <Button
              variant="ghost"
              size="sm"
              onClick={handleShortlist}
              title={t("candidates.buttons.shortlist")}
              aria-label={t("candidates.buttons.shortlist")}
            >
              <Star className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCopy}
              title={t("candidates.buttons.copyTemplate")}
              aria-label={t("candidates.buttons.copyTemplate")}
            >
              <Copy className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleContact}
              title={t("candidates.buttons.contact")}
              aria-label={t("candidates.buttons.contact")}
            >
              <Mail className="h-4 w-4" />
            </Button>

            {/* Feedback Buttons */}
            <Button
              variant="outline"
              size="sm"
              onClick={() => onFeedback(interactionId, "thumbs_up")}
              disabled={isSubmitting}
            >
              {isSubmitting ? (
                <Loader2 className="h-4 w-4 mr-1 animate-spin" />
              ) : (
                <ThumbsUp className="h-4 w-4 mr-1" />
              )}
              {isSubmitting ? t("candidates.buttons.submitting") : t("candidates.buttons.good")}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onFeedback(interactionId, "thumbs_down")}
              disabled={isSubmitting}
            >
              {isSubmitting ? (
                <Loader2 className="h-4 w-4 mr-1 animate-spin" />
              ) : (
                <ThumbsDown className="h-4 w-4 mr-1" />
              )}
              {isSubmitting ? t("candidates.buttons.submitting") : t("candidates.buttons.ng")}
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
