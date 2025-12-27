import { Loader2, MessageCircle, ThumbsDown, ThumbsUp } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { ProjectMatch } from "@/api";

type TalentMatchCardProps = {
  match: ProjectMatch;
  onPropose: (interactionId: number) => void;
  onReject: (interactionId: number) => void;
  onViewDetail: (interactionId: number) => void;
  isBusy?: boolean;
  isDisabled?: boolean;
  labels: { propose: string; reject: string; details: string; scoreLabel?: string; rateLabel?: string };
};

export function TalentMatchCard({
  match,
  onPropose,
  onReject,
  onViewDetail,
  isBusy = false,
  isDisabled = false,
  labels,
}: TalentMatchCardProps) {
  const skills = match.keySkills ?? [];
  const visibleSkills = skills.slice(0, 4);
  const remainingSkills = skills.length - visibleSkills.length;
  const rateRange =
    match.desiredRateMin !== null && match.desiredRateMin !== undefined
      ? `¥${match.desiredRateMin.toLocaleString()}${match.desiredRateMax ? ` - ¥${match.desiredRateMax.toLocaleString()}` : ""}`
      : null;

  return (
    <Card>
      <CardHeader className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="space-y-2">
          <CardTitle>{match.talentName ?? `Talent #${match.talentId}`}</CardTitle>
          {match.headline ? <CardDescription>{match.headline}</CardDescription> : null}
          <div className="flex flex-wrap gap-2">
            <Badge variant="secondary">
              {labels.scoreLabel ?? "Score"}: {match.score.toFixed(1)}
            </Badge>
            {visibleSkills.map((skill) => (
              <Badge key={skill} variant="outline">
                {skill}
              </Badge>
            ))}
            {remainingSkills > 0 ? (
              <Badge variant="outline" className="text-muted-foreground">
                +{remainingSkills}
              </Badge>
            ) : null}
          </div>
        </div>
        <div className="text-sm text-muted-foreground space-y-1 text-left sm:text-right">
          {rateRange ? <div>{labels.rateLabel ?? "Rate"}: {rateRange}</div> : null}
        </div>
      </CardHeader>
      <CardContent className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => onPropose(match.interactionId)}
            disabled={isDisabled}
            aria-label={labels.propose}
          >
            {isBusy ? <Loader2 className="mr-2 h-4 w-4 animate-spin" /> : <ThumbsUp className="mr-2 h-4 w-4" />}
            {labels.propose} 👍
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onReject(match.interactionId)}
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
