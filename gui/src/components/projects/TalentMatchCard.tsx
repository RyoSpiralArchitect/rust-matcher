import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import type { ProjectMatch } from "@/api";
import { InteractionBadge } from "@/components/common/InteractionBadge";
import { InteractionActionButton } from "@/components/common/InteractionActionButton";
import { componentTheme, type InteractionAction, type InteractionState } from "@/theme/component-theme";

type TalentMatchCardProps = {
  match: ProjectMatch;
  onPropose?: (interactionId: number) => void;
  onReject?: (interactionId: number) => void;
  onInterview?: (interactionId: number) => void;
  onViewDetail?: (interactionId: number) => void;
  isBusy?: boolean;
  isDisabled?: boolean;
  labels: {
    propose?: string;
    reject?: string;
    interview?: string;
    details?: string;
    scoreLabel?: string;
    rateLabel?: string;
  };
  statusLabel?: string;
  statusState?: InteractionState;
};

export function TalentMatchCard({
  match,
  onPropose,
  onReject,
  onInterview,
  onViewDetail,
  isBusy = false,
  isDisabled = false,
  labels,
  statusLabel,
  statusState,
}: TalentMatchCardProps) {
  const skills = match.keySkills ?? [];
  const visibleSkills = skills.slice(0, 4);
  const remainingSkills = skills.length - visibleSkills.length;
  const rateRange =
    match.desiredRateMin !== null && match.desiredRateMin !== undefined
      ? `¥${match.desiredRateMin.toLocaleString()}${match.desiredRateMax ? ` - ¥${match.desiredRateMax.toLocaleString()}` : ""}`
      : null;
  const actions: { action: InteractionAction; label: string; handler: () => void }[] = [];

  if (onPropose) {
    actions.push({ action: "propose", label: labels.propose ?? "Propose", handler: () => onPropose(match.interactionId) });
  }

  if (onReject) {
    actions.push({ action: "reject", label: labels.reject ?? "Reject", handler: () => onReject(match.interactionId) });
  }

  if (onInterview) {
    actions.push({
      action: "interview",
      label: labels.interview ?? "Interview",
      handler: () => onInterview(match.interactionId),
    });
  }

  if (onViewDetail) {
    actions.push({ action: "details", label: labels.details ?? "Details", handler: () => onViewDetail(match.interactionId) });
  }

  return (
    <Card>
      <CardHeader className={componentTheme.layout.cardHeader}>
        <div className="space-y-2">
          <CardTitle>{match.talentName ?? `Talent #${match.talentId}`}</CardTitle>
          {match.headline ? <CardDescription>{match.headline}</CardDescription> : null}
          <div className={componentTheme.layout.badgeRow}>
            {statusLabel && statusState ? (
              <InteractionBadge state={statusState} label={statusLabel} />
            ) : null}
            <Badge variant="secondary" className={componentTheme.badges.metric}>
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
        <div className={componentTheme.layout.metaColumn}>
          {rateRange ? <div>{labels.rateLabel ?? "Rate"}: {rateRange}</div> : null}
        </div>
      </CardHeader>
      <CardContent className={componentTheme.spacing.cardStack}>
        <div className={componentTheme.layout.actionRow}>
          {actions.map(({ action, label, handler }) => (
            <InteractionActionButton
              key={action}
              action={action}
              label={label}
              onClick={handler}
              disabled={isDisabled}
              isBusy={isBusy}
            />
          ))}
        </div>
      </CardContent>
    </Card>
  );
}
