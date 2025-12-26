import { useParams } from "react-router-dom";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ThumbsUp, ThumbsDown, Star, Mail, Copy } from "lucide-react";
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

export function CandidatesPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const id = Number(projectId);

  const { data, isLoading, error } = useCandidates(id);
  const feedbackMutation = useSendFeedback();

  if (isLoading) {
    return <div className="text-muted-foreground">Loading candidates...</div>;
  }

  if (error) {
    return (
      <div className="text-destructive">
        Error: {error instanceof Error ? error.message : "Unknown error"}
      </div>
    );
  }

  const handleFeedback = (interactionId: number, feedbackType: FeedbackType) => {
    feedbackMutation.mutate(
      { interactionId, feedbackType, source: "gui" },
      {
        onSuccess: () => {
          toast.success("Feedback sent");
        },
        onError: (err) => {
          toast.error(`Failed to send feedback: ${err.message}`);
        },
      }
    );
  };

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold">{data?.projectName ?? "Project"}</h1>
        <p className="text-sm text-muted-foreground">
          {data?.candidates.length ?? 0} candidates found
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
            No candidates found for this project.
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
            <CardTitle>{candidate.talentName ?? `Talent #${candidate.talentId}`}</CardTitle>
            <CardDescription>
              {candidate.skills.join(" / ")}
            </CardDescription>
          </div>
          <div className="flex items-center gap-2">
            <Badge variant="secondary">
              Score: {candidate.totalScore.toFixed(2)}
            </Badge>
            {candidate.twoTowerScore && (
              <Badge variant="outline">
                TT: {candidate.twoTowerScore.toFixed(2)}
              </Badge>
            )}
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between">
          <div className="text-sm text-muted-foreground space-y-1">
            <div>
              希望単価: {candidate.desiredPriceMin ? `${candidate.desiredPriceMin / 10000}万〜` : "-"}
            </div>
            <div>
              {candidate.residentialTodofuken ?? "-"} / {candidate.availabilityDate ?? "未定"}
            </div>
            {candidate.koResult.needsManualReview && (
              <Badge variant="secondary" className="mt-1">
                要レビュー: {candidate.koResult.reasons.join(", ")}
              </Badge>
            )}
          </div>

          <div className="flex items-center gap-2">
            {/* Action Buttons */}
            <Button
              variant="ghost"
              size="sm"
              onClick={handleShortlist}
              title="Shortlist"
              aria-label="Shortlist"
            >
              <Star className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCopy}
              title="Copy Template"
              aria-label="Copy Template"
            >
              <Copy className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleContact}
              title="Contact"
              aria-label="Contact"
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
              <ThumbsUp className="h-4 w-4 mr-1" />
              Good
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onFeedback(interactionId, "thumbs_down")}
              disabled={isSubmitting}
            >
              <ThumbsDown className="h-4 w-4 mr-1" />
              NG
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
